"""Tests for `KnowledgeBase.upsert_chunk` two-layer dedup.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/knowledge-base/spec.md
  Requirement: KnowledgeBase exposes upsert_chunk for Q&A add_to_kb path
"""
from __future__ import annotations

import asyncio
import hashlib
import uuid
from datetime import datetime, timezone
from pathlib import Path

import pytest

from codebus_agent.kb.knowledge_base import KnowledgeBase
from codebus_agent.kb.payload import KBHit, KBPayload
from codebus_agent.providers.usage_tracker import UsageTracker

from .conftest import EMBEDDING_DIM, InMemoryQdrantBackend, SpyProvider


def _make_kb(backend, provider, tmp_path: Path) -> KnowledgeBase:
    tracker = UsageTracker(tmp_path / "tracker.jsonl")
    return KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root="/abs/ws/test_upsert_chunk",
        embedding_dim=EMBEDDING_DIM,
    )


def _payload(text: str) -> KBPayload:
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
        related_stations=["s02-storage"],
    )


@pytest.mark.anyio("asyncio")
async def test_hash_dedup_short_circuits(tmp_path: Path, monkeypatch) -> None:
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)

    # Seed an exact-text point so Layer 1 hash dedup hits.
    await backend.ensure_collection(kb.collection_name, expected_dim=EMBEDDING_DIM)
    seed_payload = _payload("hello")
    seed_resp = await provider.embed(["hello"])
    await backend.upsert_points(
        kb.collection_name,
        [{"id": str(uuid.uuid4()), "vector": seed_resp.vectors[0], "payload": seed_payload}],
    )

    embed_calls: list[list[str]] = []
    upsert_calls: list[str] = []

    async def _spy_embed(texts):
        embed_calls.append(list(texts))
        return await SpyProvider().embed(texts)

    async def _spy_upsert(collection, points):
        upsert_calls.append("called")

    monkeypatch.setattr(provider, "embed", _spy_embed)
    monkeypatch.setattr(backend, "upsert_points", _spy_upsert)

    result = await kb.upsert_chunk("hello", payload=_payload("hello"))
    assert result == "dedup:hash"
    assert embed_calls == []
    assert upsert_calls == []


@pytest.mark.anyio("asyncio")
async def test_similarity_dedup_after_embed(tmp_path: Path, monkeypatch) -> None:
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)

    await backend.ensure_collection(kb.collection_name, expected_dim=EMBEDDING_DIM)

    # Force find_similar to return a high-score hit; novel hash so Layer 1 misses.
    fake_payload = _payload("existing-text")
    fake_hit = KBHit(
        point_id="existing-pt-01",
        score=0.97,
        payload=fake_payload,
    )

    async def _fake_find_similar(text: str, *, threshold: float = 0.95) -> KBHit | None:
        # Score 0.97 ≥ threshold 0.95 → similarity dedup must trigger.
        return fake_hit

    monkeypatch.setattr(kb, "find_similar", _fake_find_similar)

    upsert_calls: list[str] = []

    async def _spy_upsert(collection, points):
        upsert_calls.append("called")

    monkeypatch.setattr(backend, "upsert_points", _spy_upsert)

    embed_calls: list[list[str]] = []
    real_embed = provider.embed

    async def _counting_embed(texts):
        embed_calls.append(list(texts))
        return await real_embed(texts)

    monkeypatch.setattr(provider, "embed", _counting_embed)

    result = await kb.upsert_chunk("hello rephrased", payload=_payload("hello rephrased"))
    assert result == "dedup:sim"
    # Layer 1 missed → embed called exactly once for the dedup probe.
    assert len(embed_calls) == 1
    assert upsert_calls == []


@pytest.mark.anyio("asyncio")
async def test_new_chunk_returns_point_id(tmp_path: Path, monkeypatch) -> None:
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)
    await backend.ensure_collection(kb.collection_name, expected_dim=EMBEDDING_DIM)

    upsert_calls: list[list[dict]] = []
    real_upsert = backend.upsert_points

    async def _capture_upsert(collection, points):
        upsert_calls.append(list(points))
        await real_upsert(collection, points)

    monkeypatch.setattr(backend, "upsert_points", _capture_upsert)

    result = await kb.upsert_chunk("totally novel text", payload=_payload("totally novel text"))
    assert isinstance(result, str)
    assert result != ""
    assert not result.startswith("dedup:")
    assert len(upsert_calls) == 1
    assert upsert_calls[0][0]["id"] == result


@pytest.mark.anyio("asyncio")
async def test_dedup_token_format_reserved(tmp_path: Path, monkeypatch) -> None:
    """All `"dedup:"`-prefixed return values MUST be drawn from a closed set."""
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    kb = _make_kb(backend, provider, tmp_path)
    await backend.ensure_collection(kb.collection_name, expected_dim=EMBEDDING_DIM)

    # 1) Layer 1 hit
    seed_payload = _payload("hash-me")
    seed_resp = await provider.embed(["hash-me"])
    await backend.upsert_points(
        kb.collection_name,
        [{"id": str(uuid.uuid4()), "vector": seed_resp.vectors[0], "payload": seed_payload}],
    )
    layer_1 = await kb.upsert_chunk("hash-me", payload=_payload("hash-me"))

    # 2) Layer 2 hit (force find_similar to return a high-score hit)
    fake_hit = KBHit(point_id="x", score=0.99, payload=_payload("x"))

    async def _force_layer2(text: str, *, threshold: float = 0.95) -> KBHit | None:
        return fake_hit

    monkeypatch.setattr(kb, "find_similar", _force_layer2)
    layer_2 = await kb.upsert_chunk("a fresh text", payload=_payload("a fresh text"))

    for token in (layer_1, layer_2):
        assert token in {"dedup:hash", "dedup:sim"}, token
