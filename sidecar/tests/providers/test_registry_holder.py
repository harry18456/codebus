"""Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: RegistryHolder enables atomic registry hot-swap
    Scenario: holder.current returns same instance until swap
    Scenario: In-flight task continues with captured reference after swap
    Scenario: swap is atomic across concurrent reads

The holder wraps a single immutable `ProviderRegistry` reference under
an `asyncio.Lock`; tests prove identity-equality on consecutive reads,
post-swap reference replacement, in-flight reference stability, and
read/write atomicity across concurrent coroutines.
"""
from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from codebus_agent.providers import (
    LLMCallLogger,
    MockProvider,
    ProviderRegistry,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)
from codebus_agent.providers.registry_holder import RegistryHolder
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


def _wrap(tmp_path: Path, *, name: str, role: ProviderRole) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / f"{name}_token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / f"{name}_llm_calls.jsonl")
    return TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=role,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(
            tmp_path / f"{name}_sanitize_audit.jsonl"
        ),
        rules_version="test-v1",
    )


def _make_registry(tmp_path: Path, *, name: str) -> ProviderRegistry:
    return ProviderRegistry(
        {
            ProviderRole.REASONING: _wrap(
                tmp_path, name=f"{name}-reasoning", role=ProviderRole.REASONING
            ),
            ProviderRole.JUDGE: _wrap(
                tmp_path, name=f"{name}-judge", role=ProviderRole.JUDGE
            ),
        }
    )


@pytest.mark.asyncio
async def test_current_returns_same_instance_until_swap(tmp_path: Path) -> None:
    """Scenario: holder.current returns same instance until swap.

    Two consecutive `await holder.current()` calls with no intervening
    `swap()` MUST return the same `ProviderRegistry` instance (identity
    comparison).
    """
    registry = _make_registry(tmp_path, name="initial")
    holder = RegistryHolder(registry)

    first = await holder.current()
    second = await holder.current()

    assert first is second
    assert first is registry


@pytest.mark.asyncio
async def test_swap_replaces_current_reference(tmp_path: Path) -> None:
    """`holder.swap(new)` MUST replace what subsequent `current()` returns."""
    initial = _make_registry(tmp_path, name="initial")
    replacement = _make_registry(tmp_path, name="replacement")
    holder = RegistryHolder(initial)

    assert await holder.current() is initial

    await holder.swap(replacement)

    assert await holder.current() is replacement
    assert replacement is not initial


@pytest.mark.asyncio
async def test_in_flight_reference_unaffected_by_swap(tmp_path: Path) -> None:
    """Scenario: In-flight task continues with captured reference after swap.

    A caller that captured `holder.current()` before a swap MUST keep
    using the old registry; a new caller after the swap MUST receive the
    new registry.
    """
    initial = _make_registry(tmp_path, name="initial")
    replacement = _make_registry(tmp_path, name="replacement")
    holder = RegistryHolder(initial)

    in_flight = await holder.current()

    await holder.swap(replacement)

    # In-flight reference still points at the old registry — Python ref
    # semantics: the local binding captured a value, swap mutates the
    # holder's slot, not the captured reference.
    assert in_flight is initial
    assert in_flight is not replacement
    # In-flight provider lookup still uses old registry.
    assert in_flight.get(ProviderRole.REASONING) is initial.get(
        ProviderRole.REASONING
    )
    # New caller receives the post-swap registry.
    assert await holder.current() is replacement


@pytest.mark.asyncio
async def test_swap_is_atomic_across_concurrent_reads(tmp_path: Path) -> None:
    """Scenario: swap is atomic across concurrent reads.

    N concurrent `await holder.current()` calls interleaved with one
    `holder.swap(new_registry)` MUST each receive either the old or the
    new registry — never a partially-constructed state — and the swap
    MUST complete in finite time.
    """
    initial = _make_registry(tmp_path, name="initial")
    replacement = _make_registry(tmp_path, name="replacement")
    holder = RegistryHolder(initial)

    n_readers = 50

    async def reader() -> ProviderRegistry:
        return await holder.current()

    async def swapper() -> None:
        await holder.swap(replacement)

    reader_tasks = [asyncio.create_task(reader()) for _ in range(n_readers)]
    swap_task = asyncio.create_task(swapper())

    results = await asyncio.gather(*reader_tasks, swap_task)
    swap_result = results[-1]
    read_results = results[:-1]

    assert swap_result is None
    # Every read MUST be one of the two well-defined registries.
    for r in read_results:
        assert r is initial or r is replacement, (
            "atomicity breach: reader observed neither old nor new registry"
        )
    # Final state MUST be the replacement.
    assert await holder.current() is replacement


def test_registry_holder_is_exported_from_providers_package() -> None:
    """The holder MUST be re-exported from `codebus_agent.providers`."""
    from codebus_agent import providers as providers_pkg

    assert hasattr(providers_pkg, "RegistryHolder")
    assert providers_pkg.RegistryHolder is RegistryHolder
