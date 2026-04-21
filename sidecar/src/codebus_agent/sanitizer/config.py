"""SanitizerConfig — two-tier (workspace / global) YAML loader.

Backs SHALL clauses in
openspec/changes/sanitizer-safety-chain/specs/sanitizer/spec.md
  Requirement: SanitizerConfig loads from workspace-then-global YAML
  Requirement: Config declares allowlist structure without requiring
               non-empty contents
  Requirement: Rules version is recorded on every audit line
    Scenario: Missing rules version rejected at config load

Per Decision "Sanitizer config — 兩層覆蓋 + Pydantic strict 驗證":
- Workspace file REPLACES global (no deep merge)
- `extra="forbid"` so typos raise at load time
- `rules_version` is a required string (format `YYYY-MM-DD-N`, but we do
  not validate the format to allow maintainers to pick bump cadence)
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import Any

import yaml
from pydantic import BaseModel, ConfigDict

_BUILTIN_RULES_VERSION = "2026-04-20-1"


class PatternAllowlistEntry(BaseModel):
    """Single allowlist row — both fields required per spec Scenario
    "Pattern allowlist entry requires reason"."""

    model_config = ConfigDict(extra="forbid")

    pattern: str
    reason: str


class SanitizerConfig(BaseModel):
    """Strict-validated config loaded from workspace-then-global YAML."""

    model_config = ConfigDict(extra="forbid")

    rules_version: str
    path_allowlist: list[str] = []
    filename_allowlist: list[str] = []
    pattern_allowlist: list[PatternAllowlistEntry] = []

    @classmethod
    def load(cls, workspace_root: Path) -> "SanitizerConfig":
        workspace_file = Path(workspace_root) / "sanitizer.local.yaml"
        if workspace_file.is_file():
            return cls._from_yaml_file(workspace_file)

        global_file = _global_config_path()
        if global_file.is_file():
            return cls._from_yaml_file(global_file)

        return cls(rules_version=_BUILTIN_RULES_VERSION)

    @classmethod
    def _from_yaml_file(cls, path: Path) -> "SanitizerConfig":
        with path.open("r", encoding="utf-8") as fp:
            data: Any = yaml.safe_load(fp) or {}
        if not isinstance(data, dict):
            raise ValueError(
                f"sanitizer config at {path} must be a YAML mapping, got {type(data).__name__}"
            )
        return cls.model_validate(data)


def _global_config_path() -> Path:
    home_env = os.environ.get("CODEBUS_HOME")
    if home_env:
        return Path(home_env) / ".codebus" / "sanitizer.local.yaml"
    return Path.home() / ".codebus" / "sanitizer.local.yaml"
