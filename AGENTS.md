# AGENTS.md

Build, test, and verification commands for the Secure AI Gateway.

## Tooling

- **Rust: cargo** — repo root · **Python (sidecar): uv** — `detector/`

## Rust gateway

```bash
cargo test --workspace -q                          # 5 tests: full PHI loop
                                                    # against a fake Presidio —
                                                    # no Python/Redis/GPU needed
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check

# offline demo server (:8005)
APP_DETECTOR_STUB=true APP_MODEL_STUB=true APP_REDIS_STUB=true \
  cargo run --bin secure-ai-gateway
```

## Python sidecar (uv only)

```bash
cd detector
uv sync
uv run python -m spacy download en_core_web_sm   # once; lg for accuracy
uv run uvicorn app.main:app --port 8006
uv run pytest                                     # skips if model absent
```

## Environment

| Variable | Default | Notes |
|----------|---------|-------|
| `APP_DETECTOR_URL` | `http://localhost:8006` | Presidio sidecar |
| `APP_DETECTOR_STUB` | `false` | stub detects nothing (offline) |
| `APP_REDIS_URL` | `redis://localhost:6380` | token map (compose exposes 6380) |
| `APP_REDIS_STUB` | `false` | in-process token map (offline) |
| `APP_OLLAMA_URL` / `APP_OLLAMA_MODEL` | `…:11434` / `qwen3:14b` | model tier |
| `APP_MODEL_STUB` | `false` | echoes sanitized prompt (offline loop) |
| `APP_AUDIT_PATH` | `data/audit.jsonl` | append-only JSONL |
| `APP_PORT` | `8005` | port map: 8000-8004 taken |

## Conventions

- Conventional commits; hygiene hook strips AI attribution
- PHI handling rules: raw values never enter the audit log, tracing
  fields, or error messages — tokens and counts only
- The detector stays a sidecar: the Rust core never links presidio
- Deps ≥7 days old (BEST_PRACTICES/INDEX.md); gateway pins via Cargo.lock,
  sidecar via uv.lock
