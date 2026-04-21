"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: Registry enforces TrackedProvider wrapping per role
    Scenario: Unwrapped provider in any role raises
    Scenario: Wrapped providers in every role succeed

Design llm-role-routing §3: guard fires at instantiation only —
`get(role)` is a bare dict lookup to keep runtime call path cheap.
"""
from __future__ import annotations

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
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


def _wrap(
    tmp_path: Path, *, tag: str, role: ProviderRole = ProviderRole.CHAT
) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / f"{tag}_token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / f"{tag}_llm_calls.jsonl")
    return TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=role,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(
            tmp_path / f"{tag}_sanitize_audit.jsonl"
        ),
        rules_version="test-v1",
    )


def test_registry_rejects_unwrapped_provider_in_reasoning_role() -> None:
    """Scenario: Unwrapped provider in any role raises — REASONING."""
    with pytest.raises(ValueError) as exc_info:
        ProviderRegistry({ProviderRole.REASONING: MockProvider()})

    msg = str(exc_info.value)
    assert "reasoning" in msg.lower()
    assert "MockProvider" in msg


def test_registry_rejects_unwrapped_provider_in_judge_role(tmp_path: Path) -> None:
    """Scenario: Unwrapped provider in any role raises — even if other roles are wrapped."""
    with pytest.raises(ValueError) as exc_info:
        ProviderRegistry(
            {
                ProviderRole.REASONING: _wrap(
                    tmp_path, tag="reasoning", role=ProviderRole.REASONING
                ),
                ProviderRole.JUDGE: MockProvider(),
            }
        )

    msg = str(exc_info.value)
    assert "judge" in msg.lower()
    assert "MockProvider" in msg


def test_registry_accepts_all_roles_wrapped(tmp_path: Path) -> None:
    """Scenario: Wrapped providers in every role succeed."""
    registry = ProviderRegistry(
        {
            ProviderRole.REASONING: _wrap(
                tmp_path, tag="reasoning", role=ProviderRole.REASONING
            ),
            ProviderRole.JUDGE: _wrap(
                tmp_path, tag="judge", role=ProviderRole.JUDGE
            ),
            ProviderRole.CHAT: _wrap(
                tmp_path, tag="chat", role=ProviderRole.CHAT
            ),
            ProviderRole.EMBED: _wrap(
                tmp_path, tag="embed", role=ProviderRole.EMBED
            ),
        }
    )
    assert set(registry.roles) == {
        ProviderRole.REASONING,
        ProviderRole.JUDGE,
        ProviderRole.CHAT,
        ProviderRole.EMBED,
    }


def test_registry_path_requires_sanitizer_injection(tmp_path: Path) -> None:
    """Task 7.7 — registry construction path MUST NOT allow building
    TrackedProvider without a SanitizerEngine. The ValueError fires at
    TrackedProvider.__init__, which is the choke point the registry
    relies on (see openspec/changes/sanitizer-safety-chain/specs/llm-provider/spec.md
    Requirement "TrackedProvider applies Sanitizer Pass 2 before dispatch").
    """
    from codebus_agent.sanitizer import SanitizerAuditLogger

    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")

    with pytest.raises(ValueError, match="sanitizer"):
        TrackedProvider(
            MockProvider(),
            tracker=tracker,
            logger=logger,
            role=ProviderRole.CHAT,
            sanitizer=None,  # type: ignore[arg-type]
            sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
            rules_version="test-v1",
        )


def test_registry_get_does_not_revalidate_at_runtime(tmp_path: Path) -> None:
    """Design §3: guard fires at __init__ only; `get(role)` is a dict lookup.

    We stuff the internal dict post-construction with an unwrapped
    provider — `get(role)` must still return it without raising,
    proving the invariant truly lives at construction time.
    """
    registry = ProviderRegistry(
        {ProviderRole.JUDGE: _wrap(tmp_path, tag="judge", role=ProviderRole.JUDGE)}
    )
    raw = MockProvider()
    registry._providers[ProviderRole.CHAT] = raw  # type: ignore[attr-defined]

    assert registry.get(ProviderRole.CHAT) is raw
