//! Environment configuration.

#[derive(Debug, Clone)]
pub struct Config {
    pub detector_url: String,
    /// When true, skip the sidecar entirely (tests, offline demo).
    pub detector_stub: bool,
    pub redis_url: String,
    /// When true, skip Redis and use an in-process token map (offline).
    pub redis_stub: bool,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Offline demo/tests: the "model" echoes the sanitized prompt
    /// (with tokens) so reverse-mapping is exercised without Ollama.
    pub model_stub: bool,
    pub audit_path: std::path::PathBuf,
    pub port: u16,
    /// Token-map TTL in seconds (Redis).
    pub token_ttl_seconds: u64,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {0}")]
pub struct ConfigError(String);

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let audit_path = std::path::PathBuf::from(env_or("APP_AUDIT_PATH", "data/audit.jsonl"));
        if let Some(parent) = audit_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError(format!("cannot create audit dir: {e}")))?;
        }
        Ok(Self {
            detector_url: env_or("APP_DETECTOR_URL", "http://localhost:8006"),
            detector_stub: env_or("APP_DETECTOR_STUB", "false").to_lowercase() == "true",
            redis_url: env_or("APP_REDIS_URL", "redis://localhost:6380"),
            redis_stub: env_or("APP_REDIS_STUB", "false").to_lowercase() == "true",
            ollama_url: env_or("APP_OLLAMA_URL", "http://localhost:11434"),
            ollama_model: env_or("APP_OLLAMA_MODEL", "qwen3:14b"),
            model_stub: env_or("APP_MODEL_STUB", "false").to_lowercase() == "true",
            audit_path,
            port: env_or("APP_PORT", "8005")
                .parse()
                .map_err(|_| ConfigError("APP_PORT must be a valid port".to_string()))?,
            token_ttl_seconds: env_or("APP_TOKEN_TTL", "3600")
                .parse()
                .map_err(|_| ConfigError("APP_TOKEN_TTL must be seconds".to_string()))?,
        })
    }
}
