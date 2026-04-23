"""Shared fixtures for KB tests.

The `InMemoryQdrantBackend` and `SpyProvider` classes implement the
runtime Protocols (`KBQdrantBackend`, `LLMProvider`) so KnowledgeBase
tests can verify pipeline semantics without a live Qdrant or a real
LLM call. See design `Qdrant 離線測試策略`.
"""
from __future__ import annotations

import asyncio
import hashlib
import math
from dataclasses import dataclass, field
from typing import Any, Mapping, Sequence

import pytest
from pydantic import BaseModel

from codebus_agent.kb.payload import KBHit, KBPayload
from codebus_agent.providers.protocol import EmbedResponse, Message, Usage


EMBEDDING_DIM = 8


# ---------------------------------------------------------------------------
# In-memory Qdrant backend
# ---------------------------------------------------------------------------


def _cosine(a: Sequence[float], b: Sequence[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(y * y for y in b))
    if na == 0.0 or nb == 0.0:
        return 0.0
    return dot / (na * nb)


@dataclass
class _StoredPoint:
    point_id: str
    vector: list[float]
    payload: dict[str, Any]


class InMemoryQdrantBackend:
    """Dict-backed `KBQdrantBackend` for offline KnowledgeBase tests.

    Cosine similarity is computed in pure Python; the backend is small
    enough that we don't need numpy. Filter handling mirrors the live
    wrapper: equality scalars and list-membership for `related_stations`.

    ``kb-build-production-wiring`` adds ``ensure_collection(expected_dim)``:
    tracks the dim first recorded per collection and raises
    ``KBDimMismatchError`` when a later caller requests a different dim
    (mirrors the live ``QdrantHttpBackend`` contract).
    """

    def __init__(self) -> None:
        self._collections: dict[str, list[_StoredPoint]] = {}
        self._dims: dict[str, int] = {}

    async def ensure_collection(
        self, collection: str, *, expected_dim: int
    ) -> None:
        from codebus_agent.kb.backend import KBDimMismatchError

        known = self._dims.get(collection)
        if known is None:
            self._dims[collection] = expected_dim
            self._collections.setdefault(collection, [])
            return
        if known != expected_dim:
            raise KBDimMismatchError(
                collection=collection,
                expected_dim=expected_dim,
                actual_dim=known,
            )

    async def ensure_indices(self, collection: str) -> None:
        self._collections.setdefault(collection, [])

    async def upsert_points(
        self,
        collection: str,
        points: Sequence[Mapping[str, Any]],
    ) -> None:
        bucket = self._collections.setdefault(collection, [])
        for p in points:
            payload_obj = p["payload"]
            if isinstance(payload_obj, KBPayload):
                payload_dict = payload_obj.model_dump(mode="json")
            else:
                payload_dict = dict(payload_obj)
            bucket.append(
                _StoredPoint(
                    point_id=str(p["id"]),
                    vector=list(p["vector"]),
                    payload=payload_dict,
                )
            )

    async def search_points(
        self,
        collection: str,
        vector: Sequence[float],
        *,
        limit: int,
        query_filter: Mapping[str, Any] | None = None,
    ) -> list[KBHit]:
        bucket = self._collections.get(collection, [])
        if not bucket:
            return []
        candidates = [pt for pt in bucket if _matches(pt.payload, query_filter)]
        scored = [
            (pt, _cosine(vector, pt.vector)) for pt in candidates
        ]
        scored.sort(key=lambda pair: pair[1], reverse=True)
        return [
            KBHit(
                point_id=pt.point_id,
                score=float(score),
                payload=KBPayload.model_validate(pt.payload),
            )
            for pt, score in scored[:limit]
        ]

    async def exists_by_hash(self, collection: str, text_hash: str) -> bool:
        bucket = self._collections.get(collection)
        if not bucket:
            return False
        return any(pt.payload.get("text_hash") == text_hash for pt in bucket)

    async def drop_collection(self, collection: str) -> None:
        self._collections.pop(collection, None)


def _matches(payload: dict[str, Any], query_filter: Mapping[str, Any] | None) -> bool:
    if not query_filter:
        return True
    for field_name, expected in query_filter.items():
        actual = payload.get(field_name)
        if isinstance(expected, (list, tuple, set)):
            if isinstance(actual, list):
                if not any(item in expected for item in actual):
                    return False
            else:
                if actual not in expected:
                    return False
        else:
            if actual != expected:
                return False
    return True


# ---------------------------------------------------------------------------
# Spy LLM provider (deterministic embeddings + call accounting)
# ---------------------------------------------------------------------------


@dataclass
class _EmbedCall:
    texts: list[str]
    vectors: list[list[float]]


@dataclass
class SpyProvider:
    """`LLMProvider`-conforming spy with deterministic, hash-keyed vectors.

    Each input text deterministically maps to a unit-ish length-`EMBEDDING_DIM`
    vector seeded by sha256(text)[:16]. Two identical texts MUST produce
    identical vectors so cosine search returns score 1.0 on exact matches.
    """

    name: str = "spy"
    model: str = "spy-embed-v1"
    embed_token_per_text: int = 5
    embed_calls: list[_EmbedCall] = field(default_factory=list)
    inflight_lock: asyncio.Event | None = None

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        raise NotImplementedError("SpyProvider only supports embed for KB tests")

    async def embed(self, texts: list[str]) -> EmbedResponse:
        vectors = [_deterministic_vector(t) for t in texts]
        self.embed_calls.append(_EmbedCall(texts=list(texts), vectors=vectors))
        if self.inflight_lock is not None:
            # Block until released by the test — used to verify in-flight cap.
            await self.inflight_lock.wait()
        usage = Usage(
            call_type="embed",
            model=self.model,
            embed_tokens=self.embed_token_per_text * len(texts),
            cost_usd=0.0,
        )
        return EmbedResponse(vectors=vectors, usage=usage)


def _deterministic_vector(text: str) -> list[float]:
    """Hash-seeded length-8 unit vector; identical texts → identical vectors."""
    digest = hashlib.sha256(text.encode("utf-8")).digest()
    raw = [digest[i] - 128 for i in range(EMBEDDING_DIM)]
    norm = math.sqrt(sum(v * v for v in raw)) or 1.0
    return [v / norm for v in raw]


# ---------------------------------------------------------------------------
# Convenience pytest fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def in_memory_backend() -> InMemoryQdrantBackend:
    return InMemoryQdrantBackend()


@pytest.fixture
def spy_provider() -> SpyProvider:
    return SpyProvider()
