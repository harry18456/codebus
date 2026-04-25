"""``generator_log.jsonl`` operational log writer.

Backs Requirement
``Degraded fallback writes per-station stub after retry exhaustion``
(log-write segment) in
`openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Per-Module operational log (parallel to ``reasoning_log.jsonl``), not
part of the seven-layer audit chain. Auto-mkdirs the parent directory
on construction (mirrors ``UsageTracker`` / ``LLMCallLogger`` pattern;
``ReasoningLogger`` is the deliberate exception that requires caller
mkdir per the agent-core spec).
"""
from __future__ import annotations

import json
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

__all__ = ["GeneratorLogger"]


class GeneratorLogger:
    """Append-only JSONL writer for generator operational events."""

    def __init__(self, path: Path | str) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.Lock()

    def append(self, *, event: str, **fields: Any) -> None:
        """Append one event line.

        ``event`` is a stable identifier (``"degraded"``,
        ``"write_failed"``, etc.); additional kwargs are merged into the
        line. ``timestamp`` is auto-populated with UTC ISO-8601.
        """
        entry: dict[str, Any] = {
            "timestamp": _iso_utc_now(),
            "event": event,
        }
        entry.update(fields)
        line = json.dumps(entry, ensure_ascii=False) + "\n"
        with self._lock:
            with self.path.open("a", encoding="utf-8") as fp:
                fp.write(line)


def _iso_utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")
