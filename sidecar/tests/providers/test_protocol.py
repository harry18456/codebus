"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/llm-provider/spec.md
  Requirement: LLMProvider protocol (MODIFIED — chat-only post-D-033)
    Scenario: Protocol declares only chat
    Scenario: Existing chat-only implementations satisfy narrowed protocol

Note: post-D-033, ``LLMProvider`` no longer declares ``embed`` —
embedding lives on the new ``EmbeddingProvider`` Protocol. The wider
narrowing story (both Protocols + MockProvider satisfying both) is
covered in ``test_protocols_narrowed.py``; this module retains the
M1-era chat conformance tests with their original test names so blame
history stays stable.
"""
from __future__ import annotations

import inspect

from pydantic import BaseModel

from codebus_agent.providers.protocol import (
    LLMProvider,
    Message,
)


class _ChatOnly:
    async def chat(
        self, messages: list[Message], *, response_model: type[BaseModel]
    ) -> BaseModel:
        return response_model()


def test_protocol_declares_chat() -> None:
    """Scenario: Protocol declares only chat (post-D-033 narrowing)."""
    assert hasattr(LLMProvider, "chat")
    # Embed moved out of LLMProvider — see EmbeddingProvider.
    assert not hasattr(LLMProvider, "embed")


def test_chat_signature_has_messages_and_response_model() -> None:
    sig = inspect.signature(LLMProvider.chat)
    assert "messages" in sig.parameters
    assert "response_model" in sig.parameters


def test_runtime_isinstance_accepts_chat_only_class() -> None:
    """Scenario: Existing chat-only implementations satisfy narrowed protocol.

    Pre-D-033 this test asserted the OPPOSITE — a class with only
    ``chat`` was rejected because the union Protocol required both
    methods. Post-D-033 narrowing makes chat-only the canonical shape
    for ``LLMProvider``.
    """
    assert isinstance(_ChatOnly(), LLMProvider)
