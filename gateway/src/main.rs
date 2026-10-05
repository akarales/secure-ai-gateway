//! Binary entry: wire detector/tokens/audit, serve.

use std::sync::Arc;

use secure_ai_gateway::audit::AuditLog;
use secure_ai_gateway::config::Config;
use secure_ai_gateway::detector::Detector;
use secure_ai_gateway::routes;
use secure_ai_gateway::state::AppState;
use secure_ai_gateway::tokenize::TokenStore;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,secure_ai_gateway=debug".into()),
        )
        .init();

    let config = Config::from_env()?;

    let detector = if config.detector_stub {
        tracing::info!("detector: stub (no Presidio sidecar)");
        Detector::Stub
    } else {
        tracing::info!(url = %config.detector_url, "detector: presidio sidecar");
        Detector::Sidecar {
            url: config.detector_url.clone(),
            http: reqwest::Client::new(),
        }
    };

    let tokens = if config.redis_stub {
        tracing::info!("token store: in-process (no Redis)");
        TokenStore::memory()
    } else {
        match TokenStore::connect_redis(&config.redis_url, config.token_ttl_seconds).await {
            Ok(store) => {
                tracing::info!(url = %config.redis_url, "token store: redis");
                store
            }
            Err(err) => {
                tracing::warn!(%err, "redis unavailable; falling back to in-process tokens");
                TokenStore::memory()
            }
        }
    };

    let audit = AuditLog::new(&config.audit_path);
    let port = config.port;

    let state = AppState {
        config: Arc::new(config),
        detector,
        tokens,
        audit,
        http: reqwest::Client::new(),
    };

    let app = routes::router(state).layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(%port, "secure ai gateway listening");
    axum::serve(listener, app).await?;
    Ok(())
}
