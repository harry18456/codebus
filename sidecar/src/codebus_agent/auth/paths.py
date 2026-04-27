"""App-level audit log path constants — sister leaf to `_audit_paths.py`.

`auth-flow` introduces the seventh audit layer
``~/.codebus/authorization_audit.jsonl`` (App-level, cross-workspace).
Its filename and home-relative subdir live here as the *single source*
of truth so callers never sprinkle the literal string across the
codebase. Mirrors the workspace-level convention enforced by
``codebus_agent._audit_paths`` for the six workspace audit files (see
that module's docstring; it explicitly notes App-level
``authorization_audit.jsonl`` was deferred to a future capability —
this is that capability).

Defensive test ``tests/auth/test_paths.py`` greps the entire
``codebus_agent`` source tree to enforce the single-source invariant
(scenario "Filename literal is single-sourced in canonical leaf module"
of the ``authorization-audit`` capability).
"""
from __future__ import annotations

from pathlib import Path

__all__ = [
    "_APP_AUDIT_HOME_SUBDIR",
    "_AUTHORIZATION_AUDIT_FILENAME",
    "authorization_audit_path",
]


_APP_AUDIT_HOME_SUBDIR = ".codebus"
_AUTHORIZATION_AUDIT_FILENAME = "authorization_audit.jsonl"


def authorization_audit_path() -> Path:
    """Resolve the App-level authorization audit log path.

    Returns ``<user_home>/.codebus/authorization_audit.jsonl``. The
    parent directory is *not* created here — that is
    ``AuthorizationAuditLogger`` constructor's responsibility (mirrors
    ``KBGrowthLogger``'s auto-mkdir convention; see capability spec
    scenario "Logger constructor auto-creates parent directory").
    """
    return Path.home() / _APP_AUDIT_HOME_SUBDIR / _AUTHORIZATION_AUDIT_FILENAME
