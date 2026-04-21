"""First-party wrapper around the ``qdrant-client`` SDK.

Backs openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
  Requirements:
    - Qdrant client wrapper module
    - CODEBUS_QDRANT_URL resolution has a single source of truth
    - Qdrant connection probe
    - Async Qdrant client lifecycle bound to FastAPI app
    - Idempotent collection provisioning

Backs openspec/changes/module-2-kb-builder-p0/specs/qdrant-client/spec.md
  Requirements:
    - KB-facing vector upsert helper
    - KB-facing vector search helper
    - Hash existence helper for deduplication
    - Idempotent KB payload index provisioning

Per D-027 (Qdrant standalone binary is the primary path; docker compose
is the documented fallback), runtime code funnels every Qdrant SDK call
through this module so that upgrades of the SDK's public API have a
single blast radius and `--healthz` / runtime `/healthz` share one probe.
"""
from __future__ import annotations

import os
from typing import Any, Mapping, Sequence
from urllib.error import URLError
from urllib.request import urlopen

from qdrant_client import AsyncQdrantClient
from qdrant_client.http.exceptions import UnexpectedResponse
from qdrant_client.http.models import (
    Distance,
    FieldCondition,
    Filter,
    MatchAny,
    MatchValue,
    PayloadSchemaType,
    PointStruct,
    VectorParams,
)

from codebus_agent.health import DependencyStatus
from codebus_agent.kb.payload import KBHit, KBPayload

DEFAULT_URL = "http://127.0.0.1:6333"
_ENV_VAR = "CODEBUS_QDRANT_URL"


class QdrantCollectionSchemaError(RuntimeError):
    """Raised when an existing Qdrant collection does not match the requested vector schema.

    Surfaced by :func:`ensure_collection` when it refuses to drop-and-recreate
    on mismatch — see design.md "ensure_collection 不符合時的行為：raise,
    不 auto-migrate".
    """


def resolve_url(override: str | None = None) -> str:
    """Return the Qdrant base URL.

    Precedence: explicit ``override`` → ``CODEBUS_QDRANT_URL`` env → ``DEFAULT_URL``.
    All other sidecar modules and the ``--healthz`` CLI call this helper
    rather than reading the environment variable themselves.
    """
    return override or os.environ.get(_ENV_VAR) or DEFAULT_URL


def probe(url: str, timeout_seconds: float = 1.0) -> DependencyStatus:
    """Single-shot ``GET /readyz`` probe — never raises, never leaks exception message.

    Backs spec「Qdrant connection probe」：網路錯誤 / timeout / 非 200 都回
    ``ok=False``。detail 僅帶 URL 與 exception type 名（design「Probe 失敗
    不 log 原始 exception」），不帶 ``str(exc)`` — 避免把 host-side 路徑或
    其他 sentinel 洩進 audit log。
    """
    target = f"{url.rstrip('/')}/readyz"
    try:
        with urlopen(target, timeout=timeout_seconds) as resp:
            return DependencyStatus(ok=resp.status == 200, detail=url)
    except (URLError, TimeoutError, ConnectionError, OSError) as exc:
        return DependencyStatus(ok=False, detail=f"{url} ({type(exc).__name__})")


def build_client(url: str) -> AsyncQdrantClient:
    """Construct an ``AsyncQdrantClient`` bound to ``url``.

    Backs design「Client 生命週期：single async client，app state 常駐」.
    Per D-027 the Qdrant binary is the primary path; this wrapper keeps
    SDK construction in one place so an upgrade that renames constructor
    kwargs (e.g. ``url`` → ``location``) has a single blast radius.
    Construction is non-blocking: the SDK opens no TCP connection here,
    which is what lets ``create_app`` stay degraded-but-alive when
    Qdrant is down.
    """
    return AsyncQdrantClient(url=url)


def _distance_str(value: object) -> str:
    """Coerce a ``Distance`` enum or string to its string name.

    Qdrant's SDK may return ``Distance.COSINE`` (enum) or the raw
    ``"Cosine"`` string depending on version; both must compare equal
    against the caller-supplied ``distance`` argument.
    """
    return getattr(value, "value", value) if not isinstance(value, str) else value


async def ensure_collection(
    client: AsyncQdrantClient,
    name: str,
    vector_size: int,
    distance: str = "Cosine",
) -> None:
    """Idempotently ensure a Qdrant collection with the requested vector config.

    Backs spec「Idempotent collection provisioning」+ design「`ensure_collection`
    不符合時的行為：raise，不 auto-migrate」—when an existing collection's
    vector size or distance differs, raise :class:`QdrantCollectionSchemaError`
    rather than drop-and-recreate, so callers (e.g. Module 2 rebuild flow)
    decide whether to destroy data.

    Design「`ensure_collection` 不做 payload index」— payload indices are
    Module 2 territory and not provisioned here.
    """
    try:
        info = await client.get_collection(collection_name=name)
    except (UnexpectedResponse, ValueError):
        await client.create_collection(
            collection_name=name,
            vectors_config=VectorParams(size=vector_size, distance=Distance(distance)),
        )
        return

    vectors = info.config.params.vectors
    existing_size = getattr(vectors, "size", None)
    existing_distance = _distance_str(getattr(vectors, "distance", None))
    if existing_size != vector_size or existing_distance != distance:
        raise QdrantCollectionSchemaError(
            f"collection {name!r} exists with vector "
            f"size={existing_size} distance={existing_distance}; "
            f"requested size={vector_size} distance={distance}"
        )


# ---------------------------------------------------------------------------
# KB-facing helpers (Module 2)
# ---------------------------------------------------------------------------


def _build_filter(query_filter: Mapping[str, Any] | None) -> Filter | None:
    """Translate a flat ``{field: value}`` mapping into a Qdrant `Filter`.

    Equality on string-or-scalar values becomes a `MatchValue`; a list value
    becomes a `MatchAny` (membership), which is what the spec requires for
    `related_stations` filtering. The mapping shape is intentionally minimal
    — KnowledgeBase translates `filter_path` / `filter_source_kind` /
    `related_stations` into this dict before delegating.
    """
    if not query_filter:
        return None
    must: list[FieldCondition] = []
    for field, value in query_filter.items():
        if isinstance(value, (list, tuple, set)):
            must.append(FieldCondition(key=field, match=MatchAny(any=list(value))))
        else:
            must.append(FieldCondition(key=field, match=MatchValue(value=value)))
    return Filter(must=must)


async def upsert_points(
    client: AsyncQdrantClient,
    collection: str,
    points: Sequence[Mapping[str, Any]],
) -> None:
    """Write a batch of ``{id, vector, payload}`` points to ``collection``.

    Each ``payload`` is a `KBPayload` (or a raw dict already shaped like one).
    `KBPayload` is dumped via ``model_dump(mode="json")`` so ``datetime``
    fields land in Qdrant as ISO-8601 strings (spec scenario "Payload
    datetimes serialized as ISO-8601").
    """
    if not points:
        return
    structs: list[PointStruct] = []
    for p in points:
        payload = p["payload"]
        if isinstance(payload, KBPayload):
            payload_dict = payload.model_dump(mode="json")
        else:
            payload_dict = dict(payload)
        structs.append(
            PointStruct(id=p["id"], vector=list(p["vector"]), payload=payload_dict)
        )
    await client.upsert(collection_name=collection, points=structs)


async def search_points(
    client: AsyncQdrantClient,
    collection: str,
    vector: Sequence[float],
    *,
    limit: int,
    query_filter: Mapping[str, Any] | None = None,
) -> list[KBHit]:
    """kNN search returning hits with deserialized `KBPayload`.

    Returns ``[]`` when the collection is empty or absent (spec scenario
    "Empty collection returns empty list"). The wrapper surfaces only
    `KBHit`, never the raw SDK ScoredPoint, so callers don't have to know
    that ``ScoredPoint.id`` may be UUID / int / str depending on backend.
    """
    try:
        response = await client.query_points(
            collection_name=collection,
            query=list(vector),
            limit=limit,
            with_payload=True,
            query_filter=_build_filter(query_filter),
        )
    except (UnexpectedResponse, ValueError):
        return []
    hits: list[KBHit] = []
    for point in response.points:
        if point.payload is None:
            continue
        hits.append(
            KBHit(
                point_id=str(point.id),
                score=float(point.score),
                payload=KBPayload.model_validate(point.payload),
            )
        )
    return hits


async def exists_by_hash(
    client: AsyncQdrantClient,
    collection: str,
    text_hash: str,
) -> bool:
    """Return True iff at least one point's payload `text_hash == text_hash`.

    A missing collection MUST return ``False`` rather than raise (spec
    scenario "Missing collection reports False, not exception"). We use
    ``count`` with an exact filter rather than ``scroll``: count returns
    only an integer so it's strictly cheaper than fetching a payload.
    """
    flt = Filter(
        must=[FieldCondition(key="text_hash", match=MatchValue(value=text_hash))]
    )
    try:
        result = await client.count(
            collection_name=collection,
            count_filter=flt,
            exact=True,
        )
    except (UnexpectedResponse, ValueError):
        return False
    return getattr(result, "count", 0) > 0


async def ensure_kb_payload_indices(
    client: AsyncQdrantClient,
    collection: str,
) -> None:
    """Create keyword payload indices for ``text_hash`` and ``related_stations``.

    Idempotent — repeated invocations MUST NOT raise (spec scenario "Repeated
    invocation no-op"). The SDK raises `UnexpectedResponse` (HTTP 4xx) when
    a payload index already exists for a field; we swallow that since the
    "create or already exists" outcome is the contract.
    """
    for field in ("text_hash", "related_stations"):
        try:
            await client.create_payload_index(
                collection_name=collection,
                field_name=field,
                field_schema=PayloadSchemaType.KEYWORD,
            )
        except (UnexpectedResponse, ValueError):
            # Index already exists — idempotent no-op.
            continue
