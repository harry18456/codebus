"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/pii-provider/spec.md
  Requirement: TrackedProvider gates PII inner classes via PII_ALLOWED_INNER_TYPES
    Scenario: Source-grep test pins allowlist to spec
  Requirement: Outbound LLM traffic gated by TrackedProvider whitelist (MODIFIED)
    Scenario: ALLOWED_INNER_TYPES and PII_ALLOWED_INNER_TYPES are disjoint

D-033 Decision 2 — defensive test rejecting drift between
``TrackedProvider.PII_ALLOWED_INNER_TYPES`` runtime contents and the
spec-pinned set ``{RuleBasedPIIProvider, MockPIIProvider}``. Mirrors
the same source-grep pattern used by ``ALLOWED_INNER_TYPES``.
"""
from __future__ import annotations

import inspect

from codebus_agent.providers import tracked as tracked_module
from codebus_agent.providers.mock import MockProvider
from codebus_agent.providers.openai_chat import OpenAIChatProvider
from codebus_agent.providers.openai_embedding import OpenAIEmbeddingProvider
from codebus_agent.providers.pii import (
    MockPIIProvider,
    RuleBasedPIIProvider,
)
from codebus_agent.providers.tracked import TrackedProvider


def test_pii_allowlist_runtime_value_pinned_to_spec() -> None:
    """Scenario: Source-grep test pins allowlist to spec."""
    expected = frozenset({RuleBasedPIIProvider, MockPIIProvider})
    assert TrackedProvider.PII_ALLOWED_INNER_TYPES == expected, (
        f"PII_ALLOWED_INNER_TYPES drifted: expected {expected}, "
        f"got {TrackedProvider.PII_ALLOWED_INNER_TYPES}"
    )


def test_llm_allowlist_runtime_value_pinned_to_spec() -> None:
    """The LLM-lane allowlist remains exactly the chat-provider-wiring set."""
    expected = frozenset(
        {MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}
    )
    assert TrackedProvider.ALLOWED_INNER_TYPES == expected, (
        f"ALLOWED_INNER_TYPES drifted: expected {expected}, "
        f"got {TrackedProvider.ALLOWED_INNER_TYPES}"
    )


def test_two_allowlists_are_disjoint() -> None:
    """Scenario: ALLOWED_INNER_TYPES and PII_ALLOWED_INNER_TYPES are disjoint."""
    intersection = (
        TrackedProvider.ALLOWED_INNER_TYPES
        & TrackedProvider.PII_ALLOWED_INNER_TYPES
    )
    assert intersection == frozenset(), (
        f"Allowlist disjoint invariant violated: {intersection}"
    )


def test_source_grep_finds_both_pii_class_names() -> None:
    """The class names referenced by ``PII_ALLOWED_INNER_TYPES`` MUST appear
    as literal source tokens in ``tracked.py`` so source-grep tooling
    (``tests/test_no_jsonl_literal_drift.py`` style) can detect drift.
    """
    src = inspect.getsource(tracked_module)
    for cls in (RuleBasedPIIProvider, MockPIIProvider):
        assert cls.__name__ in src, (
            f"{cls.__name__!r} MUST appear as a literal source token in "
            f"tracked.py — found {src.count(cls.__name__)} occurrences"
        )
