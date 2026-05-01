"""RegistryHolder — atomic hot-swap wrapper around `ProviderRegistry`.

Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: RegistryHolder enables atomic registry hot-swap
    Scenario: holder.current returns same instance until swap
    Scenario: In-flight task continues with captured reference after swap
    Scenario: swap is atomic across concurrent reads

Design provider-settings-and-onboarding §Decision 3:
- Inner `ProviderRegistry` stays immutable; each `swap()` installs a
  freshly constructed registry — never mutates an existing one.
- `current()` and `swap()` share an `asyncio.Lock` so concurrent reads
  always observe either the pre-swap or the post-swap reference, never
  a torn state.
- In-flight callers that already captured a reference keep using it
  (Python ref semantics — `swap()` only replaces the holder's own slot).
"""
from __future__ import annotations

import asyncio

from .registry import ProviderRegistry


class RegistryHolder:
    """Single mutex-guarded reference to the current immutable registry."""

    def __init__(self, initial: ProviderRegistry) -> None:
        if not isinstance(initial, ProviderRegistry):
            raise TypeError(
                "RegistryHolder requires a ProviderRegistry instance; "
                f"got {type(initial).__name__}"
            )
        self._registry: ProviderRegistry = initial
        self._lock = asyncio.Lock()

    async def current(self) -> ProviderRegistry:
        """Return the active registry under the lock.

        Returning under the lock guarantees readers cannot observe a
        torn state mid-swap; the returned reference is then safe to
        retain past the lock window because the underlying registry is
        immutable.
        """
        async with self._lock:
            return self._registry

    async def swap(self, new_registry: ProviderRegistry) -> None:
        """Atomically replace the active registry reference."""
        if not isinstance(new_registry, ProviderRegistry):
            raise TypeError(
                "RegistryHolder.swap requires a ProviderRegistry instance; "
                f"got {type(new_registry).__name__}"
            )
        async with self._lock:
            self._registry = new_registry
