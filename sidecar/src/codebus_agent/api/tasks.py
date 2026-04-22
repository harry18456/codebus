"""Single-slot task registry + SSE & result endpoints.

Backs openspec/changes/sse-progress-skeleton/specs/sidecar-runtime/spec.md
  Requirement: Single-slot in-memory task registry
  Requirement: SSE event stream endpoint
  Requirement: Task result lookup endpoint
  Requirement: task_id format
  Requirement: Background task error containment

Design notes (see `openspec/changes/sse-progress-skeleton/design.md`):

* `Single-slot task store over dict-based pool` — registry holds a single
  ``Optional[TaskHandle]`` so the only states are *empty / running /
  terminal*. ``create()`` returns ``None`` while another task is still
  ``running``; the endpoint layer translates that into HTTP 409
  ``TASK_IN_FLIGHT``.
* `task_id 用前綴 + 8 字 hex random` — ``secrets.token_hex(4)`` is plenty
  of entropy for a single-slot store and keeps log lines easy to grep.
* `asyncio.Queue 作 event channel；每位訂閱者自帶 asyncio.Queue 副本` —
  ``TaskHandle`` keeps a list of subscriber queues; emit fans out.
* `Background task error containment` — wrap the user-supplied coroutine
  so subscribers always get either ``done`` or a sanitized ``error`` event
  before the stream closes. ``repr(exc)`` / tracebacks NEVER hit the wire.
"""
from __future__ import annotations

import asyncio
import logging
import secrets
from typing import Any, Awaitable, Callable, Literal

from fastapi import APIRouter, HTTPException, Request, status
from sse_starlette.sse import EventSourceResponse

logger = logging.getLogger(__name__)

TaskKind = Literal["scan", "kb"]
TaskStatus = Literal["running", "done", "error"]

# Closed values per design `error event 安全性`. The endpoint MUST pick from
# this table — never echo `repr(exc)` into the wire.
ERROR_CODES: frozenset[str] = frozenset(
    {"SCAN_FAILED", "KB_EMBED_FAILED", "INTERNAL_ERROR"}
)

_VALID_KINDS: frozenset[str] = frozenset({"scan", "kb"})

# Sentinel pushed into subscriber queues to signal "stream closed". Picked as
# a private string so callers never collide with a real event payload.
_STREAM_CLOSE_SENTINEL: dict[str, Any] = {"__close__": True}


def _generate_task_id(kind: TaskKind) -> str:
    """Return ``{kind}_{8-hex}``; raise ``ValueError`` on unknown kind.

    Spec `task_id format`: ``^(scan|kb)_[0-9a-f]{8}$``. ``secrets.token_hex(4)``
    yields exactly 8 lowercase hex chars from a cryptographic source.
    """
    if kind not in _VALID_KINDS:
        raise ValueError(
            f"unknown task kind {kind!r}; expected one of {sorted(_VALID_KINDS)}"
        )
    return f"{kind}_{secrets.token_hex(4)}"


class TaskHandle:
    """One in-flight or terminal background task.

    Mutating ``status`` / ``result`` from outside this module is OK during
    M2 wiring — the registry doesn't enforce a state machine; the
    ``_run_background_task`` wrapper does.
    """

    __slots__ = (
        "id",
        "kind",
        "status",
        "subscribers",
        "result",
        "error_event",
    )

    def __init__(self, id: str, kind: TaskKind) -> None:
        self.id = id
        self.kind: TaskKind = kind
        self.status: TaskStatus = "running"
        # Each SSE subscriber gets its own queue; emit() fans out to all.
        self.subscribers: list[asyncio.Queue[dict[str, Any]]] = []
        # Terminal payload — populated by the background coroutine on success.
        self.result: dict[str, Any] | None = None
        # Cached terminal `error` event so late subscribers still see it
        # rather than hanging forever (spec scenario "Subscriber connecting
        # after error still observes terminal event").
        self.error_event: dict[str, Any] | None = None

    def subscribe(self) -> asyncio.Queue[dict[str, Any]]:
        """Register a fresh subscriber queue and return it.

        The returned queue is owned by the caller; ``unsubscribe`` MUST be
        called when the SSE stream closes so the registry doesn't pile up
        garbage queues for long-lived tasks.
        """
        queue: asyncio.Queue[dict[str, Any]] = asyncio.Queue()
        self.subscribers.append(queue)
        # Spec scenario "Subscriber connecting after error still observes
        # terminal event" — replay the cached terminal event for late joiners.
        if self.status == "error" and self.error_event is not None:
            queue.put_nowait(self.error_event)
            queue.put_nowait(_STREAM_CLOSE_SENTINEL)
        elif self.status == "done":
            queue.put_nowait({"type": "done"})
            queue.put_nowait(_STREAM_CLOSE_SENTINEL)
        return queue

    def unsubscribe(self, queue: asyncio.Queue[dict[str, Any]]) -> None:
        """Remove ``queue`` from subscriber list; idempotent."""
        try:
            self.subscribers.remove(queue)
        except ValueError:
            pass

    def emit(self, event: dict[str, Any]) -> None:
        """Fan out ``event`` to every subscriber queue.

        Called from the producer coroutine. asyncio is single-threaded and
        this method has no ``await`` point, so the list iteration is
        atomic w.r.t. subscribe / unsubscribe.
        """
        for queue in list(self.subscribers):
            queue.put_nowait(event)

    def close_subscribers(self) -> None:
        """Signal all subscribers that the stream is over."""
        for queue in list(self.subscribers):
            queue.put_nowait(_STREAM_CLOSE_SENTINEL)


class TaskRegistry:
    """Single-slot in-memory task store.

    Concurrency model: instantiated once per ``FastAPI`` app and accessed
    via ``request.app.state.tasks``. asyncio is single-threaded so ``create``
    / ``get`` / ``current_running`` are all atomic without locks.
    """

    def __init__(self) -> None:
        self._slot: TaskHandle | None = None

    def create(self, kind: TaskKind) -> TaskHandle | None:
        """Allocate a new handle and overwrite the slot, or return ``None``
        when another task is still ``running``.

        ``None`` is the registry-level signal that the endpoint layer must
        translate into HTTP 409 ``TASK_IN_FLIGHT`` (per spec scenario
        "Second concurrent task rejected with 409"). Returning ``None`` —
        rather than raising — keeps the call-site terse and avoids leaking
        framework-specific exceptions into a pure-Python data structure.
        """
        current = self._slot
        if current is not None and current.status == "running":
            return None
        handle = TaskHandle(id=_generate_task_id(kind), kind=kind)
        self._slot = handle
        return handle

    def get(self, task_id: str) -> TaskHandle | None:
        """Look up the slot by id — only reachable while the slot is the
        most recently created handle. After overwrite, old ids return
        ``None`` (spec: "until a subsequent successful task creation
        overwrites the slot").
        """
        if self._slot is not None and self._slot.id == task_id:
            return self._slot
        return None

    def current_running(self) -> TaskHandle | None:
        """Return the slot iff its status is ``running``; otherwise ``None``."""
        if self._slot is not None and self._slot.status == "running":
            return self._slot
        return None


router = APIRouter()


def _classify_exception(exc: BaseException) -> str:
    """Pick one of ``ERROR_CODES`` for ``exc``; default ``INTERNAL_ERROR``.

    The mapping table is deliberately tiny — narrowing means more handlers
    leak shape information about internal failures into the wire. The full
    exception is logged separately so operators still have full diagnostic
    context.
    """
    name = type(exc).__name__
    # Heuristic dispatch — kept narrow on purpose. Each branch's message is
    # already safe (constants, no `repr(exc)`).
    if name == "ScanError" or "scan" in name.lower():
        return "SCAN_FAILED"
    if "embed" in name.lower() or "kb" in name.lower():
        return "KB_EMBED_FAILED"
    return "INTERNAL_ERROR"


def _safe_error_message(code: str) -> str:
    """Map an error code to a human-readable, sanitized message."""
    if code == "SCAN_FAILED":
        return "scan task failed"
    if code == "KB_EMBED_FAILED":
        return "knowledge-base build failed"
    return "internal sidecar error"


async def _run_background_task(
    handle: TaskHandle,
    coro_factory: Callable[[], Awaitable[Any]],
    *,
    classify: Callable[[BaseException], str] = _classify_exception,
) -> None:
    """Spawn ``coro_factory()`` under error containment.

    Invariants per spec `Background task error containment`:
      * Subscribers always observe either a ``done`` or an ``error`` event
        before the stream closes.
      * The wire ``error`` event carries only ``code`` + safe ``message``;
        ``repr(exc)`` / tracebacks NEVER hit the wire — they go to the
        sidecar logger instead.
      * ``done`` and ``error`` are mutually exclusive: ``done`` only fires
        on a clean return path, ``error`` only on the except path.
    """
    try:
        result = await coro_factory()
    except BaseException as exc:  # noqa: BLE001 — we log + sanitize, never propagate
        code = classify(exc)
        if code not in ERROR_CODES:
            code = "INTERNAL_ERROR"
        message = _safe_error_message(code)
        error_event = {"type": "error", "code": code, "message": message}
        handle.status = "error"
        handle.error_event = error_event
        handle.emit(error_event)
        logger.exception(
            "background task %s (%s) failed: %s",
            handle.id,
            handle.kind,
            type(exc).__name__,
        )
    else:
        handle.result = result if isinstance(result, dict) else None
        handle.status = "done"
        handle.emit({"type": "done"})
    finally:
        handle.close_subscribers()


def _require_registry(request: Request) -> TaskRegistry:
    registry = getattr(request.app.state, "tasks", None)
    if registry is None:
        # Defensive — `create_app` always wires a registry; if this fires
        # in production something has tampered with `app.state`.
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="task registry not initialized",
        )
    return registry


@router.get("/tasks/{task_id}/events")
async def stream_task_events(task_id: str, request: Request) -> EventSourceResponse:
    """Subscribe to a task's event stream.

    Spec scenarios honoured here:
      * "Stream emits progress, done, and final close" — events flow in
        FIFO order; the wrapper closes the stream after the terminal one.
      * "Multiple subscribers receive identical event sequences" — each
        subscriber gets its own ``asyncio.Queue``; emits fan out.
      * "Subscriber connecting after error still observes terminal event"
        — ``TaskHandle.subscribe()`` replays the cached terminal event.
    """
    registry = _require_registry(request)
    handle = registry.get(task_id)
    if handle is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "TASK_NOT_FOUND", "task_id": task_id},
        )

    queue = handle.subscribe()

    async def _event_generator():
        try:
            while True:
                event = await queue.get()
                if event is _STREAM_CLOSE_SENTINEL:
                    return
                yield {"data": _json_dump(event)}
        finally:
            handle.unsubscribe(queue)

    return EventSourceResponse(_event_generator())


@router.get("/tasks/{task_id}/result")
async def get_task_result(task_id: str, request: Request) -> dict[str, Any]:
    """Return the task's terminal payload, or 409 / 404 per spec."""
    registry = _require_registry(request)
    handle = registry.get(task_id)
    if handle is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "TASK_NOT_FOUND", "task_id": task_id},
        )
    if handle.status == "running":
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_NOT_DONE",
                "task_id": task_id,
                "status": "running",
            },
        )
    if handle.status == "error":
        # `error` is terminal too but distinct from `done`; the spec only
        # defines 200-on-done. Surface the cached error event verbatim so
        # callers can introspect without trusting the SSE channel.
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_FAILED",
                "task_id": task_id,
                "error": handle.error_event,
            },
        )
    # status == "done"
    return handle.result if handle.result is not None else {}


def _json_dump(event: dict[str, Any]) -> str:
    """Compact JSON for SSE ``data:`` lines.

    Pulled into a tiny helper so tests can monkeypatch it if a future
    change wants to add NDJSON or msgpack on top.
    """
    import json

    return json.dumps(event, separators=(",", ":"), ensure_ascii=False)


__all__ = [
    "ERROR_CODES",
    "TaskHandle",
    "TaskKind",
    "TaskRegistry",
    "TaskStatus",
    "_classify_exception",
    "_generate_task_id",
    "_run_background_task",
    "router",
]
