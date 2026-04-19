"""LLMProvider Protocol + shared dataclasses.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: LLMProvider protocol
    Scenario: Protocol methods present
    Scenario: Protocol is checkable at type level

The M1 Protocol is a deliberate subset of `docs/llm-provider.md §二`:
only `chat(messages, response_model)` and `embed(texts)` are required,
because M1 exercises the structured-output path end-to-end without
introducing streaming, tool-calling, or multi-provider routing.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Protocol, runtime_checkable

from pydantic import BaseModel


@dataclass
class Message:
    """Single chat message; shape matches `docs/llm-provider.md §二`."""

    role: Literal["system", "user", "assistant", "tool"]
    content: str
    tool_call_id: str | None = None


@dataclass
class Usage:
    """Per-call token / cost accounting — Phase 7 UsageTracker consumes this."""

    call_type: Literal["chat", "embed"]
    model: str
    prompt_tokens: int = 0
    completion_tokens: int = 0
    embed_tokens: int = 0
    cost_usd: float | None = None
    estimated: bool = False


@dataclass
class EmbedResponse:
    """Return value of `LLMProvider.embed`."""

    vectors: list[list[float]]
    usage: Usage


@runtime_checkable
class LLMProvider(Protocol):
    """Structural contract every provider must satisfy.

    The `@runtime_checkable` decorator lets registries use
    `isinstance(provider, LLMProvider)` as a last-line safety net;
    static type checkers enforce the richer generic signatures.
    """

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        """Send `messages` and return a validated instance of `response_model`."""
        ...

    async def embed(self, texts: list[str]) -> EmbedResponse:
        """Return one vector per input text plus usage accounting."""
        ...
