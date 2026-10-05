# Secure AI Gateway

A working demo of PHI-aware AI routing — the ZAP Runtime gateway
architecture applied to protected health information. Every prompt that
crosses the model boundary is **detected** (Presidio), **tokenized**
(`<PERSON_ab12cd34>` with a TTL Redis map), **tier-routed** (complexity
model selection), and **reverse-mapped** on return — with an append-only
audit log recording every hop. Rust (axum) gateway core; the PHI detector
is a small Python Presidio sidecar (the one component with no serious
Rust equivalent).

> Demo application. Not HIPAA-certified. Never route real PHI through a
> demo gateway.

## The pipeline

```
            prompt
              │
   ┌──────────▼───────────┐   POST /detect
   │ Presidio sidecar     │──────────────► Python (FastAPI)
   │ (Rust gateway calls) │◄────────────── entities + offsets
   └──────────┬───────────┘
              │ tokenize (<PERSON_ab12cd34>) + Redis SETEX map
   ┌──────────▼───────────┐
   │ tier router          │  clinical/long → capable model
   │ (ZAP model-router)   │  short/simple → fast model
   └──────────┬───────────┘
              │ sanitized prompt
   ┌──────────▼───────────┐
   │ model (Ollama|stub)  │
   └──────────┬───────────┘
              │ reverse-map tokens → originals
   ┌──────────▼───────────┐
   │ audit log (JSONL)    │  counts + kinds only — never raw PHI
   └──────────────────────┘
              │
           response
```

## What it demonstrates

- **PHI never crosses the model boundary**: prompts are sanitized before
  the model sees them; responses are restored before the user does
- **Tokenization with TTL**: Redis `SETEX` token→original map (in-process
  fallback for offline) — deterministic tokens the model can reason over
- **Tiered routing**: the ZAP Runtime model-router pattern — clinical
  context or long prompts route to the capable tier
- **Audit without exposure**: append-only JSONL records entity counts and
  kinds, never values
- **Offline-everything stubs**: detector, model, and token store all have
  stub modes — `cargo test` needs no Python, Redis, or GPU (an axum fake
  sidecar impersonates Presidio in the integration test)

## Quickstart

```bash
# Rust gateway, fully offline (stub detector + stub model + memory tokens)
APP_DETECTOR_STUB=true APP_MODEL_STUB=true APP_REDIS_STUB=true \
  cargo run --bin secure-ai-gateway        # :8005

curl -X POST localhost:8005/api/v1/gateway/chat \
  -H 'content-type: application/json' \
  -d '{"prompt":"Patient John Smith has a question about his medication"}'

# Full stack (Redis + Presidio + Ollama)
docker compose up --build
```

Sidecar development (optional):

```bash
cd detector
uv sync
uv run python -m spacy download en_core_web_sm
uv run uvicorn app.main:app --port 8006
uv run pytest            # skips gracefully if the model is missing
```

## API surface

| Endpoint | Purpose |
|----------|---------|
| `GET /health` | liveness |
| `POST /api/v1/gateway/chat` | full PHI-aware proxy round trip |
| `POST /api/v1/gateway/route` | routing preview (which tier, why) |

Response fields include `sanitized_prompt` (what the model saw),
`phi_detected`, `entity_kinds`, `tier`, and the restored `response` —
so the whole pipeline is observable in one payload.

## Project structure

```
├── gateway/            # Rust axum core: detector, tokenize, audit, routes
│   └── tests/          # integration tests incl. fake-Presidio full loop
├── detector/          # Python Presidio sidecar (uv): /detect + /health
└── .github/workflows/ # CI: fmt + clippy + cargo test
```

## Roadmap

- [x] Phase 0 — scaffold: pipeline, stubs, audit, CI
- [ ] Phase 1 — streaming responses (NDJSON) with token restoration
- [ ] Phase 2 — real model routing (qwen3:8b / qwen3:14b), WireGuard-only
      listener policy (the deployment doc lives in ZAP_RUNTIME)
- [ ] Phase 3 — policy engine: per-tenant PHI classes + redaction deny lists

## License

MIT — see [LICENSE](LICENSE).
