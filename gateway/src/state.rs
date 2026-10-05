//! Shared state: detector, token store, audit log, model client.

use std::sync::Arc;

use crate::audit::AuditLog;
use crate::config::Config;
use crate::detector::Detector;
use crate::tokenize::TokenStore;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub detector: Detector,
    pub tokens: TokenStore,
    pub audit: AuditLog,
    pub http: reqwest::Client,
}
