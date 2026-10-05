# Architecture

## Layout

```
secure-ai-gateway/
├── gateway/            # Rust axum core (the hot path)
│   ├── src/detector.rs  # Detector enum: Sidecar | Stub
│   ├── src/tokenize.rs  # sanitize / restore + TokenStore (Redis | Memory)
│   ├── src/audit.rs     # append-only JSONL
│   ├── src/routes.rs    # chat pipeline + route preview
│   └── tests/gateway.rs # full-loop tests with a fake Presidio
└── detector/           # Python Presidio sidecar (uv)
    ├── app/main.py      # POST /detect — entities with offsets
    └── tests/           # skip gracefully without the spaCy model
```

## The pipeline (one request)

1. **Detect** — the gateway calls the sidecar's `/detect` with the prompt;
   entities come back with `kind`, `text`, character offsets, and scores
   (≥0.5 kept). Enum dispatch (`Sidecar`/`Stub`) so tests swap the backend
   without touching the pipeline.
2. **Tokenize** — each entity becomes a deterministic
   `<KIND_hex8>` token (e.g. `<MEDICAL_RECORD_NUMBER_ab12cd34>`); the
   token→original mapping is stored via Redis `SETEX` with TTL (or an
   in-process map offline). `sanitize()` replaces every entity span in
   the prompt, in offset order, skipping overlaps.
3. **Route** — the ZAP Runtime model-router pattern: clinical keywords
   (`patient`, `dose`, `medication`, …) or prompts >240 chars go to the
   capable tier; short general prompts to the fast tier. `route_preview`
   exposes the decision for demos.
4. **Model** — Ollama, or an echo stub that returns the sanitized prompt
   verbatim (exercises restoration without a GPU).
5. **Restore** — `restore()` walks the response, replacing every valid
   token (`rsplit_once('_')` on the inner text so underscore-bearing
   kinds split at the LAST underscore; nonce must be 8 hex chars) with its
   original. Unknown tokens pass through untouched.
6. **Audit** — an append-only JSONL record: request id, tier, model,
   entity counts, entity KINDS (never values), tokens created, restored
   count, latency. A test asserts the audit file contains no PHI.

## Safety rules encoded in tests

- The sanitized prompt must not contain the PHI (names, MRNs)
- The returned response must contain the ORIGINALS again
- The audit log must never contain raw PHI — counts and kinds only
- Tracing fields carry the same discipline

## Why a Python sidecar at all

Presidio (analyzer + spaCy NER) has no serious Rust equivalent, and
production PHI gateways genuinely use this split: a Rust hot path that
never links the NER stack, plus a small sidecar that can crash/restart
independently. The sidecar is stateless and tiny — `/detect` and
`/health` only; detection quality comes from Presidio's recognizer
registry, not application code.

## Stub modes (all three independent)

| Stub | Env | What it does |
|------|-----|--------------|
| Detector | `APP_DETECTOR_STUB=true` | detects nothing — pipeline runs, zero entities |
| Model | `APP_MODEL_STUB=true` | echoes the sanitized prompt — reverse-mapping exercised offline |
| Redis | `APP_REDIS_STUB=true` | in-process token map — TTL semantics live only in Redis mode |

`cargo test` uses a real axum server in-test impersonating Presidio with
canned entities — the FULL loop (detect → tokenize → route → restore →
audit) runs with zero Python, Redis, or GPU.
