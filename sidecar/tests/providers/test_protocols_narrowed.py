"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/llm-provider/spec.md
  Requirement: LLMProvider protocol (MODIFIED)
    Scenario: Protocol declares only chat
    Scenario: Protocol is runtime checkable over narrowed interface
    Scenario: Existing chat-only implementations satisfy narrowed protocol
  Requirement: EmbeddingProvider protocol (ADDED)
    Scenario: Protocol declares only embed
    Scenario: Protocol is runtime checkable
    Scenario: Existing embed-only implementation satisfies new protocol
    Scenario: MockProvider satisfies both LLMProvider and EmbeddingProvider

Design context: D-033 Decision 7 — MockProvider stays a single class
implementing both narrow Protocols simultaneously (Python structural
subtyping permits this) so existing ~50 tests do not need rewriting.
"""
from __future__ import annotations

from codebus_agent.providers import (
    EmbeddingProvider,
    LLMProvider,
    MockProvider,
)


def test_llm_provider_protocol_declares_only_chat() -> None:
    """Scenario: Protocol declares only chat."""
    assert hasattr(LLMProvider, "chat")
    assert not hasattr(LLMProvider, "embed")


def test_embedding_provider_protocol_declares_only_embed() -> None:
    """Scenario: Protocol declares only embed."""
    assert hasattr(EmbeddingProvider, "embed")
    assert not hasattr(EmbeddingProvider, "chat")


def test_mock_provider_satisfies_llm_protocol() -> None:
    """Scenario: MockProvider satisfies LLMProvider (chat-only narrowing)."""
    assert isinstance(MockProvider(), LLMProvider)


def test_mock_provider_satisfies_embedding_protocol() -> None:
    """Scenario: MockProvider satisfies EmbeddingProvider (embed-only narrowing)."""
    assert isinstance(MockProvider(), EmbeddingProvider)


def test_chat_only_class_satisfies_narrowed_llm_provider() -> None:
    """Scenario: Existing chat-only implementations satisfy narrowed protocol.

    A throwaway class with only ``chat`` (no ``embed``) MUST be accepted
    as an LLMProvider — this is the post-D-033 narrowing benefit:
    OpenAIChatProvider (which has no embed method) is now a valid
    isinstance match.
    """

    class _ChatOnly:
        async def chat(self, messages, *, response_model):  # type: ignore[no-untyped-def]
            ...

    assert isinstance(_ChatOnly(), LLMProvider)
    assert not isinstance(_ChatOnly(), EmbeddingProvider)


def test_embed_only_class_satisfies_narrowed_embedding_provider() -> None:
    """Scenario: Existing embed-only implementation satisfies new protocol."""

    class _EmbedOnly:
        async def embed(self, texts):  # type: ignore[no-untyped-def]
            ...

    assert isinstance(_EmbedOnly(), EmbeddingProvider)
    assert not isinstance(_EmbedOnly(), LLMProvider)
