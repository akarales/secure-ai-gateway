//! Gateway integration tests. A local axum server impersonates the
//! Presidio sidecar (canned entities) so the FULL loop — detect →
//! tokenize → route → model-stub → restore — runs with zero external
//! services. No Python, no Redis, no GPU.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use tower::ServiceExt;

use secure_ai_gateway::audit::AuditLog;
use secure_ai_gateway::config::Config;
use secure_ai_gateway::detector::Detector;
use secure_ai_gateway::routes;
use secure_ai_gateway::state::AppState;
use secure_ai_gateway::tokenize::TokenStore;

/// Fake Presidio sidecar: detects a fixed NAME + MRN in any text.
async fn fake_sidecar() -> String {
    let app = Router::new().route(
        "/detect",
        post(|Json(body): Json<Value>| async move {
            let text = body["text"].as_str().unwrap_or_default();
            let mut entities = Vec::new();
            if let Some(start) = text.find("John Smith") {
                entities.push(json!({
                    "kind": "PERSON", "text": "John Smith",
                    "start": start, "end": start + 10, "score": 0.99
                }));
            }
            if let Some(start) = text.find("MRN-12345") {
                entities.push(json!({
                    "kind": "MEDICAL_RECORD_NUMBER", "text": "MRN-12345",
                    "start": start, "end": start + 9, "score": 0.95
                }));
            }
            Json(json!({ "entities": entities }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

fn test_config(detector_url: String) -> Config {
    Config {
        detector_url,
        detector_stub: false,
        redis_url: "redis://unused".to_string(),
        redis_stub: true,
        ollama_url: "http://unused".to_string(),
        ollama_model: "qwen3:14b".to_string(),
        model_stub: true,
        audit_path: std::env::temp_dir().join("gateway-test-audit.jsonl"),
        port: 0,
        token_ttl_seconds: 60,
    }
}

async fn gateway_app() -> axum::Router {
    let config = Arc::new(test_config(fake_sidecar().await));
    let state = AppState {
        detector: Detector::Sidecar {
            url: config.detector_url.clone(),
            http: reqwest::Client::new(),
        },
        tokens: TokenStore::memory(),
        audit: AuditLog::new(&config.audit_path),
        http: reqwest::Client::new(),
        config,
    };
    routes::router(state)
}

async fn post_json(path: &str, prompt: &str) -> (StatusCode, Value) {
    let app = gateway_app().await;
    let response = app
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "prompt": prompt }).to_string()))
                .expect("request builds"),
        )
        .await
        .expect("in-process request");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json"))
}

#[tokio::test]
async fn health_ok() {
    let app = routes::router(AppState {
        config: Arc::new(test_config("unused".into())),
        detector: Detector::Stub,
        tokens: TokenStore::memory(),
        audit: AuditLog::new(std::env::temp_dir().join("gateway-test-audit.jsonl")),
        http: reqwest::Client::new(),
    });
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).expect("builds"))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn phi_loop_tokens_in_and_originals_out() {
    let prompt = "Patient John Smith (MRN-12345) reports chest pain.";
    let (status, body) = post_json("/api/v1/gateway/chat", prompt).await;
    assert_eq!(status, StatusCode::OK);

    // The sanitized prompt (model input) must NOT contain the PHI.
    let sanitized = body["sanitized_prompt"].as_str().expect("sanitized");
    assert!(!sanitized.contains("John Smith"), "name must be tokenized");
    assert!(!sanitized.contains("MRN-12345"), "MRN must be tokenized");
    assert!(sanitized.contains("<PERSON_"), "person token present");
    assert!(
        sanitized.contains("<MEDICAL_RECORD_NUMBER_"),
        "mrn token present"
    );
    assert!(sanitized.contains("chest pain"), "clinical text survives");

    // The returned response must contain the ORIGINALS again.
    let response = body["response"].as_str().expect("response");
    assert!(response.contains("John Smith"), "name restored");
    assert!(response.contains("MRN-12345"), "MRN restored");

    // Accounting.
    assert_eq!(body["phi_detected"], 2);
    let kinds = body["entity_kinds"].as_array().expect("kinds");
    assert!(kinds.iter().any(|k| k == "PERSON"));
    assert_eq!(body["stub_model"], true);
}

#[tokio::test]
async fn routing_tier_by_clinical_context() {
    let (_, clinical) = post_json(
        "/api/v1/gateway/route",
        "Summarize this patient's lab results",
    )
    .await;
    assert_eq!(clinical["tier"], "complex");

    let (_, simple) = post_json("/api/v1/gateway/route", "Say hello").await;
    assert_eq!(simple["tier"], "simple");
}

#[tokio::test]
async fn audit_log_written_without_phi() {
    let audit_path = std::env::temp_dir().join("gateway-test-audit-run.jsonl");
    let _ = std::fs::remove_file(&audit_path);

    let sidecar_url = fake_sidecar().await;
    let config = test_config(sidecar_url);
    let state = AppState {
        config: Arc::new(config.clone()),
        detector: Detector::Sidecar {
            url: config.detector_url.clone(),
            http: reqwest::Client::new(),
        },
        tokens: TokenStore::memory(),
        audit: AuditLog::new(&audit_path),
        http: reqwest::Client::new(),
    };
    let app = routes::router(state);
    let response = app
        .oneshot(
            Request::post("/api/v1/gateway/chat")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "prompt": "Patient John Smith has a headache." }).to_string(),
                ))
                .expect("builds"),
        )
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let raw = std::fs::read_to_string(&audit_path).expect("audit file exists");
    assert!(!raw.contains("John Smith"), "audit must never contain PHI");
    let record: Value = serde_json::from_str(raw.lines().next().expect("one line")).expect("jsonl");
    assert_eq!(record["entities_detected"], 1);
    assert_eq!(record["entity_kinds"][0], "PERSON");
}

#[tokio::test]
async fn empty_prompt_rejected() {
    let (status, _) = post_json("/api/v1/gateway/chat", "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
