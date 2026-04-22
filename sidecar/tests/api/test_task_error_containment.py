"""TDD red tests for `_run_background_task` error containment — Section 8 of
openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/sidecar-runtime/spec.md
  Requirement: Background task error containment

Invariants under test:
  * SSE error event carries only ``code`` + safe ``message``.
  * Late subscribers (joining after the failure) still see the cached
    error event rather than hanging indefinitely.
  * Full traceback hits the sidecar logger only — never the wire.
"""
from __future__ import annotations

import asyncio
import logging

import pytest

from codebus_agent.api.tasks import (
    ERROR_CODES,
    TaskHandle,
    _run_background_task,
)


async def test_background_exception_emits_safe_error_event() -> None:
    """Spec scenario: "Background task exception surfaces as safe error event".

    The wire payload MUST NOT include `repr(exc)` / class names / paths.
    """
    handle = TaskHandle(id="scan_deadbeef", kind="scan")
    queue = handle.subscribe()

    async def boom() -> dict:
        raise RuntimeError(
            "secret-internal-detail /etc/passwd should NEVER hit the wire"
        )

    await _run_background_task(handle, boom)

    received: list[dict] = []
    while not queue.empty():
        received.append(queue.get_nowait())

    error_events = [e for e in received if e.get("type") == "error"]
    assert len(error_events) == 1
    err = error_events[0]
    assert err["code"] in ERROR_CODES
    assert isinstance(err["message"], str)
    # No leakage: neither `repr(exc)`, class name, nor the leaked path.
    assert "RuntimeError" not in err["message"]
    assert "secret-internal-detail" not in err["message"]
    assert "/etc/passwd" not in err["message"]
    # And the handle's status reflects the failure.
    assert handle.status == "error"


async def test_subscriber_after_error_still_receives_terminal_event() -> None:
    """Spec scenario: "Subscriber connecting after error still observes
    terminal event" — late joiners replay the cached error.
    """
    handle = TaskHandle(id="kb_cafef00d", kind="kb")

    async def boom() -> dict:
        raise ValueError("kb embed broke")

    await _run_background_task(handle, boom)
    # Background already finished. Now subscribe and assert we still see
    # the cached error event before the stream-close sentinel.
    late = handle.subscribe()
    received: list[dict] = []
    # Pull at most a handful — enough to drain replay + sentinel.
    for _ in range(4):
        try:
            received.append(late.get_nowait())
        except asyncio.QueueEmpty:
            break
    error_payloads = [e for e in received if isinstance(e, dict) and e.get("type") == "error"]
    assert len(error_payloads) == 1
    assert error_payloads[0]["code"] in ERROR_CODES


async def test_full_traceback_written_to_logger_only(caplog) -> None:
    """`logger.exception(...)` records the underlying exception class +
    traceback so operators retain diagnostic context, while subscribers
    only see the sanitized event.
    """
    handle = TaskHandle(id="scan_12345678", kind="scan")
    queue = handle.subscribe()

    async def boom() -> dict:
        raise RuntimeError("traceable-internal-message")

    with caplog.at_level(logging.ERROR, logger="codebus_agent.api.tasks"):
        await _run_background_task(handle, boom)

    # Wire payload — no traceback, no class name.
    error_events = [e for e in (queue.get_nowait() for _ in range(queue.qsize())) if e.get("type") == "error"]
    assert error_events, "expected an error event on the wire"
    assert "traceback" not in error_events[0]["message"].lower()
    assert "RuntimeError" not in error_events[0]["message"]

    # Logger payload — class name + message must appear (so on-call has
    # something to grep). At least one ERROR record from our logger.
    matching = [
        rec for rec in caplog.records
        if rec.name == "codebus_agent.api.tasks" and rec.levelno == logging.ERROR
    ]
    assert matching, f"expected ERROR log line; got {[r.name for r in caplog.records]!r}"
    formatted = "\n".join(
        rec.getMessage() + (rec.exc_text or "") for rec in matching
    )
    assert "RuntimeError" in formatted
    assert "traceable-internal-message" in formatted
