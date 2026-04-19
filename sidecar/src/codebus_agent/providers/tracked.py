"""TrackedProvider — audit-emitting wrapper around an inner `LLMProvider`.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Wrapper preserves protocol shape
    Scenario: Direct provider use forbidden (enforced in registry)
    Scenario: Skipping wrapper emits test failure (enforced in registry)

Every call fans out to both `UsageTracker` (token / cost ledger —
D-021) and `LLMCallLogger` (full request / response capture — D-022).
Exceptions from the inner provider are captured as failure lines and
re-raised so callers see the original error.

M1 constrains the inner type to `MockProvider`; real provider
adapters arrive in a later change, together with the Sanitizer
Pass 2 flag flip referenced in `llm_call_logger.py`.
"""
from __future__ import annotations

import json
from dataclasses import asdict, is_dataclass
from typing import Any, ClassVar

from pydantic import BaseModel

from .llm_call_logger import LLMCallLogger
from .mock import MockProvider
from .protocol import EmbedResponse, Message
from .usage_tracker import UsageTracker


class TrackedProvider:
    """Decorator-style wrapper enforcing audit on every LLM call."""

    ALLOWED_INNER_TYPES: ClassVar[frozenset[type]] = frozenset({MockProvider})

    def __init__(
        self,
        inner: Any,
        *,
        tracker: UsageTracker,
        logger: LLMCallLogger,
    ) -> None:
        if type(inner) not in self.ALLOWED_INNER_TYPES:
            raise TypeError(
                f"TrackedProvider inner must be one of "
                f"{{{', '.join(t.__name__ for t in self.ALLOWED_INNER_TYPES)}}}; "
                f"got {type(inner).__name__}. No outbound LLM traffic during M1."
            )
        self._inner = inner
        self._tracker = tracker
        self._logger = logger
        self.name: str = getattr(inner, "name", "tracked")

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        request = _serialize_chat_request(messages, response_model)
        try:
            result = await self._inner.chat(messages, response_model=response_model)
        except BaseException as exc:
            self._logger.log_failure(request=request, exception=exc)
            raise

        response_payload = _serialize_response(result)
        self._logger.log(request=request, response=response_payload)
        self._tracker.record(
            provider=self.name,
            model=_chat_model_id(self._inner),
            operation="chat",
            input_tokens=_estimate_tokens(_join_message_text(messages)),
            output_tokens=_estimate_tokens(json.dumps(response_payload, ensure_ascii=False)),
            cost_usd=0.0,
        )
        return result

    async def embed(self, texts: list[str]) -> EmbedResponse:
        request = {"texts": list(texts)}
        try:
            result = await self._inner.embed(texts)
        except BaseException as exc:
            self._logger.log_failure(request=request, exception=exc)
            raise

        response_payload = {
            "vectors": result.vectors,
            "usage": asdict(result.usage),
        }
        self._logger.log(request=request, response=response_payload)
        cost = result.usage.cost_usd if result.usage.cost_usd is not None else 0.0
        self._tracker.record(
            provider=self.name,
            model=result.usage.model,
            operation="embed",
            input_tokens=int(result.usage.embed_tokens),
            output_tokens=0,
            cost_usd=cost,
        )
        return result


def _serialize_chat_request(
    messages: list[Message], response_model: type[BaseModel]
) -> dict[str, Any]:
    return {
        "messages": [asdict(m) for m in messages],
        "response_model": response_model.__name__,
        "response_schema": response_model.model_json_schema(),
    }


def _serialize_response(result: Any) -> dict[str, Any]:
    if isinstance(result, BaseModel):
        return result.model_dump(mode="json")
    if is_dataclass(result):
        return asdict(result)
    return {"value": result}


def _join_message_text(messages: list[Message]) -> str:
    return "\n".join(m.content for m in messages)


def _estimate_tokens(text: str) -> int:
    """Cheap heuristic (≈ 4 chars per token) — D-021 allows estimated=True."""
    return max(1, len(text) // 4)


def _chat_model_id(inner: Any) -> str:
    return f"{getattr(inner, 'name', 'unknown')}-chat-v1"
