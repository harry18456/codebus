"""`POST /kb/build` async endpoint — Module 2 KB Builder over SSE.

Backs openspec/changes/sse-progress-skeleton/specs/knowledge-base/spec.md
  Requirement: POST /kb/build async endpoint
  Requirement: KB progress phase translation to wire schema

關鍵契約：

* 預設 async — `POST /kb/build` SHALL NOT have a synchronous variant.
  The endpoint creates a `kind="kb"` task, spawns the build coroutine,
  and returns ``{"task_id": "kb_<hex8>"}`` immediately.
* Phase translation collapses `chunking` / `embedding` / `upserting` to
  the single wire phase ``"embedding"``; source `done` MUST NOT emit a
  wire `progress` event (the SSE `done` is emitted by the task wrapper).
* The wire stream is monotonic 0 → total; the adapter holds an anchor
  total (chunks_emitted) so embedded / upserting events are scaled to a
  consistent denominator and `current` is clamped non-decreasing.
* KB dependencies (`backend`, `provider`, `usage_tracker`,
  `embedding_dim`) are pulled from ``app.state``; tests inject offline
  doubles, production wiring lives in ``api/__init__.py`` (added when
  the production Qdrant + provider plumbing lands).
"""
from __future__ import annotations

import asyncio
from typing import Any

from fastapi import APIRouter, HTTPException, Request, status
from pydantic import BaseModel, ConfigDict, Field

from codebus_agent.api.tasks import TaskRegistry, _run_background_task
from codebus_agent.kb.knowledge_base import KnowledgeBase
from codebus_agent.kb.payload import KBProgressEvent, ProgressCallback
from codebus_agent.scanner.models import ScanResult

router = APIRouter()


class KBBuildRequest(BaseModel):
    """Request body for ``POST /kb/build``.

    ``scan_result`` is the full ``ScanResult`` JSON returned by a prior
    ``/scan`` invocation. Validating it through Pydantic up-front means
    a malformed body is rejected with 422 before the background task is
    spawned (no leaked task_id for invalid input).
    """

    model_config = ConfigDict(extra="forbid")

    workspace_root: str
    scan_result: ScanResult


class _KBProgressAdapter:
    """Stateful translator: ``KBProgressEvent`` → wire ``progress`` dict.

    Spec invariants (`KB progress phase translation to wire schema`):

    * ``done`` source phase → returns ``None`` (no wire emit; the task
      wrapper emits the SSE ``done``).
    * All non-``done`` phases collapse to wire ``phase: "embedding"``.
    * Wire stream is monotonic non-decreasing in ``current`` and reaches
      the anchor total (set from the chunking event's total) by the end.
    * Anchor total is captured on the first ``chunking`` event so wire
      events use a consistent denominator even if dedup shrinks the
      embedding phase's intrinsic total.
    """

    def __init__(self) -> None:
        self._anchor_total = 0
        self._last_current = 0
        self._anchored = False

    def translate(self, event: KBProgressEvent) -> dict[str, Any] | None:
        if event.phase == "done":
            return None

        if not self._anchored:
            # First non-done event seeds the anchor. Chunking is the
            # natural source (KB emits it first with total == chunks_emitted),
            # but if a build skips it (defensive), we fall back to the
            # first event's total so the stream still has *some* denominator.
            self._anchor_total = max(event.total, 1)
            self._anchored = True
            self._last_current = 0

        if event.phase == "chunking":
            # Spec: at least one event with `current == 0` near the start.
            # KB emits chunking with current == total, so we override to 0.
            self._last_current = 0
            return self._wire(0)

        if event.phase == "embedding":
            if event.total > 0:
                proposed = int(event.current * self._anchor_total / event.total)
            else:
                proposed = 0
            proposed = max(self._last_current, min(self._anchor_total, proposed))
            self._last_current = proposed
            return self._wire(proposed)

        if event.phase == "upserting":
            # Spec: at least one event with `current == total` near the end.
            # Snap to the anchor on every upserting event so the wire
            # always reaches total even if the build short-circuits.
            self._last_current = self._anchor_total
            return self._wire(self._anchor_total)

        return None

    def _wire(self, current: int) -> dict[str, Any]:
        return {
            "type": "progress",
            "phase": "embedding",
            "current": current,
            "total": self._anchor_total,
        }


def _make_kb_progress_adapter(handle) -> ProgressCallback:
    """Return a ``ProgressCallback`` that fans translated events out to
    the SSE subscribers attached to ``handle``.

    A fresh adapter is created per build so monotonicity state never
    leaks between tasks.
    """
    adapter = _KBProgressAdapter()

    async def _on_progress(event: KBProgressEvent) -> None:
        wire = adapter.translate(event)
        if wire is not None:
            handle.emit(wire)

    return _on_progress


def _require_kb_deps(request: Request):
    """Resolve KB dependencies from ``app.state``; 503 if not wired.

    Production wiring (``kb-build-production-wiring`` change, D-032
    decision 3 / A-plan) populates:
      * ``app.state.kb_backend``        — ``QdrantHttpBackend`` instance
      * ``app.state.kb_provider``       — ``Callable[[Path], TrackedProvider]`` factory
      * ``app.state.kb_usage_tracker``  — ``Callable[[Path], UsageTracker]`` factory
      * ``app.state.kb_embedding_dim``  — ``int``

    Tests inject ``lambda _ws: instance`` for the factory slots to reuse
    singleton doubles across builds.
    """
    state = request.app.state
    backend = getattr(state, "kb_backend", None)
    provider_factory = getattr(state, "kb_provider", None)
    tracker_factory = getattr(state, "kb_usage_tracker", None)
    embedding_dim = getattr(state, "kb_embedding_dim", None)
    if (
        backend is None
        or provider_factory is None
        or tracker_factory is None
        or embedding_dim is None
    ):
        missing = [
            name
            for name, val in (
                ("kb_backend", backend),
                ("kb_provider", provider_factory),
                ("kb_usage_tracker", tracker_factory),
                ("kb_embedding_dim", embedding_dim),
            )
            if val is None
        ]
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": "KB_NOT_CONFIGURED",
                "message": "knowledge-base dependencies not initialized on this sidecar",
                "missing": missing,
            },
        )
    return backend, provider_factory, tracker_factory, embedding_dim


@router.post("/kb/build", status_code=status.HTTP_202_ACCEPTED)
async def kb_build_endpoint(
    request: KBBuildRequest, http_request: Request
) -> dict[str, str]:
    """Spawn a background KB build and return ``{"task_id": ...}`` immediately.

    Concurrency: a 409 ``TASK_IN_FLIGHT`` is returned when the registry
    already holds a running task. Errors raised by ``KnowledgeBase.build``
    surface via the ``_run_background_task`` wrapper as a sanitized SSE
    ``error`` event (see ``api/tasks.py`` `Background task error containment`).

    Factory dispatch: ``kb_provider`` and ``kb_usage_tracker`` are
    ``Callable[[Path], ...]`` per D-032 A-plan, so audit logs land under
    the caller's ``workspace_root``. Both factories are invoked here with
    the request's ``workspace_root`` before the background task spawns.
    """
    backend, provider_factory, tracker_factory, embedding_dim = _require_kb_deps(
        http_request
    )

    registry: TaskRegistry = http_request.app.state.tasks
    handle = registry.create("kb")
    if handle is None:
        running = registry.current_running()
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_IN_FLIGHT",
                "running_task_id": running.id if running else None,
            },
        )

    on_progress = _make_kb_progress_adapter(handle)
    from pathlib import Path as _Path
    workspace_path = _Path(request.workspace_root)
    provider = provider_factory(workspace_path)
    tracker = tracker_factory(workspace_path)

    async def _coro_factory() -> dict[str, Any]:
        kb = KnowledgeBase(
            backend=backend,
            provider=provider,
            usage_tracker=tracker,
            workspace_root=request.workspace_root,
            embedding_dim=embedding_dim,
        )
        stats = await kb.build(request.scan_result, on_progress=on_progress)
        return stats.model_dump(mode="json")

    asyncio.create_task(_run_background_task(handle, _coro_factory))
    return {"task_id": handle.id}


# ---------------------------------------------------------------------------
# POST /kb/query — synchronous JSON endpoint (change `kb-query-endpoint`)
# ---------------------------------------------------------------------------


class KBQueryRequest(BaseModel):
    """Request body for ``POST /kb/query``.

    Backs openspec/changes/kb-query-endpoint/specs/knowledge-base/spec.md
      Requirement: POST /kb/query endpoint

    `top_k` capped at 50 per the proposal Non-Goal: avoid accidental
    large queries; typical RAG use is 8-16. `extra="forbid"` so callers
    don't accidentally smuggle unsupported filter knobs.
    """

    model_config = ConfigDict(extra="forbid")

    workspace_root: str
    text: str = Field(min_length=1)
    top_k: int = Field(default=8, ge=1, le=50)
    filter_path: str | None = None
    filter_source_kind: list[str] | None = None


def _require_query_deps(request: Request):
    """Resolve the query path's dependencies; 503 KB_NOT_CONFIGURED on miss.

    Reads ``app.state.kb_query_provider`` (the ``default_module="kb_query"``
    factory wired by ``wire_kb_dependencies``) — distinct from
    ``kb_provider`` so cost accounting can split build vs query in
    ``token_usage.jsonl`` without per-call ``module=`` plumbing.
    """
    state = request.app.state
    backend = getattr(state, "kb_backend", None)
    query_provider_factory = getattr(state, "kb_query_provider", None)
    tracker_factory = getattr(state, "kb_usage_tracker", None)
    embedding_dim = getattr(state, "kb_embedding_dim", None)
    if (
        backend is None
        or query_provider_factory is None
        or tracker_factory is None
        or embedding_dim is None
    ):
        missing = [
            name
            for name, val in (
                ("kb_backend", backend),
                ("kb_query_provider", query_provider_factory),
                ("kb_usage_tracker", tracker_factory),
                ("kb_embedding_dim", embedding_dim),
            )
            if val is None
        ]
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": "KB_NOT_CONFIGURED",
                "message": "knowledge-base query dependencies not initialized",
                "missing": missing,
            },
        )
    return backend, query_provider_factory, tracker_factory, embedding_dim


@router.post("/kb/query")
async def kb_query_endpoint(
    request: KBQueryRequest, http_request: Request
) -> dict[str, Any]:
    """Synchronously embed ``text`` and return top-k matching ``KBHit``s.

    Backs openspec/changes/kb-query-endpoint/specs/knowledge-base/spec.md
      Requirement: POST /kb/query endpoint

    Empty / unbuilt collection → ``200 {"hits": []}`` (Non-Goal documented:
    no 404 for unbuilt workspace; callers see a single "no results" mode).
    """
    from pathlib import Path as _Path

    backend, provider_factory, tracker_factory, embedding_dim = _require_query_deps(
        http_request
    )

    workspace_path = _Path(request.workspace_root)
    provider = provider_factory(workspace_path)
    tracker = tracker_factory(workspace_path)

    kb = KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root=request.workspace_root,
        embedding_dim=embedding_dim,
    )
    hits = await kb.query(
        request.text,
        top_k=request.top_k,
        filter_path=request.filter_path,
        filter_source_kind=request.filter_source_kind,
    )
    return {"hits": [hit.model_dump(mode="json") for hit in hits]}


__all__ = [
    "router",
    "KBBuildRequest",
    "KBQueryRequest",
    "kb_build_endpoint",
    "kb_query_endpoint",
    "_KBProgressAdapter",
    "_make_kb_progress_adapter",
]
