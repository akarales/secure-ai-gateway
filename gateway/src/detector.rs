//! PHI detection: Presidio sidecar (Python) behind enum dispatch so
//! tests and offline demos run without the sidecar.

use serde::{Deserialize, Serialize};

/// One detected PHI entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub kind: String,
    pub text: String,
    /// Character offsets in the source prompt.
    pub start: usize,
    pub end: usize,
    pub score: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum DetectorError {
    #[error("sidecar error: {0}")]
    Sidecar(String),
}

#[derive(Clone)]
pub enum Detector {
    /// Python Presidio sidecar: POST /detect {"text": "..."}.
    Sidecar { url: String, http: reqwest::Client },
    /// Offline stub: detects nothing (tests, APP_DETECTOR_STUB=true).
    Stub,
}

impl Detector {
    pub async fn detect(&self, text: &str) -> Result<Vec<Entity>, DetectorError> {
        match self {
            Detector::Stub => Ok(Vec::new()),
            Detector::Sidecar { url, http } => {
                #[derive(Debug, Deserialize)]
                struct SidecarResponse {
                    entities: Vec<Entity>,
                }
                let response: SidecarResponse = http
                    .post(format!("{url}/detect"))
                    .json(&serde_json::json!({ "text": text }))
                    .send()
                    .await
                    .map_err(|e| DetectorError::Sidecar(e.to_string()))?
                    .error_for_status()
                    .map_err(|e| DetectorError::Sidecar(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| DetectorError::Sidecar(e.to_string()))?;
                Ok(response
                    .entities
                    .into_iter()
                    .filter(|entity| entity.score >= 0.5)
                    .collect())
            }
        }
    }
}
