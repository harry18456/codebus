"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Wrapper preserves protocol shape (via TrackedProvider inner-type guard)

The role-level registry-dispatch and wrapping invariants migrated to
`test_registry_role_dispatch.py` and `test_registry_guard_roles.py`
when the llm-role-routing change reshaped the construction API from
`register(name, provider)` to `ProviderRegistry({role: provider})`.
The surviving scenario here covers the `TrackedProvider` inner-type
guard, which is orthogonal to the registry surface and unchanged by
the role refactor.
"""
from __future__ import annotations

from pathlib import Path

import pytest

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


class _WouldBeOpenAI:
    """Stand-in for OpenAI / Anthropic / Gemini / Ollama adapters."""

    name = "openai-fake"

    async def chat(self, messages, *, response_model):
        raise AssertionError("should never be called in M1")

    async def embed(self, texts):
        raise AssertionError("should never be called in M1")


def test_tracked_rejects_non_mock_inner_provider(tmp_path: Path) -> None:
    """`TrackedProvider.ALLOWED_INNER_TYPES` blocks non-Mock inner providers.

    This M1 invariant is orthogonal to registry dispatch and must
    survive the llm-role-routing refactor unchanged: any production
    attempt to wrap a real vendor adapter dies at wrap time, not at
    registration time.
    """
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    with pytest.raises(TypeError, match="MockProvider"):
        TrackedProvider(
            _WouldBeOpenAI(),
            tracker=tracker,
            logger=logger,
            role=ProviderRole.CHAT,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
            rules_version="test-v1",
        )
