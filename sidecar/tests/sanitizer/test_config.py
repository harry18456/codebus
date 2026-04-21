"""Tests for SanitizerConfig — covers Requirement
"SanitizerConfig loads from workspace-then-global YAML" and
"Config declares allowlist structure without requiring non-empty contents"
plus the Decision "Sanitizer config — 兩層覆蓋 + Pydantic strict 驗證".
"""
from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import ValidationError

from codebus_agent.sanitizer import PatternAllowlistEntry, SanitizerConfig


def _write(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def test_config_load_workspace_replaces_global(tmp_path, monkeypatch):
    """Workspace file must replace global entirely — not deep-merge."""
    home = tmp_path / "home"
    monkeypatch.setenv("CODEBUS_HOME", str(home))
    workspace = tmp_path / "ws"

    _write(
        home / ".codebus" / "sanitizer.local.yaml",
        'rules_version: "global-v1"\npath_allowlist:\n  - "global/**"\n',
    )
    _write(
        workspace / "sanitizer.local.yaml",
        'rules_version: "ws-v1"\npath_allowlist:\n  - "ws/**"\n',
    )

    cfg = SanitizerConfig.load(workspace)

    assert cfg.rules_version == "ws-v1"
    assert cfg.path_allowlist == ["ws/**"]
    assert "global/**" not in cfg.path_allowlist


def test_config_fallback_global_when_workspace_absent(tmp_path, monkeypatch):
    home = tmp_path / "home"
    monkeypatch.setenv("CODEBUS_HOME", str(home))
    workspace = tmp_path / "ws"
    workspace.mkdir()

    _write(
        home / ".codebus" / "sanitizer.local.yaml",
        'rules_version: "global-v1"\nfilename_allowlist:\n  - ".env.example"\n',
    )

    cfg = SanitizerConfig.load(workspace)

    assert cfg.rules_version == "global-v1"
    assert cfg.filename_allowlist == [".env.example"]


def test_config_builtin_defaults_when_neither_file(tmp_path, monkeypatch):
    home = tmp_path / "home"
    monkeypatch.setenv("CODEBUS_HOME", str(home))
    workspace = tmp_path / "ws"
    workspace.mkdir()

    cfg = SanitizerConfig.load(workspace)

    assert isinstance(cfg.rules_version, str)
    assert cfg.rules_version != ""
    assert cfg.path_allowlist == []
    assert cfg.filename_allowlist == []
    assert cfg.pattern_allowlist == []


def test_config_unknown_field_rejected(tmp_path, monkeypatch):
    home = tmp_path / "home"
    monkeypatch.setenv("CODEBUS_HOME", str(home))
    workspace = tmp_path / "ws"

    _write(
        workspace / "sanitizer.local.yaml",
        'rules_version: "ws-v1"\nmystery_field: 42\n',
    )

    with pytest.raises(ValidationError) as exc:
        SanitizerConfig.load(workspace)

    assert "mystery_field" in str(exc.value)


def test_config_missing_rules_version_raises(tmp_path, monkeypatch):
    home = tmp_path / "home"
    monkeypatch.setenv("CODEBUS_HOME", str(home))
    workspace = tmp_path / "ws"

    _write(workspace / "sanitizer.local.yaml", "path_allowlist: []\n")

    with pytest.raises(ValidationError) as exc:
        SanitizerConfig.load(workspace)

    assert "rules_version" in str(exc.value)


def test_pattern_allowlist_entry_requires_reason():
    with pytest.raises(ValidationError) as exc:
        PatternAllowlistEntry(pattern="^FAKE_KEY_")  # type: ignore[call-arg]

    assert "reason" in str(exc.value)


def test_pattern_allowlist_entry_accepts_both_fields():
    entry = PatternAllowlistEntry(pattern="^FAKE_KEY_", reason="test fixture")
    assert entry.pattern == "^FAKE_KEY_"
    assert entry.reason == "test fixture"


def test_config_empty_allowlists_accepted(tmp_path, monkeypatch):
    home = tmp_path / "home"
    monkeypatch.setenv("CODEBUS_HOME", str(home))
    workspace = tmp_path / "ws"

    _write(workspace / "sanitizer.local.yaml", 'rules_version: "v1"\n')

    cfg = SanitizerConfig.load(workspace)

    assert cfg.path_allowlist == []
    assert cfg.filename_allowlist == []
    assert cfg.pattern_allowlist == []
