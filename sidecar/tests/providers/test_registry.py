"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: No outbound LLM traffic during M1
    Scenario: Only MockProvider registered
    Scenario: Integration test asserts no outbound calls

and openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Direct provider use forbidden
    Scenario: Skipping wrapper emits test failure
"""
from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider
from codebus_agent.providers.protocol import Message
from codebus_agent.providers.registry import ProviderRegistry, ProviderRegistryError
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker


class _WouldBeOpenAI:
    """Stand-in for OpenAI / Anthropic / Gemini / Ollama adapters."""

    name = "openai-fake"

    async def chat(self, messages, *, response_model):
        raise AssertionError("should never be called in M1")

    async def embed(self, texts):
        raise AssertionError("should never be called in M1")


class _Echo(BaseModel):
    content: str = ""


def _wrap(tmp_path: Path, inner: object | None = None) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    return TrackedProvider(
        inner or MockProvider(), tracker=tracker, logger=logger
    )


def test_registry_accepts_tracked_provider(tmp_path: Path) -> None:
    """Scenario: Direct provider use forbidden — only TrackedProvider allowed."""
    registry = ProviderRegistry()
    registry.register(_wrap(tmp_path))
    assert "mock" in registry.names


def test_registry_rejects_raw_mock_provider() -> None:
    """Scenario: Skipping wrapper emits test failure (raw MockProvider blocked)."""
    registry = ProviderRegistry()
    with pytest.raises(ProviderRegistryError):
        registry.register(MockProvider())


def test_tracked_rejects_non_mock_inner_provider(tmp_path: Path) -> None:
    """Scenario: Only MockProvider registered — fake inner raises at wrap time.

    TrackedProvider's `ALLOWED_INNER_TYPES` guard fires first, so the
    unwrapped path is unreachable from production code even before the
    registry gets a chance to look.
    """
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    with pytest.raises(TypeError, match="MockProvider"):
        TrackedProvider(_WouldBeOpenAI(), tracker=tracker, logger=logger)


def test_registry_get_returns_registered_provider(tmp_path: Path) -> None:
    registry = ProviderRegistry()
    wrapped = _wrap(tmp_path)
    registry.register(wrapped)
    assert registry.get("mock") is wrapped


@pytest.mark.asyncio
async def test_no_outbound_http_during_tracked_mock_provider_calls(
    tmp_path: Path,
    block_outbound_sockets: list,
) -> None:
    """Scenario: Integration test asserts no outbound calls."""
    registry = ProviderRegistry()
    registry.register(_wrap(tmp_path))
    provider = registry.get("mock")

    await provider.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Echo,
    )
    await provider.embed(texts=["a", "b", "c"])

    assert block_outbound_sockets == [], (
        f"unexpected outbound connections: {block_outbound_sockets!r}"
    )
