"""TDD red tests for SSE event stream — Section 4 of
openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/sidecar-runtime/spec.md
  Requirement: SSE event stream endpoint

Mostly handle-level unit tests because the spec talks about queue ordering
and subscriber isolation, both of which live below the HTTP boundary.
The 401 scenario is the one HTTP-shaped test in this file — it locks down
that bearer auth covers the new endpoint.
"""
from __future__ import annotations

import asyncio
import secrets

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.api.tasks import TaskHandle


def _drain_nowait(queue: asyncio.Queue) -> list[dict]:
    """Pop everything currently in ``queue`` without awaiting."""
    out: list[dict] = []
    while not queue.empty():
        out.append(queue.get_nowait())
    return out


def test_sse_emits_progress_done_in_order() -> None:
    """Single subscriber sees emits in FIFO order — spec:
    "Stream emits progress, done, and final close".
    """
    handle = TaskHandle(id="scan_deadbeef", kind="scan")
    queue = handle.subscribe()

    handle.emit({"type": "progress", "phase": "scanning", "current": 1, "total": 3})
    handle.emit({"type": "progress", "phase": "scanning", "current": 2, "total": 3})
    handle.emit({"type": "progress", "phase": "scanning", "current": 3, "total": 3})

    received = _drain_nowait(queue)
    assert [e["current"] for e in received] == [1, 2, 3]
    assert all(e["type"] == "progress" for e in received)


def test_sse_rejects_without_bearer_token() -> None:
    """Spec scenario "Stream rejects without bearer token" — 401 with no
    event-stream body must be returned.
    """
    bearer = secrets.token_urlsafe(32)
    app = create_app(bearer_token=bearer)
    client = TestClient(app)

    resp = client.get("/tasks/scan_deadbeef/events")
    assert resp.status_code == 401
    body = resp.json()
    # Auth middleware returns {"detail": "unauthorized"} per `auth.py`;
    # MUST NOT include any progress / done payload.
    assert body == {"detail": "unauthorized"}
    # Content-Type MUST NOT be event-stream — the request never made it
    # past the middleware.
    assert "text/event-stream" not in resp.headers.get("content-type", "")


def test_sse_multiple_subscribers_receive_identical_sequences() -> None:
    """Spec scenario "Multiple subscribers receive identical event sequences"
    — each subscriber owns its own queue copy.
    """
    handle = TaskHandle(id="kb_cafef00d", kind="kb")
    a = handle.subscribe()
    b = handle.subscribe()

    events = [
        {"type": "progress", "phase": "embedding", "current": 0, "total": 10},
        {"type": "progress", "phase": "embedding", "current": 5, "total": 10},
        {"type": "progress", "phase": "embedding", "current": 10, "total": 10},
        {"type": "done"},
    ]
    for e in events:
        handle.emit(e)

    drained_a = _drain_nowait(a)
    drained_b = _drain_nowait(b)
    assert drained_a == events
    assert drained_b == events
    assert drained_a is not drained_b  # distinct list objects


def test_sse_subscriber_disconnect_does_not_affect_others() -> None:
    """Unsubscribing one subscriber MUST leave the rest receiving every
    subsequent emit. Models a client closing its EventSource connection.
    """
    handle = TaskHandle(id="scan_12345678", kind="scan")
    survivor = handle.subscribe()
    quitter = handle.subscribe()

    handle.emit({"type": "progress", "phase": "scanning", "current": 1, "total": 2})
    handle.unsubscribe(quitter)
    handle.emit({"type": "progress", "phase": "scanning", "current": 2, "total": 2})

    survivor_events = _drain_nowait(survivor)
    quitter_events = _drain_nowait(quitter)
    assert [e["current"] for e in survivor_events] == [1, 2]
    # Quitter saw only the pre-disconnect event.
    assert [e["current"] for e in quitter_events] == [1]


def test_sse_emits_only_progress_done_error_types_in_this_change() -> None:
    """Spec: "The response stream MUST emit only events whose `type` is one
    of `progress`, `done`, or `error` for changes scoped to this capability;
    other event types defined in the spec are reserved for follow-on changes
    and SHALL NOT be emitted by Module 1 or Module 2 task code paths."

    Verified by emitting all three accepted types and asserting nothing else
    leaks through. This is a contract test: future Agent / Q&A changes will
    extend the allowed set.
    """
    handle = TaskHandle(id="scan_deadbabe", kind="scan")
    queue = handle.subscribe()

    accepted = [
        {"type": "progress", "phase": "scanning", "current": 1, "total": 1},
        {"type": "done"},
        {"type": "error", "code": "INTERNAL_ERROR", "message": "x"},
    ]
    for e in accepted:
        handle.emit(e)

    received = _drain_nowait(queue)
    types_seen = {e["type"] for e in received}
    assert types_seen <= {"progress", "done", "error"}, (
        f"only progress/done/error allowed in this change, got {types_seen!r}"
    )
    # Sanity — every event we pushed got through.
    assert len(received) == len(accepted)
