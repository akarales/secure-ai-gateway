//! secure-ai-gateway — PHI-aware AI gateway (Rust core).
//!
//! Architecture (the ZAP Runtime pattern applied to PHI):
//! 1. Inbound prompt → PHI **detection** (Presidio sidecar over HTTP)
//! 2. Every detected entity → deterministic **token** (`<PERSON_ab12cd34>`);
//!    token→original mapping stored in Redis with TTL
//! 3. Sanitized prompt → **routed** to the model tier (local Ollama)
//! 4. Response tokens → **reverse-mapped** before returning
//! 5. Every request appended to an append-only **audit log** (JSONL)
//!
//! The `detector` is a trait: `SidecarDetector` calls Presidio; the
//! `StubDetector` (default in tests) finds nothing — so `cargo test`
//! never needs the sidecar, Redis, or a GPU.

pub mod audit;
pub mod config;
pub mod detector;
pub mod error;
pub mod routes;
pub mod state;
pub mod tokenize;
