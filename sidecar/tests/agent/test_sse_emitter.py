"""RED tests for SSEEmitter (agent-sse-wiring §2).

Backs openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: SSEEmitter is an opt-in runtime-checkable Protocol

Scenarios:
  * NullEmitter satisfies Protocol
  * TaskHandleEmitter fans out to subscribers
  * Custom non-inheriting impl satisfies `@runtime_checkable`
"""
from __future__ import annotations

import asyncio

from codebus_agent.agent.emitter import NullEmitter, SSEEmitter, TaskHandleEmitter
from codebus_agent.api.tasks import TaskHandle


def test_null_emitter_satisfies_protocol() -> None:
    """`NullEmitter()` is a structural SSEEmitter and `.emit` is a side-effect-free no-op."""
    emitter = NullEmitter()
    assert isinstance(emitter, SSEEmitter), (
        "NullEmitter MUST satisfy the runtime-checkable SSEEmitter Protocol"
    )
    # Two emits — no exceptions, no I/O, no implicit state mutation.
    emitter.emit({"type": "progress", "current": 1, "total": 10})
    emitter.emit({"type": "usage_delta", "module": "judge"})


def test_task_handle_emitter_fans_out() -> None:
    """`TaskHandleEmitter(handle).emit(event)` fans out to every subscriber queue."""
    handle = TaskHandle(id="explore_deadbeef", kind="explore")
    a = handle.subscribe()
    b = handle.subscribe()

    emitter = TaskHandleEmitter(handle)
    event = {"type": "progress", "phase": "exploring", "current": 2, "total": 10}
    emitter.emit(event)

    def _drain(q: asyncio.Queue) -> list[dict]:
        out: list[dict] = []
        while not q.empty():
            out.append(q.get_nowait())
        return out

    drained_a = _drain(a)
    drained_b = _drain(b)
    assert drained_a == [event]
    assert drained_b == [event]


def test_custom_impl_without_inherit_satisfies_protocol() -> None:
    """Structural check — any class with `emit(event)` passes `isinstance`.

    This is the `@runtime_checkable` pin so Q&A Agent / custom test doubles
    can plug into the SSE wiring without importing the Protocol base.
    """

    class _Spy:
        def __init__(self) -> None:
            self.events: list[dict] = []

        def emit(self, event: dict) -> None:
            self.events.append(event)

    spy = _Spy()
    assert isinstance(spy, SSEEmitter), (
        "@runtime_checkable MUST accept structurally-conforming classes"
    )
    spy.emit({"type": "llm_call", "request_id": "r1"})
    assert spy.events == [{"type": "llm_call", "request_id": "r1"}]
