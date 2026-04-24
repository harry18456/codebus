"""LLMCallLogger — append-only `llm_calls.jsonl` writer + optional SSE emit.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: LLMCallLogger writes llm_calls.jsonl
    Scenario: Request and response captured
    Scenario: Sanitizer-ready field reserved
    Scenario: Failure still logged

and openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: TrackedProvider records role in audit log

and openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: LLMCallLogger emits llm_call event carrying preview
    (optional `emitter` kwarg; `preview` truncated to 200 chars drawn
     from the first user message's content.)

Implements `docs/decisions.md` D-022 (LLM Call Inspector audit trail).
The per-call record carries the full wire payload needed by the
O-04 Trust Layer Inspector: role, provider_id, model, token counts,
plus the original request / response.  The `sanitizer_pass2_applied`
field is reserved for Sanitizer Pass 2 (M2); pre-sanitizer-safety-chain
call sites always write `false`.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .protocol import ProviderRole

if TYPE_CHECKING:
    from ..agent.emitter import SSEEmitter


_PREVIEW_TRUNCATE_LIMIT: int = 200


class LLMCallLogger:
    def __init__(
        self,
        path: Path | str,
        *,
        emitter: "SSEEmitter | None" = None,
    ) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._emitter = emitter

    def set_emitter(self, emitter: "SSEEmitter | None") -> None:
        """Late-wire the SSE emitter after construction.

        The KB / Explorer factories build `LLMCallLogger` at workspace-scope
        time (before the per-task `TaskHandle` exists). The endpoint layer
        calls this once the handle is created so `llm_call` events reach
        the task-specific subscriber queue.
        """
        self._emitter = emitter

    def log(
        self,
        *,
        request: dict[str, Any],
        response: dict[str, Any] | None,
        role: ProviderRole,
        provider_id: str,
        model: str,
        prompt_tokens: int,
        completion_tokens: int,
        sanitizer_pass2_applied: bool = False,
        module: str = "",
        request_id: str | None = None,
        call_type: str = "chat",
        cost_usd: float | None = None,
        latency_ms: int | None = None,
    ) -> None:
        entry = self._base_entry(
            request=request,
            role=role,
            provider_id=provider_id,
            model=model,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            sanitizer_pass2_applied=sanitizer_pass2_applied,
        )
        entry["response"] = response
        self._append(entry)
        self._emit_llm_call(
            request=request,
            role=role,
            provider_id=provider_id,
            model=model,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            module=module,
            request_id=request_id,
            call_type=call_type,
            cost_usd=cost_usd,
            latency_ms=latency_ms,
        )

    def log_failure(
        self,
        *,
        request: dict[str, Any],
        exception: BaseException,
        role: ProviderRole,
        provider_id: str,
        model: str,
        prompt_tokens: int = 0,
        completion_tokens: int = 0,
        sanitizer_pass2_applied: bool = False,
        module: str = "",
        request_id: str | None = None,
        call_type: str = "chat",
        cost_usd: float | None = None,
        latency_ms: int | None = None,
    ) -> None:
        entry = self._base_entry(
            request=request,
            role=role,
            provider_id=provider_id,
            model=model,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            sanitizer_pass2_applied=sanitizer_pass2_applied,
        )
        entry["response"] = None
        entry["error"] = {
            "class": type(exception).__name__,
            "message": str(exception),
        }
        self._append(entry)
        self._emit_llm_call(
            request=request,
            role=role,
            provider_id=provider_id,
            model=model,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            module=module,
            request_id=request_id,
            call_type=call_type,
            cost_usd=cost_usd,
            latency_ms=latency_ms,
        )

    def _base_entry(
        self,
        *,
        request: dict[str, Any],
        role: ProviderRole,
        provider_id: str,
        model: str,
        prompt_tokens: int,
        completion_tokens: int,
        sanitizer_pass2_applied: bool,
    ) -> dict[str, Any]:
        return {
            "timestamp": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
            "role": role.value,
            "provider_id": provider_id,
            "model": model,
            "prompt_tokens": int(prompt_tokens),
            "completion_tokens": int(completion_tokens),
            "sanitizer_pass2_applied": bool(sanitizer_pass2_applied),
            "request": request,
        }

    def _append(self, entry: dict[str, Any]) -> None:
        with self.path.open("a", encoding="utf-8") as fp:
            fp.write(json.dumps(entry, ensure_ascii=False, default=str) + "\n")

    def _emit_llm_call(
        self,
        *,
        request: dict[str, Any],
        role: ProviderRole,
        provider_id: str,
        model: str,
        prompt_tokens: int,
        completion_tokens: int,
        module: str,
        request_id: str | None,
        call_type: str,
        cost_usd: float | None,
        latency_ms: int | None,
    ) -> None:
        """Fan out an `llm_call` SSE event when an emitter is wired.

        `preview` is drawn from the first `role=="user"` message in
        `request["messages"]` and truncated to 200 chars so large prompts
        don't flood the SSE channel (wire-log parity in `llm_calls.jsonl`
        keeps the full payload for detail endpoints to serve on demand).
        """
        if self._emitter is None:
            return
        from ..agent.context_vars import current_step

        preview = _extract_first_user_preview(request)
        self._emitter.emit(
            {
                "type": "llm_call",
                "request_id": request_id or "",
                "module": module,
                "step_id": current_step(),
                "role": role.value,
                "provider": provider_id,
                "model": model,
                "call_type": call_type,
                "latency_ms": latency_ms,
                "tokens": {
                    "prompt": int(prompt_tokens),
                    "completion": int(completion_tokens),
                },
                "cost_usd": cost_usd,
                "preview": preview,
            }
        )


def _extract_first_user_preview(request: dict[str, Any]) -> str:
    """Return the first user-role message content, truncated to 200 chars.

    Defensive: callers may have exotic `request` shapes (e.g. embed requests
    carry `texts` instead of `messages`). When no user message exists, the
    preview falls back to an empty string — consumers treat `""` as "nothing
    to preview".
    """
    messages = request.get("messages") if isinstance(request, dict) else None
    if not isinstance(messages, list):
        return ""
    for m in messages:
        if isinstance(m, dict) and m.get("role") == "user":
            content = m.get("content")
            if isinstance(content, str):
                return content[:_PREVIEW_TRUNCATE_LIMIT]
    return ""
