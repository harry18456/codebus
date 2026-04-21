"""KB-facing Qdrant wrapper tests (auto-skip if Qdrant unreachable).

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/qdrant-client/spec.md
  Requirement: KB-facing vector upsert helper
  Requirement: KB-facing vector search helper
  Requirement: Hash existence helper for deduplication
  Requirement: Idempotent KB payload index provisioning

Per pyproject.toml marker `qdrant`: tests are auto-skipped when Qdrant
is not reachable at CODEBUS_QDRANT_URL (default http://127.0.0.1:6333).
Run manually with `bash sidecar/scripts/start-qdrant.sh &` first.
"""
from __future__ import annotations

import hashlib
import os
import uuid
from datetime import datetime, timezone
from urllib.error import URLError
from urllib.request import urlopen

import pytest
from qdrant_client import AsyncQdrantClient

from codebus_agent.kb import qdrant_client as qc
from codebus_agent.kb.payload import KBPayload

DEFAULT_URL = "http://127.0.0.1:6333"
VECTOR_SIZE = 8


def _qdrant_url() -> str:
    return os.environ.get("CODEBUS_QDRANT_URL", DEFAULT_URL)


def _reachable(url: str) -> bool:
    try:
        with urlopen(f"{url.rstrip('/')}/readyz", timeout=1.0) as resp:
            return resp.status == 200
    except (URLError, TimeoutError, ConnectionError, OSError):
        return False


pytestmark = [
    pytest.mark.qdrant,
    pytest.mark.skipif(
        not _reachable(_qdrant_url()),
        reason=(
            f"Qdrant not reachable at {_qdrant_url()} — start via "
            "sidecar/scripts/start-qdrant.{sh,ps1} or docker compose"
        ),
    ),
]


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _payload(
    text: str = "alpha\n",
    *,
    file_path: str = "src/x.ts",
    source_kind: str = "code",
    related_stations: list[str] | None = None,
) -> KBPayload:
    return KBPayload(
        source_kind=source_kind,
        file_path=file_path,
        line_start=1,
        line_end=1,
        commit_oid=None,
        text=text,
        text_hash=_sha256(text.strip()),
        language="typescript",
        added_by="scanner",
        session_id=None,
        chunk_index=0,
        chunk_total=1,
        created_at=datetime(2026, 4, 21, 12, 0, 0, tzinfo=timezone.utc),
        source_mtime=None,
        sanitize_stats={},
        related_stations=related_stations or [],
    )


def _vec(seed: int) -> list[float]:
    """Deterministic length-8 unit-ish vector keyed by `seed`."""
    base = [(seed + i) * 0.013 for i in range(VECTOR_SIZE)]
    norm = sum(v * v for v in base) ** 0.5
    return [v / norm for v in base]


@pytest.fixture
async def fresh_collection():
    """Create a unique collection for the test, drop on teardown."""
    name = f"codebus_kbtest_{uuid.uuid4().hex[:12]}"
    client = AsyncQdrantClient(url=_qdrant_url())
    await qc.ensure_collection(client, name=name, vector_size=VECTOR_SIZE)
    try:
        yield client, name
    finally:
        try:
            await client.delete_collection(collection_name=name)
        except Exception:
            pass
        await client.close()


# -- Requirement: KB-facing vector upsert helper ----------------------------


async def test_upsert_points_writes_and_search_returns_ids(fresh_collection) -> None:
    """Scenario: Upsert writes all points to the named collection."""
    client, collection = fresh_collection
    points = [
        {
            "id": str(uuid.uuid4()),
            "vector": _vec(seed),
            "payload": _payload(text=f"chunk-{seed}\n"),
        }
        for seed in (1, 2, 3)
    ]

    await qc.upsert_points(client, collection, points)

    for p in points:
        hits = await qc.search_points(client, collection, p["vector"], limit=1)
        assert len(hits) == 1
        assert hits[0].point_id == p["id"]


async def test_upsert_points_serializes_datetime_as_iso8601(fresh_collection) -> None:
    """Scenario: Payload datetimes serialized as ISO-8601."""
    client, collection = fresh_collection
    created = datetime(2026, 4, 21, 12, 30, 45, tzinfo=timezone.utc)
    payload = KBPayload(
        source_kind="code",
        file_path="src/x.ts",
        line_start=1,
        line_end=1,
        text="hello\n",
        text_hash=_sha256("hello"),
        language="typescript",
        added_by="scanner",
        chunk_index=0,
        chunk_total=1,
        created_at=created,
    )
    pid = str(uuid.uuid4())
    await qc.upsert_points(
        client, collection, [{"id": pid, "vector": _vec(7), "payload": payload}]
    )

    hits = await qc.search_points(client, collection, _vec(7), limit=1)
    assert len(hits) == 1
    raw_created = hits[0].payload.created_at
    # `model_dump(mode="json")` emits ISO-8601; reading it back through
    # Pydantic re-parses to a datetime equal to the original.
    assert raw_created == created


# -- Requirement: KB-facing vector search helper ----------------------------


async def test_search_points_empty_collection_returns_empty_list(
    fresh_collection,
) -> None:
    """Scenario: Empty collection returns empty list."""
    client, collection = fresh_collection
    hits = await qc.search_points(client, collection, _vec(0), limit=10)
    assert hits == []


async def test_search_points_filter_by_file_path(fresh_collection) -> None:
    """Scenario: Filter on file_path restricts results."""
    client, collection = fresh_collection
    await qc.ensure_kb_payload_indices(client, collection)

    a_id, b_id = str(uuid.uuid4()), str(uuid.uuid4())
    await qc.upsert_points(
        client,
        collection,
        [
            {
                "id": a_id,
                "vector": _vec(1),
                "payload": _payload(text="a\n", file_path="src/a.ts"),
            },
            {
                "id": b_id,
                "vector": _vec(2),
                "payload": _payload(text="b\n", file_path="src/b.ts"),
            },
        ],
    )

    hits = await qc.search_points(
        client,
        collection,
        _vec(1),
        limit=10,
        query_filter={"file_path": "src/a.ts"},
    )
    assert len(hits) >= 1
    for h in hits:
        assert h.payload.file_path == "src/a.ts"


# -- Requirement: Hash existence helper for deduplication -------------------


async def test_exists_by_hash_true_false_and_missing_collection(
    fresh_collection,
) -> None:
    """Scenarios: present True / absent False / missing-collection False."""
    client, collection = fresh_collection
    await qc.ensure_kb_payload_indices(client, collection)

    payload = _payload(text="dedup-target\n")
    await qc.upsert_points(
        client,
        collection,
        [{"id": str(uuid.uuid4()), "vector": _vec(5), "payload": payload}],
    )

    assert await qc.exists_by_hash(client, collection, payload.text_hash) is True
    assert await qc.exists_by_hash(client, collection, "deadbeef" * 8) is False
    assert await qc.exists_by_hash(client, "no_such_collection_xyz", "0" * 64) is False


# -- Requirement: Idempotent KB payload index provisioning ------------------


async def test_ensure_kb_payload_indices_idempotent(fresh_collection) -> None:
    """Scenarios: Indices created when absent + Repeated invocation no-op."""
    client, collection = fresh_collection

    await qc.ensure_kb_payload_indices(client, collection)
    # Second call MUST NOT raise.
    await qc.ensure_kb_payload_indices(client, collection)

    info = await client.get_collection(collection_name=collection)
    payload_schema = getattr(info, "payload_schema", None) or {}
    # Schema keys must include both indexed fields after ensure runs.
    assert "text_hash" in payload_schema, (
        f"text_hash index missing; payload_schema keys={list(payload_schema)}"
    )
    assert "related_stations" in payload_schema, (
        f"related_stations index missing; payload_schema keys={list(payload_schema)}"
    )
