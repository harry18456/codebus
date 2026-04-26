"""Tests for `QATools` seven-tool surface.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: QATools exposes seven tools with audit_fields declared
"""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock

import pytest

from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.tools.qa_tools import QATools
from codebus_agent.agent.types import ExplorerState
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine


def _make_qa_tools(tmp_path: Path) -> QATools:
    workspace = tmp_path / "ws"
    workspace.mkdir()
    ctx = ToolContext(
        workspace_root=workspace,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )
    state = ExplorerState(
        task="t", budget_steps_left=10, budget_tokens_left=0
    )
    folder_tools = FolderTools(ctx=ctx, state=state)
    return QATools(folder_tools=folder_tools, ctx=ctx)


def test_seven_tools_with_audit_fields(tmp_path: Path) -> None:
    qa_tools = _make_qa_tools(tmp_path)
    expected = [
        "search",
        "list_dir",
        "read_file",
        "trace_import",
        "find_callers",
        "kb_search",
        "add_to_kb",
    ]
    for name in expected:
        method = getattr(qa_tools, name, None)
        assert method is not None, f"missing tool: {name}"
        assert callable(method), name
        # Check audit_fields on the bound method's underlying function.
        audit_fields = getattr(method, "audit_fields", None)
        if audit_fields is None and hasattr(method, "__func__"):
            audit_fields = getattr(method.__func__, "audit_fields", None)
        assert isinstance(audit_fields, list), (
            f"{name}.audit_fields must be a list[str]; got {audit_fields!r}"
        )


def test_register_with_tool_sandbox_does_not_raise(tmp_path: Path) -> None:
    """Each tool method exposes `audit_fields` so a `name`-bearing wrapper can register."""
    from codebus_agent.sandbox import ToolSandbox

    qa_tools = _make_qa_tools(tmp_path)
    sandbox = ToolSandbox(audit_log_path=tmp_path / "ws" / ".codebus" / "tool_audit.jsonl")

    # Each tool needs name + audit_fields + run shape per SandboxTool Protocol.
    @dataclass
    class _ToolWrapper:
        name: str
        audit_fields: list[str]
        impl: Any

        def run(self, args, ctx):
            return None

    expected = [
        "search",
        "list_dir",
        "read_file",
        "trace_import",
        "find_callers",
        "kb_search",
        "add_to_kb",
    ]
    for name in expected:
        method = getattr(qa_tools, name)
        audit_fields = getattr(method, "audit_fields", None) or getattr(
            method.__func__, "audit_fields", None
        )
        assert audit_fields is not None
        wrapper = _ToolWrapper(name=name, audit_fields=list(audit_fields), impl=method)
        sandbox.register(wrapper)  # MUST NOT raise


@pytest.mark.anyio("asyncio")
async def test_reused_read_tools_delegate_to_folder_tools_semantics(
    tmp_path: Path, monkeypatch
) -> None:
    qa_tools = _make_qa_tools(tmp_path)
    # Replace folder.search with a spy that returns a known sentinel.
    sentinel = ["sentinel-result"]
    spy = AsyncMock(return_value=sentinel)
    monkeypatch.setattr(qa_tools._folder, "search", spy)
    result = await qa_tools.search("anything")
    assert result == sentinel
    spy.assert_awaited_once_with("anything")
