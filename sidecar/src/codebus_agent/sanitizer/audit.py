"""SanitizerAuditLogger — append-only `sanitize_audit.jsonl` writer.

Backs SHALL clauses in
openspec/changes/sanitizer-safety-chain/specs/sanitizer/spec.md
  Requirement: SanitizerAuditLogger appends each replacement to JSONL
  Requirement: Rules version is recorded on every audit line

Per Decision "sanitize_audit.jsonl schema — 固定 10 欄位 + `extra`":
each line is a single JSON object with a fixed, append-only schema.
Nothing in the line carries the pre-sanitize value, the sanitized
payload, or context surrounding the match — the audit record is
metadata only, per D-015's "原值不儲存" invariant.

A process-local `threading.Lock` serializes writes so concurrent
threads never interleave partial lines. Cross-process atomicity is not
required by the current spec (the sidecar is a single process).
"""
from __future__ import annotations

import json
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Literal

from .engine import AuditEntry

SCHEMA_VERSION: Literal[1] = 1


class SanitizerAuditLogger:
    """Serialized JSONL writer for Pass 1 / 2 / 3 audit entries."""

    def __init__(self, path: Path | str) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.Lock()

    def append(
        self,
        *,
        entry: AuditEntry,
        pass_num: int,
        rules_version: str,
        session_id: str,
    ) -> None:
        if pass_num not in (1, 2, 3):
            raise ValueError(
                f"pass_num must be 1, 2, or 3; got {pass_num!r}"
            )
        line = {
            "ts": _iso_utc_now(),
            "schema_version": SCHEMA_VERSION,
            "rules_version": rules_version,
            "pass": pass_num,
            "session_id": session_id,
            "source": entry.source,
            "rule_id": entry.rule_id,
            "kind": entry.kind,
            "placeholder_index": entry.placeholder_index,
            "extra": dict(entry.extra),
        }
        payload = json.dumps(line, ensure_ascii=False) + "\n"
        with self._lock:
            with self.path.open("a", encoding="utf-8") as fp:
                fp.write(payload)


def _iso_utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")
