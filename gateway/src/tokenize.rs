//! Forward tokenization + reverse mapping with a TTL token store.
//!
//! Redis in production (token → original, TTL); an in-process map when
//! APP_REDIS_STUB=true or in tests. Tokens are deterministic in form
//! (`<PERSON_ab12cd34>`) so the model sees stable references.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum TokenStoreError {
    #[error("token store error: {0}")]
    Store(String),
}

#[derive(Clone)]
pub enum TokenStore {
    Redis {
        client: redis::aio::ConnectionManager,
        ttl_seconds: u64,
    },
    Memory {
        map: std::sync::Arc<Mutex<HashMap<String, String>>>,
    },
}

impl TokenStore {
    pub fn memory() -> Self {
        TokenStore::Memory {
            map: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn connect_redis(url: &str, ttl_seconds: u64) -> Result<Self, TokenStoreError> {
        let client = redis::Client::open(url)
            .map_err(|e| TokenStoreError::Store(format!("redis client: {e}")))?;
        let manager = client
            .get_connection_manager()
            .await
            .map_err(|e| TokenStoreError::Store(format!("redis connect: {e}")))?;
        Ok(TokenStore::Redis {
            client: manager,
            ttl_seconds,
        })
    }

    /// Generate a token for one entity and remember the mapping.
    pub async fn tokenize(&self, entity_kind: &str, original: &str) -> String {
        let nonce: String = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
        let token = format!("<{}_{}>", entity_kind.to_uppercase(), nonce);
        let key = format!("tok:{token}");
        match self {
            TokenStore::Memory { map } => {
                map.lock()
                    .expect("token map lock")
                    .insert(key, original.to_string());
            }
            TokenStore::Redis {
                client,
                ttl_seconds,
            } => {
                let mut conn = client.clone();
                let result: Result<(), _> = redis::cmd("SETEX")
                    .arg(&key)
                    .arg(*ttl_seconds)
                    .arg(original)
                    .query_async(&mut conn)
                    .await;
                if let Err(err) = result {
                    tracing::warn!(%err, %key, "token store write failed");
                }
            }
        }
        token
    }

    /// Reverse-map a token back to the original value.
    pub async fn reverse(&self, token: &str) -> Option<String> {
        let key = format!("tok:{token}");
        match self {
            TokenStore::Memory { map } => map.lock().expect("token map lock").get(&key).cloned(),
            TokenStore::Redis { client, .. } => {
                let mut conn = client.clone();
                redis::cmd("GET")
                    .arg(&key)
                    .query_async::<Option<String>>(&mut conn)
                    .await
                    .ok()
                    .flatten()
            }
        }
    }
}

/// Replace all detected entity spans with their tokens (forward map).
/// Entities must be non-overlapping; sorted by start offset.
pub fn sanitize(text: &str, entities: &[(String, String, usize, usize)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (token, _original, start, end) in entities {
        let (start, end) = (*start, *end);
        if start < cursor || end > text.len() {
            continue;
        }
        out.push_str(&text[cursor..start]);
        out.push_str(token);
        cursor = end;
    }
    out.push_str(&text[cursor.min(text.len())..]);
    out
}

/// Replace every `<KIND_xxxxxxxx>` token in the text with the original
/// value (reverse map) — unknown tokens pass through untouched.
pub async fn restore(text: &str, store: &TokenStore) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let bytes = text.as_bytes();
    while cursor < bytes.len() {
        if bytes[cursor] == b'<'
            && let Some(close) = text[cursor..].find('>')
        {
            let end = cursor + close + 1;
            let token = &text[cursor..end];
            if is_token(token)
                && let Some(original) = store.reverse(token).await
            {
                out.push_str(&original);
                cursor = end;
                continue;
            }
        }
        let ch = text[cursor..].chars().next().expect("in-bounds char");
        out.push(ch);
        cursor += ch.len_utf8();
    }
    out
}

fn is_token(token: &str) -> bool {
    let inner = token
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .unwrap_or_default();
    // Kinds may contain underscores (MEDICAL_RECORD_NUMBER): split at the
    // LAST underscore so the nonce is exactly 8 hex chars.
    let Some((kind, nonce)) = inner.rsplit_once('_') else {
        return false;
    };
    !kind.is_empty() && nonce.len() == 8 && nonce.chars().all(|c| c.is_ascii_hexdigit())
}
