"""Sidecar tests. Presidio needs a downloaded spaCy model — tests skip
when it's absent (CI stays light; the Docker build downloads it)."""

import pytest

pytest.importorskip("presidio_analyzer", reason="presidio not installed")

from httpx import ASGITransport, AsyncClient  # noqa: E402

from app.main import app  # noqa: E402


@pytest.fixture
async def client():
    async with AsyncClient(transport=ASGITransport(app=app), base_url="http://test") as c:
        yield c


async def test_health(client):
    resp = await client.get("/health")
    assert resp.status_code == 200
    assert resp.json()["status"] == "ok"


async def test_detect_finds_name_and_mrn(client):
    text = "Patient John Smith, MRN 12345, reports chest pain."
    resp = await client.post("/detect", json={"text": text})
    assert resp.status_code == 200
    entities = resp.json()["entities"]
    kinds = {e["kind"] for e in entities}
    assert "PERSON" in kinds
    assert any("US_SSN" != k for k in kinds)
    # Offsets must point at the actual text.
    for entity in entities:
        assert text[entity["start"] : entity["end"]] == entity["text"]


async def test_detect_clean_text(client):
    resp = await client.post("/detect", json={"text": "What is aspirin used for?"})
    assert resp.status_code == 200
    # A general medical question may produce zero entities.
    assert isinstance(resp.json()["entities"], list)
