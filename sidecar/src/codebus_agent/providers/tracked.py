"""TrackedProvider — audit-emitting wrapper around an inner `LLMProvider`.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Wrapper preserves protocol shape
    Scenario: Direct provider use forbidden (enforced in registry)
    Scenario: Skipping wrapper emits test failure (enforced in registry)

openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: TrackedProvider records role in audit log

and openspec/changes/sanitizer-safety-chain/specs/llm-provider/spec.md
  Requirement: TrackedProvider applies Sanitizer Pass 2 before dispatch
  Requirement: TrackedProvider writes audit entries to sanitize_audit.jsonl

Every call fans out to both `UsageTracker` (token / cost ledger —
D-021) and `LLMCallLogger` (full wire payload — D-022).  Pre-dispatch,
`SanitizerEngine.sanitize` rewrites every message / text so the
wrapped provider and `llm_calls.jsonl` only ever see redacted
payloads (D-015 Pass 2).  Each replacement also appends to
`sanitize_audit.jsonl` via the injected `SanitizerAuditLogger`.

Design sanitizer-safety-chain §"Pass 2 hook point":
the sanitizer is **required** at construction — a missing engine
raises `ValueError`.  That collapses the registry guard and the
wrapper into a single choke point: nothing can reach the inner
provider without Pass 2 applied.
"""
from __future__ import annotations

import json
import uuid
from dataclasses import asdict, is_dataclass
from typing import TYPE_CHECKING, Any, ClassVar

from pydantic import BaseModel

from ..sanitizer import (
    MessageSource,
    SanitizerAuditLogger,
    SanitizerEngine,
)
from .llm_call_logger import LLMCallLogger
from .mock import MockProvider
from .openai_chat import OpenAIChatProvider
from .openai_embedding import OpenAIEmbeddingProvider
from .protocol import EmbedResponse, Message, ProviderRole
from .usage_tracker import UsageTracker

if TYPE_CHECKING:
    from ..agent.emitter import SSEEmitter


class TrackedProvider:
    """Decorator-style wrapper enforcing audit on every LLM call."""

    # `kb-build-production-wiring` added `OpenAIEmbeddingProvider` per D-032:
    # M2 permits outbound traffic specifically for embeddings.
    # `chat-provider-wiring` extends the allowlist to `OpenAIChatProvider`
    # so chat-ish roles (REASONING / JUDGE / CHAT) can reach OpenAI for
    # Module 4 Explorer (D-012). Future live providers (Ollama, Anthropic)
    # remain gated behind changes that must extend this set explicitly —
    # the `Outbound LLM traffic gated by TrackedProvider whitelist`
    # Requirement in `openspec/specs/llm-provider/spec.md` enumerates the
    # allowlist so spec + code stay in lockstep.
    ALLOWED_INNER_TYPES: ClassVar[frozenset[type]] = frozenset(
        {MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}
    )

    def __init__(
        self,
        inner: Any,
        *,
        tracker: UsageTracker,
        logger: LLMCallLogger,
        role: ProviderRole,
        sanitizer: SanitizerEngine,
        sanitizer_audit: SanitizerAuditLogger,
        rules_version: str,
        default_module: str | None = None,
        emitter: "SSEEmitter | None" = None,
    ) -> None:
        if type(inner) not in self.ALLOWED_INNER_TYPES:
            raise TypeError(
                f"TrackedProvider inner must be one of "
                f"{{{', '.join(t.__name__ for t in self.ALLOWED_INNER_TYPES)}}}; "
                f"got {type(inner).__name__}. Outbound LLM traffic is gated "
                f"by this allowlist — extend ALLOWED_INNER_TYPES (and the "
                f"spec Requirement `Outbound LLM traffic gated by "
                f"TrackedProvider whitelist`) in a new change to add providers."
            )
        if not isinstance(role, ProviderRole):
            raise TypeError(
                f"TrackedProvider role must be a ProviderRole; "
                f"got {type(role).__name__}"
            )
        if sanitizer is None or not isinstance(sanitizer, SanitizerEngine):
            raise ValueError(
                "TrackedProvider requires a SanitizerEngine injection "
                "(sanitizer=...). Pass 2 is non-bypassable — see "
                "openspec/changes/sanitizer-safety-chain/specs/llm-provider/spec.md."
            )
        if sanitizer_audit is None or not isinstance(
            sanitizer_audit, SanitizerAuditLogger
        ):
            raise ValueError(
                "TrackedProvider requires a SanitizerAuditLogger injection "
                "(sanitizer_audit=...). Audit trail must receive every Pass 2 hit."
            )
        if not isinstance(rules_version, str) or not rules_version:
            raise ValueError(
                "TrackedProvider requires a non-empty rules_version string "
                "so every sanitize_audit.jsonl line can reference the rule set in effect."
            )
        self._inner = inner
        self._tracker = tracker
        self._logger = logger
        self._role = role
        self._sanitizer = sanitizer
        self._sanitizer_audit = sanitizer_audit
        self._rules_version = rules_version
        # `usage-tracker-dedup` (Option A): default_module is the SOLE label
        # path into `token_usage.jsonl`. Empty string preserves M1 records'
        # behavior (no module field meant blank). Callers like KB build pass
        # "kb_build" via the wire_kb_dependencies factory so KB no longer
        # needs to call `tracker.record(...)` itself.
        self._default_module: str = default_module or ""
        # agent-sse-wiring: optional SSE channel. Kept `| None` (rather than
        # NullEmitter default) so the wrapper can gate emit calls behind a
        # single `is not None` check — the emit path is already rare compared
        # to the file-only audit writes.
        self._emitter = emitter
        # Local running-total accumulator so `usage_delta.session_total_cost_usd`
        # reflects every TrackedProvider call made on this instance. UsageTracker
        # intentionally stays append-only on disk; the running total lives in
        # memory on the provider because that's the scope SSE consumers care
        # about (per-provider-instance ≈ per-task for Explorer wiring).
        self._session_total_cost_usd: float = 0.0
        self.name: str = getattr(inner, "name", "tracked")

    @property
    def role(self) -> ProviderRole:
        return self._role

    def set_emitter(self, emitter: "SSEEmitter | None") -> None:
        """Late-wire the SSE emitter and propagate to the inner LLMCallLogger.

        Factory code (`wire_kb_dependencies`) builds `TrackedProvider` at
        workspace-scope time, before any per-task `TaskHandle` exists.
        `api/explore.py` calls this after the handle is created so both
        `usage_delta` and `llm_call` events land on the task's SSE channel.
        """
        self._emitter = emitter
        self._logger.set_emitter(emitter)

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        call_id = f"chat_req_{uuid.uuid4()}"
        sanitized_messages = self._sanitize_messages(messages, call_id)

        request = _serialize_chat_request(sanitized_messages, response_model)
        model_id = _chat_model_id(self._inner)
        prompt_tokens = _estimate_tokens(_join_message_text(sanitized_messages))

        try:
            result = await self._inner.chat(
                sanitized_messages, response_model=response_model
            )
        except BaseException as exc:
            self._logger.log_failure(
                request=request,
                exception=exc,
                role=self._role,
                provider_id=self.name,
                model=model_id,
                prompt_tokens=prompt_tokens,
                completion_tokens=0,
                sanitizer_pass2_applied=True,
            )
            raise

        response_payload = _serialize_response(result)
        completion_tokens = _estimate_tokens(
            json.dumps(response_payload, ensure_ascii=False)
        )
        self._logger.log(
            request=request,
            response=response_payload,
            role=self._role,
            provider_id=self.name,
            model=model_id,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            sanitizer_pass2_applied=True,
        )
        self._tracker.record(
            provider=self.name,
            model=model_id,
            operation="chat",
            input_tokens=prompt_tokens,
            output_tokens=completion_tokens,
            cost_usd=0.0,
            module=self._default_module,
        )
        self._emit_usage_delta(
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            cost_usd=0.0,
        )
        return result

    async def embed(self, texts: list[str]) -> EmbedResponse:
        call_id = f"embed_req_{uuid.uuid4()}"
        sanitized_texts = self._sanitize_texts(texts, call_id)

        request = {"texts": list(sanitized_texts)}
        prompt_tokens = sum(len(t) for t in sanitized_texts)

        try:
            result = await self._inner.embed(sanitized_texts)
        except BaseException as exc:
            self._logger.log_failure(
                request=request,
                exception=exc,
                role=self._role,
                provider_id=self.name,
                model=_EMBED_UNKNOWN_MODEL,
                prompt_tokens=prompt_tokens,
                completion_tokens=0,
                sanitizer_pass2_applied=True,
            )
            raise

        response_payload = {
            "vectors": result.vectors,
            "usage": asdict(result.usage),
        }
        self._logger.log(
            request=request,
            response=response_payload,
            role=self._role,
            provider_id=self.name,
            model=result.usage.model,
            prompt_tokens=int(result.usage.embed_tokens),
            completion_tokens=0,
            sanitizer_pass2_applied=True,
        )
        cost = result.usage.cost_usd if result.usage.cost_usd is not None else 0.0
        self._tracker.record(
            provider=self.name,
            model=result.usage.model,
            operation="embed",
            input_tokens=int(result.usage.embed_tokens),
            output_tokens=0,
            cost_usd=cost,
            module=self._default_module,
        )
        self._emit_usage_delta(
            prompt_tokens=int(result.usage.embed_tokens),
            completion_tokens=0,
            cost_usd=cost,
        )
        return result

    def _emit_usage_delta(
        self,
        *,
        prompt_tokens: int,
        completion_tokens: int,
        cost_usd: float,
    ) -> None:
        """Fan out a `usage_delta` SSE event after a successful call.

        No-op when no emitter is wired. `phase` / `step` come from the
        module-level `ContextVar`s so the Explorer loop can scope them per
        iteration without threading values through every call site. Running
        total stays on the provider instance — per-task scope is exactly
        what the Agent console consumer wants.
        """
        if self._emitter is None:
            return
        # Lazy import to avoid a circular: agent.context_vars is pure stdlib.
        from ..agent.context_vars import current_phase, current_step

        self._session_total_cost_usd += float(cost_usd)
        self._emitter.emit(
            {
                "type": "usage_delta",
                "phase": current_phase(),
                "module": self._default_module,
                "step": current_step(),
                "prompt_tokens": int(prompt_tokens),
                "completion_tokens": int(completion_tokens),
                "cost_usd": float(cost_usd),
                "session_total_cost_usd": round(self._session_total_cost_usd, 8),
            }
        )

    def _sanitize_messages(
        self, messages: list[Message], call_id: str
    ) -> list[Message]:
        session_id = str(uuid.uuid4())
        source = MessageSource(message_id=call_id)

        sanitized: list[Message] = []
        for m in messages:
            result = self._sanitizer.sanitize(m.content, source=source)
            for entry in result.entries:
                self._sanitizer_audit.append(
                    entry=entry,
                    pass_num=2,
                    rules_version=self._rules_version,
                    session_id=session_id,
                )
            sanitized.append(
                Message(
                    role=m.role,
                    content=result.text,
                    tool_call_id=m.tool_call_id,
                )
            )
        return sanitized

    def _sanitize_texts(self, texts: list[str], call_id: str) -> list[str]:
        session_id = str(uuid.uuid4())
        source = MessageSource(message_id=call_id)

        sanitized: list[str] = []
        for text in texts:
            result = self._sanitizer.sanitize(text, source=source)
            for entry in result.entries:
                self._sanitizer_audit.append(
                    entry=entry,
                    pass_num=2,
                    rules_version=self._rules_version,
                    session_id=session_id,
                )
            sanitized.append(result.text)
        return sanitized


_EMBED_UNKNOWN_MODEL = "unknown-embed"


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
