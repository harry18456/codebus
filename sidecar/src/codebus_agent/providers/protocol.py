"""LLMProvider Protocol + shared dataclasses.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: LLMProvider protocol
    Scenario: Protocol methods present
    Scenario: Protocol is checkable at type level

and openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: ProviderRole enumerates call-site categories
    Scenario: ProviderRole exposes four members
    Scenario: ProviderRole is a StrEnum
  Requirement: RoleConfig binds provider, model, and default parameters per role
    Scenario: RoleConfig exposes required fields
    Scenario: RoleConfig is frozen

The M1 Protocol is a deliberate subset of `docs/llm-provider.md §二`:
only `chat(messages, response_model)` and `embed(texts)` are required,
because M1 exercises the structured-output path end-to-end without
introducing streaming, tool-calling, or multi-provider routing.

`ProviderRole` + `RoleConfig` land the llm-role-routing change
(D-003 follow-up): role dispatch replaces the flat chat/embed split
before Sanitizer Pass 2 wires its pre-flight hook.
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import Literal, Protocol, runtime_checkable

from pydantic import BaseModel


class ProviderRole(StrEnum):
    """Semantic call-site category — see llm-role-routing design §1.

    Exactly four members; vision / multimodal is intentionally out of
    scope per D-028 and remains an additive future extension.
    """

    REASONING = "reasoning"
    JUDGE = "judge"
    CHAT = "chat"
    EMBED = "embed"


@dataclass(frozen=True)
class RoleConfig:
    """Binds a `ProviderRole` to a concrete provider + default params.

    Defaults reflect llm-role-routing design §2: `temperature=0.2`
    covers chat-like roles conservatively; `max_tokens=None` defers to
    the underlying provider's own default until a caller opts in.
    """

    provider_id: str
    model: str
    temperature: float = 0.2
    max_tokens: int | None = None


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
    """Chat-shaped Provider Protocol — D-033 narrowing.

    Backs `LLMProvider protocol` Requirement (post-D-033). The Protocol
    declares ONLY `chat`; the embedding call shape lives in
    :class:`EmbeddingProvider`. This split makes
    `@runtime_checkable` usable for actual LLM-only inners (e.g.,
    :class:`codebus_agent.providers.openai_chat.OpenAIChatProvider`,
    which has no `embed` method) and removes the M1-era awkwardness
    where every concrete class had to declare both methods to satisfy
    the union Protocol.
    """

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        """Send `messages` and return a validated instance of `response_model`."""
        ...


@runtime_checkable
class EmbeddingProvider(Protocol):
    """Embed-shaped Provider Protocol — D-033 introduces this split.

    Backs `EmbeddingProvider protocol` Requirement. The Protocol
    declares ONLY `embed`; chat-shaped calls live in :class:`LLMProvider`.

    `EmbedResponse` retains its M1 shape (`vectors`, `usage`) so existing
    embedding implementations satisfy the new Protocol with no signature
    change.
    """

    async def embed(self, texts: list[str]) -> EmbedResponse:
        """Return one vector per input text plus usage accounting."""
        ...
