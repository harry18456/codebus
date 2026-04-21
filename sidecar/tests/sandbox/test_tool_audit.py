"""Tests for ToolSandbox tool_audit.jsonl wiring — covers Requirements
"ToolSandbox appends every invocation to tool_audit.jsonl",
"Tools declare their auditable field whitelist", and
"Schema version on every tool audit line" from
openspec/changes/sanitizer-safety-chain/specs/tool-sandbox/spec.md.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.sandbox import (
    PathEscapeError,
    ToolContext,
    ToolSandbox,
)


class _ReadFileTool:
    """Minimal tool that returns the bytes of an in-workspace path.

    `path_args` names the single path argument so `ToolSandbox` can
    run `ensure_in_workspace` on it before the body executes.
    """

    name = "read_file"
    audit_fields = ["path"]
    path_args = ["path"]

    def run(self, args: dict, ctx: ToolContext) -> str:
        return f"ok:{args['path']}"


class _SearchTool:
    """Tool that does not expose its free-form `query` to the audit log."""

    name = "search"
    audit_fields = ["path"]
    path_args: list[str] = []

    def run(self, args: dict, ctx: ToolContext) -> str:
        return "searched"


class _NoAuditFieldsTool:
    name = "bad"
    # Intentionally no `audit_fields` attribute.

    def run(self, args: dict, ctx: ToolContext) -> None:
        return None


def _read_lines(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def _ctx(tmp_path: Path) -> ToolContext:
    return ToolContext(
        workspace_root=tmp_path,
        workspace_type="folder",
        workspace_id="ws-1",
        session_id="00000000-0000-4000-8000-000000000000",
    )


def test_tool_audit_successful_invocation_line(tmp_path: Path) -> None:
    """Scenario: Successful invocation writes allowed audit line."""
    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)
    sandbox.register(_ReadFileTool())

    target = tmp_path / "file.txt"
    target.write_text("data", encoding="utf-8")

    result = sandbox.invoke("read_file", {"path": str(target)}, _ctx(tmp_path))
    assert result.startswith("ok:")

    lines = _read_lines(audit_path)
    assert len(lines) == 1
    line = lines[0]
    assert line["tool_name"] == "read_file"
    assert line["allowed"] is True
    assert line["denial_reason"] is None
    assert line["resolved_path"] is not None
    assert line["resolved_path"].endswith("file.txt")


def test_tool_audit_denied_invocation_writes_denial_reason(tmp_path: Path) -> None:
    """Scenario: Denied invocation writes denial audit line."""
    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)
    sandbox.register(_ReadFileTool())

    # Path escape: '..' traversal past workspace root.
    escape = str(tmp_path / ".." / "outside.txt")

    ran: list[bool] = []
    original_run = _ReadFileTool.run

    def spy_run(self, args, ctx):  # type: ignore[no-untyped-def]
        ran.append(True)
        return original_run(self, args, ctx)

    _ReadFileTool.run = spy_run  # type: ignore[assignment]
    try:
        with pytest.raises(PathEscapeError):
            sandbox.invoke("read_file", {"path": escape}, _ctx(tmp_path))
    finally:
        _ReadFileTool.run = original_run  # type: ignore[assignment]

    assert ran == []  # body MUST NOT execute
    lines = _read_lines(audit_path)
    assert len(lines) == 1
    line = lines[0]
    assert line["allowed"] is False
    assert line["denial_reason"] in {
        "path_escape",
        "symlink_outside",
        "unc_path",
        "long_path_prefix_invalid",
        "case_variant",
        "trailing_whitespace",
    }


def test_tool_audit_args_summary_excludes_non_whitelisted(tmp_path: Path) -> None:
    """Scenario: args_summary excludes sensitive values."""
    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)
    sandbox.register(_SearchTool())

    sandbox.invoke(
        "search",
        {"path": "src/app.py", "query": "leaked-password-secret"},
        _ctx(tmp_path),
    )

    lines = _read_lines(audit_path)
    assert len(lines) == 1
    args_summary = lines[0]["args_summary"]
    assert args_summary == {"path": "src/app.py"}
    # Sensitive free-form text MUST NOT appear anywhere in the audit line.
    assert "leaked-password-secret" not in json.dumps(lines[0])


def test_tool_without_audit_fields_rejected_at_registration(tmp_path: Path) -> None:
    """Scenario: Tool without audit_fields rejected at registration."""
    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)

    with pytest.raises(ValueError, match="audit_fields"):
        sandbox.register(_NoAuditFieldsTool())


def test_tool_audit_schema_version_equals_1(tmp_path: Path) -> None:
    """Scenario: Initial schema version."""
    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)
    sandbox.register(_SearchTool())

    sandbox.invoke("search", {"path": "src/app.py"}, _ctx(tmp_path))

    lines = _read_lines(audit_path)
    assert len(lines) == 1
    assert lines[0]["schema_version"] == 1


def test_tool_audit_empty_audit_fields_yields_empty_summary(tmp_path: Path) -> None:
    """Scenario: Empty audit_fields permitted — args_summary == {}."""

    class _SilentTool:
        name = "silent"
        audit_fields: list[str] = []
        path_args: list[str] = []

        def run(self, args: dict, ctx: ToolContext) -> None:
            return None

    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)
    sandbox.register(_SilentTool())

    sandbox.invoke("silent", {"x": 1, "y": "z"}, _ctx(tmp_path))

    lines = _read_lines(audit_path)
    assert len(lines) == 1
    assert lines[0]["args_summary"] == {}


def test_tool_audit_line_contains_required_fields(tmp_path: Path) -> None:
    """Scenario: Audit line contains required fields."""
    audit_path = tmp_path / "tool_audit.jsonl"
    sandbox = ToolSandbox(audit_log_path=audit_path)
    sandbox.register(_SearchTool())

    sandbox.invoke("search", {"path": "src/app.py"}, _ctx(tmp_path))

    lines = _read_lines(audit_path)
    required = {
        "ts",
        "schema_version",
        "workspace_type",
        "tool_name",
        "args_summary",
        "resolved_path",
        "allowed",
        "denial_reason",
        "session_id",
    }
    assert set(lines[0]).issuperset(required)
    assert lines[0]["workspace_type"] == "folder"
