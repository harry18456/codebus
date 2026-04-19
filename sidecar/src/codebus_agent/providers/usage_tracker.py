"""UsageTracker — append-only `token_usage.jsonl` writer.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: UsageTracker writes token_usage.jsonl
    Scenario: One line per chat call
    Scenario: Required fields present
    Scenario: Embed calls tracked

Underpins `docs/decisions.md` D-021 (token / cost audit) and
`docs/agent-core.md §十三`.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Literal


class UsageTracker:
    def __init__(self, path: Path | str) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def record(
        self,
        *,
        provider: str,
        model: str,
        operation: Literal["chat", "embed"],
        input_tokens: int,
        output_tokens: int,
        cost_usd: float,
    ) -> None:
        entry = {
            "timestamp": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
            "provider": provider,
            "model": model,
            "operation": operation,
            "input_tokens": int(input_tokens),
            "output_tokens": int(output_tokens),
            "cost_usd": float(cost_usd),
        }
        self._append(entry)

    def _append(self, entry: dict) -> None:
        with self.path.open("a", encoding="utf-8") as fp:
            fp.write(json.dumps(entry, ensure_ascii=False) + "\n")
