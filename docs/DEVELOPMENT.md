# Development Guide

Machine-facing commands live in [AGENTS.md](../AGENTS.md).

## Prerequisites

- Rust 1.96, cargo
- Docker (full stack) — or nothing for the offline demo
- For the sidecar: uv + a spaCy model (below)

## Offline demo (zero infra)

```bash
APP_DETECTOR_STUB=true APP_MODEL_STUB=true APP_REDIS_STUB=true \
  cargo run --bin secure-ai-gateway
```

All three stubs are independent — mix freely (e.g. real sidecar + memory
tokens + stub model).

## Presidio sidecar

```bash
cd detector
uv sync
uv run python -m spacy download en_core_web_sm   # once; lg for accuracy
uv run uvicorn app.main:app --port 8006
uv run pytest                                    # skips if the model is missing
```

The sidecar is stateless: `/detect` + `/health`. Its Dockerfile
(`detector/Dockerfile`) is uv-based; compose wires it with Redis and the
gateway.

## Testing strategy

- `cargo test --workspace -q` runs the FULL PHI loop against a fake
  Presidio — an in-test axum server serving canned entities. Assertions:
  sanitized prompt has tokens (not PHI), response has originals, audit
  has no PHI, routing tiers are correct
- The detector's Python tests skip when the spaCy model isn't installed;
  CI compile-checks the sidecar without downloading the model

## Full stack

```bash
docker compose up --build     # redis (:6380) + detector (:8006) + gateway (:8005)
```

`APP_MODEL_STUB` defaults to true in compose — flip to false when
Ollama is reachable.

## Gotchas learned here

- **setup-uv action**: the `v10` major tag doesn't exist — pin
  `astral-sh/setup-uv@v10.0.1`
- **Token kinds with underscores** (`MEDICAL_RECORD_NUMBER`): the nonce
  boundary is the LAST underscore — `rsplit_once('_')`
- **CI clippy is newer than local stable** — a `Result<_, axum::Response>`
  tripped `result_large_err` remotely; the error payload is boxed
- Port 8006 is the sidecar's; the gateway owns 8005

## Conventions

Conventional commits; hygiene hook strips AI attribution. PHI handling
rule: raw values never enter the audit log, tracing fields, or error
messages — tokens and counts only.
