"""AuthorizationAuditLogger — append-only writer for the seventh audit layer.

Backs SHALL clauses in
``openspec/changes/auth-flow/specs/authorization-audit/spec.md``:

  Requirement: AuthorizationAuditLogger is the sole writer for the App-level audit log
  Requirement: Three-event audit schema with workspace_type discriminator

Each ``write_grant_*`` call appends exactly one JSON line to the path
provided at construction (typically resolved from
``codebus_agent.auth.paths.authorization_audit_path()``). This class
is the *only* legitimate writer of that path; direct ``open()``
against the audit path is an invariant violation flagged by code
review (capability spec scenario "Direct file open against the audit
path is rejected by review").

Mirrors ``codebus_agent.kb.growth_logger.KBGrowthLogger`` pattern but
exposes three typed methods rather than one — the three event kinds
have disjoint required fields and benefit from compile-time
distinction over a dynamic dispatch (design D-A2).
"""
from __future__ import annotations

import json
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

__all__ = ["AuthorizationAuditLogger"]


class AuthorizationAuditLogger:
    """Append-only JSONL writer for the App-level authorization audit log.

    The constructor SHALL refuse relative paths (capability spec
    scenario "Relative path raises at construction") and SHALL
    auto-create the parent directory ("Logger constructor auto-creates
    parent directory"). The writer holds no persistent file handle;
    each ``write_grant_*`` opens, appends, flushes, and closes under a
    lock so concurrent callers cannot interleave partial JSON.
    """

    def __init__(self, path: Path) -> None:
        path = Path(path)
        if not path.is_absolute():
            raise ValueError(
                f"AuthorizationAuditLogger requires an absolute path; got {path!r}"
            )
        self._path = path
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.Lock()

    @property
    def path(self) -> Path:
        return self._path

    def write_grant_issued(
        self,
        *,
        session_id: str,
        workspace_id: str,
        workspace_type: str,
        workspace_source: dict[str, Any],
        scenario: str,
        scope: dict[str, Any],
        sanitizer_rules_version: str,
        user_ack: list[str],
    ) -> None:
        """Append one ``grant_issued`` line."""
        self._append(
            {
                "ts": _iso_utc_now(),
                "event": "grant_issued",
                "session_id": session_id,
                "workspace_id": workspace_id,
                "workspace_type": workspace_type,
                "workspace_source": dict(workspace_source),
                "scenario": scenario,
                "scope": dict(scope),
                "sanitizer_rules_version": sanitizer_rules_version,
                "user_ack": list(user_ack),
            }
        )

    def write_grant_denied(
        self,
        *,
        session_id: str,
        workspace_type: str,
        workspace_source: dict[str, Any],
        scenario: str,
        reason: str,
    ) -> None:
        """Append one ``grant_denied`` line."""
        self._append(
            {
                "ts": _iso_utc_now(),
                "event": "grant_denied",
                "session_id": session_id,
                "workspace_type": workspace_type,
                "workspace_source": dict(workspace_source),
                "scenario": scenario,
                "reason": reason,
            }
        )

    def write_grant_revoked(
        self,
        *,
        session_id: str,
        workspace_id: str,
        grant_ts: str,
        trigger: str,
    ) -> None:
        """Append one ``grant_revoked`` line."""
        self._append(
            {
                "ts": _iso_utc_now(),
                "event": "grant_revoked",
                "session_id": session_id,
                "workspace_id": workspace_id,
                "grant_ts": grant_ts,
                "trigger": trigger,
            }
        )

    def _append(self, line: dict[str, Any]) -> None:
        payload = json.dumps(line, ensure_ascii=False) + "\n"
        with self._lock:
            with self._path.open("a", encoding="utf-8", newline="") as fp:
                fp.write(payload)


def _iso_utc_now() -> str:
    """ISO 8601 timestamp with UTC offset (matches kb-growth ts pattern)."""
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")
