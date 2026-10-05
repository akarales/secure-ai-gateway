# API Reference

Base URL: `http://localhost:8005`.

## Health

```bash
curl localhost:8005/health
```

```json
{ "status": "ok", "version": "0.1.0" }
```

## Chat (the full pipeline)

```bash
curl -X POST localhost:8005/api/v1/gateway/chat \
  -H 'content-type: application/json' \
  -d '{"prompt":"Patient John Smith (MRN-12345) reports chest pain."}'
```

With the sidecar + model stub, the response anatomy:

```json
{
  "response": "Ack: Patient John Smith (MRN-12345) reports chest pain.",
  "tier": "complex",
  "model": "qwen3:14b",
  "routing_reason": "clinical context or long prompt",
  "phi_detected": 2,
  "entity_kinds": ["PERSON", "MEDICAL_RECORD_NUMBER"],
  "sanitized_prompt": "Patient <PERSON_ab12cd34> (<MEDICAL_RECORD_NUMBER_9f8e7d6c>) reports chest pain.",
  "stub_detector": false,
  "stub_model": true
}
```

Every stage is observable in one payload:

- `sanitized_prompt` — exactly what the model saw (tokens in place)
- `response` — the model output with originals restored
- `tier` / `routing_reason` — the model-router decision
- `phi_detected` / `entity_kinds` — detection accounting (kinds, never values)

## Route preview

```bash
curl -X POST localhost:8005/api/v1/gateway/route \
  -H 'content-type: application/json' \
  -d '{"prompt":"Summarize this patient lab results"}'
```

```json
{ "tier": "complex", "model": "qwen3:14b", "reason": "clinical context or long prompt" }
```

## Errors

| Status | Meaning |
|--------|---------|
| `400` | empty prompt, hops out of range |
| `502` | detector sidecar unreachable, or model upstream error |

## Audit records

One JSONL line per request at `APP_AUDIT_PATH`:

```json
{ "at": "2026-10-05T16:08:50Z", "request_id": "…", "tier": "complex",
  "model": "qwen3:14b", "entities_detected": 2,
  "entity_kinds": ["PERSON", "MEDICAL_RECORD_NUMBER"],
  "tokens_created": 2, "response_tokens_restored": 2,
  "latency_ms": 4 }
```
