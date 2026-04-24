"""SSEEmitter — structural Protocol + two concrete implementations.

Backs SHALL clauses in
openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: SSEEmitter is an opt-in runtime-checkable Protocol

The Explorer loop, `TrackedProvider`, and `LLMCallLogger` each take an
optional `emitter: SSEEmitter | None = None`. Passing `None` keeps the
legacy file-only behaviour; passing any structurally-conforming object
(inherited or not — the Protocol is `@runtime_checkable`) fans SSE
events out through the caller-supplied channel.

Two concrete impls ship here:

- ``NullEmitter`` — silent no-op; the default when no SSE wire exists
  (unit tests, golden-sample replay, in-process Explorer runs).
- ``TaskHandleEmitter`` — wraps a ``TaskHandle`` and delegates ``.emit``
  straight to ``handle.emit(event)``, piggybacking on the existing
  ``sse-progress-skeleton`` fan-out machinery.
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from codebus_agent.api.tasks import TaskHandle


__all__ = ["SSEEmitter", "NullEmitter", "TaskHandleEmitter"]


@runtime_checkable
class SSEEmitter(Protocol):
    """Structural SSE emission surface.

    `@runtime_checkable` lets callers assert conformance via `isinstance`
    without forcing nominal inheritance — test doubles stay short and
    future Q&A / Topic-mode emitters plug in without touching this file.
    """

    def emit(self, event: dict) -> None: ...


class NullEmitter:
    """Silent emitter. Every callable that accepts `SSEEmitter | None`
    may substitute `NullEmitter()` to drop the `is None` check from its
    hot path at the cost of one extra allocation.
    """

    def emit(self, event: dict) -> None:
        return None


class TaskHandleEmitter:
    """Fan out events through an existing `TaskHandle`.

    Kept deliberately thin: every emit delegates to `handle.emit` so the
    single-slot registry's fan-out + terminal-event semantics (see
    `api/tasks.py`) remain the canonical source of truth.
    """

    def __init__(self, handle: "TaskHandle") -> None:
        self._handle = handle

    def emit(self, event: dict) -> None:
        self._handle.emit(event)
