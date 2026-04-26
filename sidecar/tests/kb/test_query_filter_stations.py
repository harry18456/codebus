"""Tests for `KnowledgeBase.query` `filter_stations` parameter.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/knowledge-base/spec.md
  Requirement: KnowledgeBase query and find_similar API (MODIFIED — filter_stations)
"""
from __future__ import annotations

import asyncio
import hashlib
import uuid
from datetime import datetime, timezone
from pathlib import Path

import pytest

from codebus_agent.kb.knowledge_base import KnowledgeBase
from codebus_agent.kb.payload import KBPayload
from codebus_agent.providers.usage_tracker import UsageTracker

from .conftest import EMBEDDING_DIM, InMemoryQdrantBackend, SpyProvider


def _make_kb(backend, provider, tmp_path: Path) -> KnowledgeBase:
    tracker = UsageTracker(tmp_path / "tracker.jsonl")
    return KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root="/abs/ws/test_filter_stations",
        embedding_dim=EMBEDDING_DIM,
    )


def _payload(text: str, related_stations: list[str]) -> KBPayload:
    return KBPayload(
        source_kind="code",
        file_path="src/x.py",
        line_start=1,
        line_end=5,
        text=text,
        text_hash=hashlib.sha256(text.encode("utf-8")).hexdigest(),
        added_by="qa_agent",
        chunk_index=0,
        chunk_total=1,
        created_at=datetime.now(timezone.utc),
        related_stations=related_stations,
    )


async def _seed(kb, backend, provider, texts_and_stations):
    """Push points directly via the in-memory backend (no embed cost)."""
    await backend.ensure_collection(kb.collection_name, expected_dim=EMBEDDING_DIM)
    points = []
    for text, stations in texts_and_stations:
        # Use the spy provider's deterministic embed so each text gets a
        # consistent vector across this seed and downstream queries.
        resp = await provider.embed([text])
        vec = resp.vectors[0]
        points.append(
            {
                "id": str(uuid.uuid4()),
                "vector": vec,
                "payload": _payload(text, stations),
            }
        )
    await backend.upsert_points(kb.collection_name, points)


@pytest.mark.anyio("asyncio")
async def test_filter_stations_or_semantics(tmp_path: Path) -> None:
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)
    await _seed(
        kb,
        backend,
        provider,
        [
            ("alpha", ["s02-storage"]),
            ("beta", ["s03-payment"]),
            ("gamma", []),
        ],
    )

    hits = await kb.query("query", filter_stations=["s02-storage", "s03-payment"])
    texts = {h.payload.text for h in hits}
    # Chunks A + B match the OR filter; C with empty related_stations does not.
    assert "alpha" in texts
    assert "beta" in texts
    assert "gamma" not in texts


@pytest.mark.anyio("asyncio")
async def test_empty_filter_stations_equivalent_to_none(tmp_path: Path) -> None:
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)
    await _seed(
        kb,
        backend,
        provider,
        [
            ("alpha", ["s02-storage"]),
            ("beta", ["s03-payment"]),
            ("gamma", []),
        ],
    )

    none_hits = await kb.query("query")
    empty_hits = await kb.query("query", filter_stations=[])
    assert [h.point_id for h in none_hits] == [h.point_id for h in empty_hits]


@pytest.mark.anyio("asyncio")
async def test_invalid_station_id_raises_pre_call(tmp_path: Path, monkeypatch) -> None:
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)

    embed_calls: list[list[str]] = []
    search_calls: list[str] = []

    async def _spy_embed(texts):
        embed_calls.append(list(texts))
        return await SpyProvider().embed(texts)

    async def _spy_search(*args, **kwargs):
        search_calls.append("called")
        return []

    monkeypatch.setattr(provider, "embed", _spy_embed)
    monkeypatch.setattr(backend, "search_points", _spy_search)

    with pytest.raises(ValueError) as excinfo:
        await kb.query("query", filter_stations=["bad-id"])
    assert "bad-id" in str(excinfo.value)
    # No embedding API call, no Qdrant search.
    assert embed_calls == []
    assert search_calls == []
