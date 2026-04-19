"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/qdrant-client/spec.md
  Requirement: qdrant-client connectivity smoke test

The smoke test is skipped automatically if no Qdrant is reachable at
CODEBUS_QDRANT_URL (default http://127.0.0.1:6333).  Run it manually
with either launch path:

    # Binary path (D-027 primary)
    bash sidecar/scripts/start-qdrant.sh &
    uv run pytest tests/qdrant/test_smoke.py -v

    # Docker fallback
    docker compose -f sidecar/docker-compose.qdrant.yml up -d
    uv run pytest tests/qdrant/test_smoke.py -v
"""
from __future__ import annotations

import os
import uuid
from urllib.error import URLError
from urllib.request import urlopen

import pytest

DEFAULT_URL = "http://127.0.0.1:6333"
COLLECTION = "m1-smoke"
VECTOR_SIZE = 8


def _qdrant_url() -> str:
    return os.environ.get("CODEBUS_QDRANT_URL", DEFAULT_URL)


def _reachable(url: str) -> bool:
    try:
        with urlopen(f"{url.rstrip('/')}/readyz", timeout=1.0) as resp:
            return resp.status == 200
    except (URLError, TimeoutError, ConnectionError, OSError):
        return False


pytestmark = pytest.mark.skipif(
    not _reachable(_qdrant_url()),
    reason=f"Qdrant not reachable at {_qdrant_url()} — start via sidecar/scripts/start-qdrant.{{sh,ps1}} or docker compose",
)


@pytest.fixture
def client():
    from qdrant_client import QdrantClient

    c = QdrantClient(url=_qdrant_url())
    try:
        c.delete_collection(COLLECTION)
    except Exception:
        pass
    yield c
    try:
        c.delete_collection(COLLECTION)
    except Exception:
        pass


@pytest.mark.qdrant
def test_smoke_create_upsert_search(client) -> None:
    """Scenario: Smoke test creates / upserts / searches / cleans up."""
    from qdrant_client.models import Distance, PointStruct, VectorParams

    client.create_collection(
        collection_name=COLLECTION,
        vectors_config=VectorParams(size=VECTOR_SIZE, distance=Distance.COSINE),
    )

    point_id = str(uuid.uuid4())
    vector = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
    payload = {"source": "m1-smoke", "tag": "power-on"}

    client.upsert(
        collection_name=COLLECTION,
        points=[PointStruct(id=point_id, vector=vector, payload=payload)],
    )

    hits = client.search(
        collection_name=COLLECTION,
        query_vector=vector,
        limit=1,
    )
    assert len(hits) == 1
    assert str(hits[0].id) == point_id
    assert hits[0].payload == payload


@pytest.mark.qdrant
def test_smoke_respects_env_url() -> None:
    """Scenario: Smoke test respects CODEBUS_QDRANT_URL.

    The smoke test module reads CODEBUS_QDRANT_URL at import time; this
    case simply asserts the hook is present by verifying `_qdrant_url()`
    reflects the env var when set.
    """
    sentinel = "http://qdrant-test.invalid:9999"
    original = os.environ.get("CODEBUS_QDRANT_URL")
    os.environ["CODEBUS_QDRANT_URL"] = sentinel
    try:
        assert _qdrant_url() == sentinel
    finally:
        if original is None:
            os.environ.pop("CODEBUS_QDRANT_URL", None)
        else:
            os.environ["CODEBUS_QDRANT_URL"] = original
