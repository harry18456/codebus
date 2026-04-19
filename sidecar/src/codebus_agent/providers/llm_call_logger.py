"""LLMCallLogger — append-only `llm_calls.jsonl` writer.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: LLMCallLogger writes llm_calls.jsonl
    Scenario: Request and response captured
    Scenario: Sanitizer-ready field reserved
    Scenario: Failure still logged

Implements `docs/decisions.md` D-022 (LLM Call Inspector audit trail).
The `sanitizer_pass2_applied` field is reserved for the Sanitizer
Pass 2 layer that future changes will wire in; during M1 it is
always ``false``.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


class LLMCallLogger:
    def __init__(self, path: Path | str) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def log(
        self,
        *,
        request: dict[str, Any],
        response: dict[str, Any] | None,
        sanitizer_pass2_applied: bool = False,
    ) -> None:
        entry = self._base_entry(request=request, sanitizer_pass2_applied=sanitizer_pass2_applied)
        entry["response"] = response
        self._append(entry)

    def log_failure(
        self,
        *,
        request: dict[str, Any],
        exception: BaseException,
        sanitizer_pass2_applied: bool = False,
    ) -> None:
        entry = self._base_entry(request=request, sanitizer_pass2_applied=sanitizer_pass2_applied)
        entry["response"] = None
        entry["error"] = {
            "class": type(exception).__name__,
            "message": str(exception),
        }
        self._append(entry)

    def _base_entry(
        self, *, request: dict[str, Any], sanitizer_pass2_applied: bool
    ) -> dict[str, Any]:
        return {
            "timestamp": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
            "request": request,
            "sanitizer_pass2_applied": bool(sanitizer_pass2_applied),
        }

    def _append(self, entry: dict[str, Any]) -> None:
        with self.path.open("a", encoding="utf-8") as fp:
            fp.write(json.dumps(entry, ensure_ascii=False, default=str) + "\n")
