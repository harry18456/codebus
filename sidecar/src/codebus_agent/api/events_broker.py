"""App-level SSE broker for cross-task events (e.g. `provider_config_changed`).

Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: provider_config_changed SSE event surface

Distinct from `api/tasks.py`: that module's broker is per-task (events
flow through a `TaskHandle` keyed by `task_id`). This broker is
*app-level* — every subscriber observes every event the broker emits.
The settings page consumes it via `GET /events?channel=app` to know
when to re-fetch its provider snapshot.

Coalescing: when several mutations land within `_COALESCE_WINDOW_S`
(50 ms per design Decision 3 invariant), the broker collapses them
into a single event whose `data.changed_roles` is the union and whose
boolean flags OR together. Implementation is a tiny state machine in
`_emit_provider_config_changed` — anything more sophisticated is
deferred until coalescing covers more event types.
"""
from __future__ import annotations

import asyncio
from typing import Any

_COALESCE_WINDOW_S: float = 0.05

_STREAM_CLOSE_SENTINEL: dict[str, Any] = {"__close__": True}


class AppEventBroker:
    """Fan-out broker with per-subscriber `asyncio.Queue` clones.

    Subscribers obtain a queue via `subscribe()`. Each `emit()` call
    pushes the same dict reference into every subscriber's queue. The
    broker holds no buffered history — late subscribers see only the
    events emitted after they subscribed.
    """

    def __init__(self) -> None:
        self._queues: list[asyncio.Queue[dict[str, Any]]] = []
        self._lock = asyncio.Lock()
        self._pending_provider_change: dict[str, Any] | None = None
        self._pending_task: asyncio.Task[None] | None = None

    def subscribe(self) -> asyncio.Queue[dict[str, Any]]:
        """Allocate a queue and register it with the broker."""
        queue: asyncio.Queue[dict[str, Any]] = asyncio.Queue()
        self._queues.append(queue)
        return queue

    def unsubscribe(self, queue: asyncio.Queue[dict[str, Any]]) -> None:
        """Remove a queue from the broker. Safe to call twice."""
        try:
            self._queues.remove(queue)
        except ValueError:
            pass

    async def emit(self, event: dict[str, Any]) -> None:
        """Push `event` into every subscriber queue without buffering."""
        for queue in list(self._queues):
            await queue.put(event)

    def emit_nowait(self, event: dict[str, Any]) -> None:
        """Sync flavor of `emit` for handlers that aren't already async-context."""
        for queue in list(self._queues):
            queue.put_nowait(event)

    async def emit_provider_config_changed(
        self,
        *,
        changed_roles: list[str],
        embed_changed: bool,
        providers_pool_changed: bool,
    ) -> None:
        """Coalesce into a single event when called multiple times within 50 ms.

        Coalescing rules:
        - `changed_roles` is the union (order-insensitive).
        - `embed_changed` / `providers_pool_changed` OR together.

        The first call within a quiet window starts a delayed flush
        task; subsequent calls within the same window merge their
        payload into the pending state.
        """
        async with self._lock:
            if self._pending_provider_change is None:
                self._pending_provider_change = {
                    "changed_roles": list(changed_roles),
                    "embed_changed": embed_changed,
                    "providers_pool_changed": providers_pool_changed,
                }
                self._pending_task = asyncio.create_task(self._flush_after_delay())
            else:
                pending = self._pending_provider_change
                merged_roles = set(pending["changed_roles"]) | set(changed_roles)
                pending["changed_roles"] = sorted(merged_roles)
                pending["embed_changed"] = (
                    pending["embed_changed"] or embed_changed
                )
                pending["providers_pool_changed"] = (
                    pending["providers_pool_changed"] or providers_pool_changed
                )

    async def _flush_after_delay(self) -> None:
        await asyncio.sleep(_COALESCE_WINDOW_S)
        async with self._lock:
            payload = self._pending_provider_change
            self._pending_provider_change = None
            self._pending_task = None
        if payload is not None:
            await self.emit(
                {"type": "provider_config_changed", "data": payload}
            )


__all__ = ["AppEventBroker", "_STREAM_CLOSE_SENTINEL"]
