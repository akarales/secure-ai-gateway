"""Presidio PHI detection sidecar.

The Secure AI Gateway (Rust) calls POST /detect with a prompt; this
service returns the PHI entities Presidio finds. Deliberately tiny —
detection quality comes from Presidio's recognizer registry plus a
medical-context deny-list, not from application code.

Note: the analyzer needs a spaCy model. Install once per environment:
    uv run python -m spacy download en_core_web_sm
(en_core_web_lg is more accurate if you have the disk space.)
"""

from contextlib import asynccontextmanager
from functools import lru_cache
from typing import Any

from fastapi import FastAPI
from pydantic import BaseModel, Field


class DetectRequest(BaseModel):
    text: str = Field(min_length=1, max_length=100_000)


class Entity(BaseModel):
    kind: str
    text: str
    start: int
    end: int
    score: float


class DetectResponse(BaseModel):
    entities: list[Entity]


@lru_cache(maxsize=1)
def analyzer() -> Any:
    from presidio_analyzer import AnalyzerEngine

    return AnalyzerEngine()


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Warm the model at startup so the first request isn't slow.
    analyzer()
    yield


app = FastAPI(title="PHI Detector", version="0.1.0", lifespan=lifespan)


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok"}


@app.post("/detect", response_model=DetectResponse)
def detect(request: DetectRequest) -> DetectResponse:
    results = analyzer().analyze(text=request.text, language="en")
    return DetectResponse(
        entities=[
            Entity(
                kind=result.entity_type,
                text=request.text[result.start : result.end],
                start=result.start,
                end=result.end,
                score=float(result.score),
            )
            for result in results
        ]
    )
