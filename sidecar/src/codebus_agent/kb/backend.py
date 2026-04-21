"""KBQdrantBackend Protocol for offline-testable storage abstraction.

Backs the design decision in
openspec/changes/module-2-kb-builder-p0/design.md `Qdrant 離線測試策略`:
the runtime path goes through `codebus_agent.kb.qdrant_client` against
the real `AsyncQdrantClient`, but tests can substitute an in-memory
implementation conforming to this Protocol so the KB pipeline stays
verifiable without a live Qdrant.

The runtime adapter `QdrantHttpBackend` delegates every method to the
existing module-level helpers in `qdrant_client` so callers see one
shape regardless of where the backing store lives.
"""
from __future__ import annotations

from typing import Any, Mapping, Protocol, Sequence, runtime_checkable

from qdrant_client import AsyncQdrantClient

from codebus_agent.kb import qdrant_client as _qc
from codebus_agent.kb.payload import KBHit


@runtime_checkable
class KBQdrantBackend(Protocol):
    """Storage Protocol consumed by `KnowledgeBase`.

    Methods mirror the KB-facing helpers in `codebus_agent.kb.qdrant_client`
    so `QdrantHttpBackend` is a thin pass-through and `InMemoryQdrantBackend`
    (declared in `tests/kb/conftest.py`) is an offline drop-in.
    """

    async def ensure_indices(self, collection: str) -> None: ...

    async def upsert_points(
        self,
        collection: str,
        points: Sequence[Mapping[str, Any]],
    ) -> None: ...

    async def search_points(
        self,
        collection: str,
        vector: Sequence[float],
        *,
        limit: int,
        query_filter: Mapping[str, Any] | None = None,
    ) -> list[KBHit]: ...

    async def exists_by_hash(self, collection: str, text_hash: str) -> bool: ...

    async def drop_collection(self, collection: str) -> None: ...


class QdrantHttpBackend:
    """Runtime adapter wrapping `AsyncQdrantClient` via `qdrant_client` helpers.

    Construct with the live client; the adapter forwards every Protocol
    method to the module-level helper, so swapping in a different backend
    in tests is purely a constructor-injection concern at the call site
    (see `KnowledgeBase.__init__`).
    """

    def __init__(self, client: AsyncQdrantClient) -> None:
        self._client = client

    async def ensure_indices(self, collection: str) -> None:
        await _qc.ensure_kb_payload_indices(self._client, collection)

    async def upsert_points(
        self,
        collection: str,
        points: Sequence[Mapping[str, Any]],
    ) -> None:
        await _qc.upsert_points(self._client, collection, points)

    async def search_points(
        self,
        collection: str,
        vector: Sequence[float],
        *,
        limit: int,
        query_filter: Mapping[str, Any] | None = None,
    ) -> list[KBHit]:
        return await _qc.search_points(
            self._client,
            collection,
            vector,
            limit=limit,
            query_filter=query_filter,
        )

    async def exists_by_hash(self, collection: str, text_hash: str) -> bool:
        return await _qc.exists_by_hash(self._client, collection, text_hash)

    async def drop_collection(self, collection: str) -> None:
        try:
            await self._client.delete_collection(collection_name=collection)
        except Exception:
            # Drop is best-effort; the contract is "afterwards the collection is gone".
            pass


__all__ = ["KBQdrantBackend", "QdrantHttpBackend"]
