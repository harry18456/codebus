"""ReasoningLogger — append-only JSONL writer for the fourth audit layer.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ReasoningLogger appends one JSONL line per Step to workspace path

Each ``write(step)`` call appends exactly one line to
``{workspace}/reasoning_log.jsonl`` (canonical path; the caller
controls what `path` is). The writer rewrites ``explorer_prompt_version``
/ ``judge_prompt_version`` on the `Step` so every line captures the
prompt revision in play — this is the contract golden-sample replay
relies on (``docs/agent-core.md §八``).

P0 scope explicitly excludes SSE emit: the ``agent-sse-wiring`` change
(step 22 in the implementation plan) will inject an ``SSEEmitter`` so
``agent_thought`` / ``judge_verdict`` / ``action_result`` events reach
the frontend Agent console without re-touching this writer.
"""
from __future__ import annotations

from pathlib import Path

from .prompts import EXPLORER_PROMPT_VERSION, JUDGE_PROMPT_VERSION
from .types import Step


__all__ = ["ReasoningLogger"]


class ReasoningLogger:
    """Append-only JSONL writer tied to a single workspace path."""

    def __init__(self, path: Path) -> None:
        # Caller is responsible for path-safety (workspace-root containment
        # + parent-dir creation). The spec scenario `Path stays under
        # workspace` says the logger MAY rely on the precondition and
        # perform no additional path check.
        self._path = Path(path)

    @property
    def path(self) -> Path:
        return self._path

    def write(self, step: Step) -> None:
        """Append one JSON line. Errors propagate — silent drops are forbidden."""
        stamped = step.model_copy(
            update={
                "explorer_prompt_version": EXPLORER_PROMPT_VERSION,
                "judge_prompt_version": JUDGE_PROMPT_VERSION,
            }
        )
        line = stamped.model_dump_json()
        with self._path.open("a", encoding="utf-8") as fh:
            fh.write(line + "\n")
