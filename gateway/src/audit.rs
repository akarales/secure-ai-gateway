//! Append-only audit log (JSONL) — every gateway request leaves a record.
//! Raw PHI is NEVER written: audit entries carry counts and token kinds,
//! not the original values.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuditRecord {
    pub at: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
    pub tier: String,
    pub model: String,
    pub entities_detected: usize,
    pub entity_kinds: Vec<String>,
    pub tokens_created: usize,
    pub response_tokens_restored: usize,
    pub latency_ms: u128,
}

#[derive(Clone)]
pub struct AuditLog {
    path: PathBuf,
    lock: std::sync::Arc<Mutex<()>>,
}

#[derive(Debug, thiserror::Error)]
#[error("audit log error: {0}")]
pub struct AuditError(String);

impl AuditLog {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lock: std::sync::Arc::new(Mutex::new(())),
        }
    }

    pub fn append(&self, record: &AuditRecord) -> Result<(), AuditError> {
        let line =
            serde_json::to_string(record).map_err(|e| AuditError(format!("serialize: {e}")))?;
        let _guard = self.lock.lock().expect("audit lock");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| AuditError(format!("open {}: {e}", self.path.display())))?;
        writeln!(file, "{line}").map_err(|e| AuditError(format!("write: {e}")))
    }
}
