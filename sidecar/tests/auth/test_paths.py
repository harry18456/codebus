"""Defensive tests for `auth/paths.py` — App-level audit log path constants.

Covers `authorization-audit` capability scenarios:
- "Filename literal is single-sourced in canonical leaf module"
- helper resolves to ~/.codebus/authorization_audit.jsonl
"""
from __future__ import annotations

import re
from pathlib import Path

from codebus_agent.auth.paths import (
    _APP_AUDIT_HOME_SUBDIR,
    _AUTHORIZATION_AUDIT_FILENAME,
    authorization_audit_path,
)


def test_authorization_audit_path_resolves_under_home_codebus() -> None:
    expected = Path.home() / ".codebus" / "authorization_audit.jsonl"
    assert authorization_audit_path() == expected


def test_constants_carry_expected_string_values() -> None:
    assert _APP_AUDIT_HOME_SUBDIR == ".codebus"
    assert _AUTHORIZATION_AUDIT_FILENAME == "authorization_audit.jsonl"


def test_filename_literal_single_sourced_in_auth_paths_module() -> None:
    """`authorization_audit.jsonl` literal MUST appear only in auth/paths.py.

    Source-grep enforcement of `authorization-audit` capability scenario
    "Filename literal is single-sourced in canonical leaf module".
    """
    src_root = Path(__file__).resolve().parents[2] / "src" / "codebus_agent"
    canonical = src_root / "auth" / "paths.py"
    pattern = re.compile(r"authorization_audit\.jsonl")

    offenders: list[Path] = []
    for py in src_root.rglob("*.py"):
        if py == canonical:
            continue
        if pattern.search(py.read_text(encoding="utf-8")):
            offenders.append(py.relative_to(src_root))

    assert offenders == [], (
        "`authorization_audit.jsonl` literal must live exclusively in "
        f"auth/paths.py; found drift in: {offenders}"
    )
