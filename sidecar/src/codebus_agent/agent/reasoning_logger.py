"""ReasoningLogger — append-only JSONL writer for the fourth audit layer.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ReasoningLogger appends one JSONL line per Step to workspace path

openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A system prompt module is isolated from Explorer prompts
    Scenario: ReasoningLogger stamps qa_prompt_version on every Q&A step

Each ``write(step)`` call appends exactly one line to
``{workspace}/reasoning_log.jsonl`` (canonical path; the caller
controls what `path` is). The writer rewrites prompt-version fields on
the `Step` so every line captures the prompt revision in play.

`mode` parameter:
- ``"explorer"`` (default): stamp ``explorer_prompt_version`` +
  ``judge_prompt_version``; this is the legacy behaviour used by Module
  4 Explorer.
- ``"qa"``: stamp ``qa_prompt_version`` only; the explorer / judge
  version fields are excluded from the serialized line so the Q&A
  audit trail is unambiguously attributed to its own prompt revision.

P0 scope explicitly excludes SSE emit: the ``agent-sse-wiring`` change
(step 22 in the implementation plan) wired SSEEmitter into the
TrackedProvider / Judge / Coverage call seams without re-touching this
writer.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Literal

from .prompts import EXPLORER_PROMPT_VERSION, JUDGE_PROMPT_VERSION, QA_PROMPT_VERSION
from .types import Step


__all__ = ["ReasoningLogger"]


_LoggerMode = Literal["explorer", "qa"]


class ReasoningLogger:
    """Append-only JSONL writer tied to a single workspace path."""

    def __init__(self, path: Path, *, mode: _LoggerMode = "explorer") -> None:
        # Caller is responsible for path-safety (workspace-root containment
        # + parent-dir creation). The spec scenario `Path stays under
        # workspace` says the logger MAY rely on the precondition and
        # perform no additional path check.
        self._path = Path(path)
        self._mode: _LoggerMode = mode

    @property
    def path(self) -> Path:
        return self._path

    @property
    def mode(self) -> _LoggerMode:
        return self._mode

    def write(self, step: Step) -> None:
        """Append one JSON line. Errors propagate — silent drops are forbidden."""
        if self._mode == "qa":
            data = step.model_dump(
                mode="json",
                exclude={"explorer_prompt_version", "judge_prompt_version", "qa_prompt_version"},
            )
            data["qa_prompt_version"] = QA_PROMPT_VERSION
            line = json.dumps(data, ensure_ascii=False, default=str)
        else:
            stamped = step.model_copy(
                update={
                    "explorer_prompt_version": EXPLORER_PROMPT_VERSION,
                    "judge_prompt_version": JUDGE_PROMPT_VERSION,
                }
            )
            line = stamped.model_dump_json()
        with self._path.open("a", encoding="utf-8") as fh:
            fh.write(line + "\n")
