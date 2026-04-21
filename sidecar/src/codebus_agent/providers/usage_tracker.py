"""UsageTracker — append-only `token_usage.jsonl` writer.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: UsageTracker writes token_usage.jsonl
    Scenario: One line per chat call
    Scenario: Required fields present
    Scenario: Embed calls tracked

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: Embedding batch pipeline with UsageTracker wiring
    — adds `record(usage=..., module="kb_build")` form so KB builder can
    tag every batch with the call-site label.

Underpins `docs/decisions.md` D-021 (token / cost audit) and
`docs/agent-core.md §十三`.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Literal

from codebus_agent.providers.protocol import Usage


class UsageTracker:
    def __init__(self, path: Path | str) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def record(
        self,
        *,
        usage: Usage | None = None,
        module: str | None = None,
        provider: str | None = None,
        model: str | None = None,
        operation: Literal["chat", "embed"] | None = None,
        input_tokens: int | None = None,
        output_tokens: int | None = None,
        cost_usd: float | None = None,
    ) -> None:
        """Append one usage entry to `token_usage.jsonl`.

        Two call shapes are supported:

        - **TrackedProvider (M1)**: explicit kwargs `provider, model,
          operation, input_tokens, output_tokens, cost_usd` — no `usage`,
          no `module`.
        - **KnowledgeBase (M2)**: `usage=Usage(...), module="kb_build"`
          — derives provider/operation/tokens/cost from the `Usage`
          dataclass. The `module` label distinguishes KB-side calls from
          chat / agent calls in audit aggregation.
        """
        if usage is not None:
            input_t = int(
                usage.embed_tokens
                if usage.call_type == "embed"
                else usage.prompt_tokens
            )
            output_t = int(
                0 if usage.call_type == "embed" else usage.completion_tokens
            )
            entry: dict = {
                "timestamp": datetime.now(timezone.utc).isoformat(
                    timespec="milliseconds"
                ),
                "provider": provider or "unknown",
                "model": usage.model,
                "operation": usage.call_type,
                "input_tokens": input_t,
                "output_tokens": output_t,
                "cost_usd": float(usage.cost_usd or 0.0),
                "module": module,
            }
        else:
            if (
                provider is None
                or model is None
                or operation is None
                or input_tokens is None
                or output_tokens is None
                or cost_usd is None
            ):
                raise TypeError(
                    "UsageTracker.record requires either `usage=...` or the "
                    "full legacy kwargs (provider/model/operation/input_tokens/"
                    "output_tokens/cost_usd)"
                )
            entry = {
                "timestamp": datetime.now(timezone.utc).isoformat(
                    timespec="milliseconds"
                ),
                "provider": provider,
                "model": model,
                "operation": operation,
                "input_tokens": int(input_tokens),
                "output_tokens": int(output_tokens),
                "cost_usd": float(cost_usd),
                "module": module,
            }
        self._append(entry)

    def _append(self, entry: dict) -> None:
        with self.path.open("a", encoding="utf-8") as fp:
            fp.write(json.dumps(entry, ensure_ascii=False) + "\n")
