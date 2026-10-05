//! Routes: health + the gateway chat proxy.

use axum::Router;
use axum::routing::{get, post};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/api/v1/gateway/chat", post(chat))
        .route("/api/v1/gateway/route", post(route_preview))
        .with_state(state)
}

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::audit::AuditRecord;
use crate::error::ApiError;
use crate::state::AppState;
use crate::tokenize::{restore, sanitize};

pub mod health {
    use axum::Json;
    use axum::extract::State;
    use serde_json::json;

    use crate::state::AppState;

    pub async fn health(State(_state): State<AppState>) -> Json<serde_json::Value> {
        Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
        }))
    }
}

#[derive(Debug, Deserialize)]
pub struct GatewayRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize)]
struct ModelChoice {
    tier: &'static str,
    model: String,
    reason: &'static str,
}

/// Complexity-based tier routing (the ZAP Runtime model router): prompts
/// that look clinical/long go to the capable tier; short/simple ones to
/// the fast tier.
fn route(prompt: &str, base_model: &str) -> ModelChoice {
    let clinical = [
        "patient",
        "diagnosis",
        "dose",
        "medication",
        "lab",
        "symptom",
    ]
    .iter()
    .any(|k| prompt.to_lowercase().contains(k));
    if prompt.len() > 240 || clinical {
        ModelChoice {
            tier: "complex",
            model: base_model.to_string(),
            reason: "clinical context or long prompt",
        }
    } else {
        ModelChoice {
            tier: "simple",
            model: base_model.to_string(),
            reason: "short general prompt",
        }
    }
}

pub async fn chat(State(state): State<AppState>, Json(request): Json<GatewayRequest>) -> Response {
    if request.prompt.trim().is_empty() {
        return ApiError::BadRequest("prompt must not be empty".into()).into_response();
    }
    let started = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();

    // 1. Detect PHI.
    let entities = match state.detector.detect(&request.prompt).await {
        Ok(entities) => entities,
        Err(err) => return ApiError::Detector(err.to_string()).into_response(),
    };

    // 2. Tokenize each entity and build the sanitized prompt.
    let mut tokenized: Vec<(String, String, usize, usize)> = Vec::new();
    for entity in &entities {
        let token = state.tokens.tokenize(&entity.kind, &entity.text).await;
        tokenized.push((token, entity.text.clone(), entity.start, entity.end));
    }
    let sanitized = sanitize(&request.prompt, &tokenized);

    // 3. Route + call the model tier (stub echoes the sanitized prompt).
    let choice = route(&request.prompt, &state.config.ollama_model);
    let model_response = if state.config.model_stub {
        format!("Ack: {sanitized}")
    } else {
        match call_ollama(&state, &choice.model, &sanitized).await {
            Ok(response) => response,
            Err(err) => return ApiError::Model(err.to_string()).into_response(),
        }
    };

    // 4. Reverse-map tokens in the response.
    let restored = restore(&model_response, &state.tokens).await;
    let restored_count = model_response.matches('<').count();

    // 5. Audit (counts and kinds only — never raw PHI).
    let record = AuditRecord {
        at: chrono::Utc::now(),
        request_id,
        tier: choice.tier.to_string(),
        model: choice.model.clone(),
        entities_detected: entities.len(),
        entity_kinds: entities.iter().map(|e| e.kind.clone()).collect(),
        tokens_created: tokenized.len(),
        response_tokens_restored: restored_count,
        latency_ms: started.elapsed().as_millis(),
    };
    if let Err(err) = state.audit.append(&record) {
        tracing::warn!(%err, "audit append failed");
    }

    let payload = json!({
        "response": restored,
        "tier": choice.tier,
        "model": choice.model,
        "routing_reason": choice.reason,
        "phi_detected": entities.len(),
        "entity_kinds": record.entity_kinds,
        "sanitized_prompt": sanitized,
        "stub_detector": matches!(state.detector, crate::detector::Detector::Stub),
        "stub_model": state.config.model_stub,
    });
    (StatusCode::OK, Json(payload)).into_response()
}

async fn call_ollama(state: &AppState, model: &str, prompt: &str) -> Result<String, String> {
    #[derive(Debug, Deserialize)]
    struct ChatResponse {
        message: ChatMessage,
    }
    #[derive(Debug, Deserialize)]
    struct ChatMessage {
        content: String,
    }

    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false,
        "options": {"temperature": 0.2}
    });
    let response: ChatResponse = state
        .http
        .post(format!("{}/api/chat", state.config.ollama_url))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("ollama unreachable: {e}"))?
        .error_for_status()
        .map_err(|e| format!("ollama status: {e}"))?
        .json()
        .await
        .map_err(|e| format!("ollama body: {e}"))?;
    Ok(response.message.content)
}

/// Config inspection for demos: what would the router choose?
pub async fn route_preview(
    State(state): State<AppState>,
    Json(request): Json<GatewayRequest>,
) -> Json<Value> {
    let choice = route(&request.prompt, &state.config.ollama_model);
    Json(json!({
        "tier": choice.tier,
        "model": choice.model,
        "reason": choice.reason,
    }))
}
