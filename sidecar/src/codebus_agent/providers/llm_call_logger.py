"""LLMCallLogger — append-only `llm_calls.jsonl` writer.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: LLMCallLogger writes llm_calls.jsonl
    Scenario: Request and response captured
    Scenario: Sanitizer-ready field reserved
    Scenario: Failure still logged

and openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: TrackedProvider records role in audit log
    Scenario: Audit record contains role field
    Scenario: Role field is additive to existing audit schema

Implements `docs/decisions.md` D-022 (LLM Call Inspector audit trail).
The per-call record carries the full wire payload needed by the
O-04 Trust Layer Inspector: role, provider_id, model, token counts,
plus the original request / response.  The `sanitizer_pass2_applied`
field is reserved for Sanitizer Pass 2 (M2); M1 always writes `false`.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .protocol import ProviderRole


class LLMCallLogger:
    def __init__(self, path: Path | str) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

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
