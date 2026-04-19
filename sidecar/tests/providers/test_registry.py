"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: No outbound LLM traffic during M1
    Scenario: Only MockProvider registered
    Scenario: Integration test asserts no outbound calls
"""
from __future__ import annotations

import pytest
from pydantic import BaseModel

from codebus_agent.providers.mock import MockProvider
from codebus_agent.providers.protocol import Message
from codebus_agent.providers.registry import ProviderRegistry, ProviderRegistryError


class _WouldBeOpenAI:
    """Stand-in for OpenAI / Anthropic / Gemini / Ollama adapters."""

    name = "openai-fake"

    async def chat(self, messages, *, response_model):
        raise AssertionError("should never be called in M1")

    async def embed(self, texts):
        raise AssertionError("should never be called in M1")


class _Echo(BaseModel):
    content: str = ""


def test_registry_accepts_mock_provider() -> None:
    """Scenario: Only MockProvider registered (positive case)."""
    registry = ProviderRegistry()
    registry.register(MockProvider())
    assert "mock" in registry.names


def test_registry_rejects_non_mock_provider_class() -> None:
    """Scenario: Only MockProvider registered (negative case)."""
    registry = ProviderRegistry()
    with pytest.raises(ProviderRegistryError):
        registry.register(_WouldBeOpenAI())


def test_registry_get_returns_registered_provider() -> None:
    registry = ProviderRegistry()
    provider = MockProvider()
    registry.register(provider)
    assert registry.get("mock") is provider


@pytest.mark.asyncio
async def test_no_outbound_http_during_mock_provider_calls(
    block_outbound_sockets: list,
) -> None:
    """Scenario: Integration test asserts no outbound calls.

    Runs the full MockProvider surface with a socket.connect guard
    that records any non-loopback attempt.  The list must stay empty.
    """
    registry = ProviderRegistry()
    registry.register(MockProvider())
    provider = registry.get("mock")

    await provider.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Echo,
    )
    await provider.embed(texts=["a", "b", "c"])

    assert block_outbound_sockets == [], (
        f"unexpected outbound connections: {block_outbound_sockets!r}"
    )
