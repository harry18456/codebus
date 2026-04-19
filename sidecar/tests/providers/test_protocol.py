"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: LLMProvider protocol
    Scenario: Protocol methods present
    Scenario: Protocol is checkable at type level
"""
from __future__ import annotations

import inspect

from pydantic import BaseModel

from codebus_agent.providers.protocol import (
    EmbedResponse,
    LLMProvider,
    Message,
    Usage,
)


class _Echo(BaseModel):
    content: str = ""


class _Conforming:
    async def chat(
        self, messages: list[Message], *, response_model: type[BaseModel]
    ) -> BaseModel:
        return response_model()

    async def embed(self, texts: list[str]) -> EmbedResponse:
        return EmbedResponse(
            vectors=[], usage=Usage(call_type="embed", model="stub")
        )


class _Partial:
    async def chat(
        self, messages: list[Message], *, response_model: type[BaseModel]
    ) -> BaseModel:
        return response_model()

    # embed missing on purpose


def test_protocol_declares_chat_and_embed() -> None:
    """Scenario: Protocol methods present."""
    assert hasattr(LLMProvider, "chat")
    assert hasattr(LLMProvider, "embed")


def test_chat_signature_has_messages_and_response_model() -> None:
    sig = inspect.signature(LLMProvider.chat)
    assert "messages" in sig.parameters
    assert "response_model" in sig.parameters


def test_embed_signature_has_texts() -> None:
    sig = inspect.signature(LLMProvider.embed)
    assert "texts" in sig.parameters


def test_runtime_isinstance_accepts_conforming_class() -> None:
    """Scenario: Protocol is checkable at type level."""
    assert isinstance(_Conforming(), LLMProvider)


def test_runtime_isinstance_rejects_partial_class() -> None:
    assert not isinstance(_Partial(), LLMProvider)
