"""RED tests for FolderTools tool_audit.jsonl integration.

Backs SHALL clauses in
openspec/specs/tool-sandbox/spec.md
  Requirement: ToolSandbox appends every invocation to tool_audit.jsonl

(FolderTools shares the same JSONL format + writer — see
`codebus_agent.sandbox.append_tool_audit_line`.)
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.sandbox import PathEscapeError, ToolContext


def _read_audit_lines(ws: Path) -> list[dict]:
    audit = ws / ".codebus" / "tool_audit.jsonl"
    if not audit.exists():
        return []
    return [json.loads(line) for line in audit.read_text("utf-8").splitlines() if line]


async def test_every_tool_invocation_writes_one_line(
    tool_context: ToolContext, explorer_state, temp_workspace: Path
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    await tools.list_dir(".")
    await tools.read_file("app.py")
    await tools.mark_station("app.py", "entry", "seed")

    lines = _read_audit_lines(temp_workspace)
    assert len(lines) == 3, f"expected 3 audit lines, got {len(lines)}: {lines}"
    for line in lines:
        assert line["schema_version"] == 1
        assert line["allowed"] is True
        assert line["denial_reason"] is None
        assert line["tool_name"] in {"list_dir", "read_file", "mark_station"}
        # Every allowed line MUST carry a resolved_path (mark_station /
        # read_file / list_dir all pass their path through ensure_in_workspace).
        assert line["resolved_path"] is not None


async def test_denied_invocation_writes_denial_line(
    tool_context: ToolContext, explorer_state, temp_workspace: Path
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    with pytest.raises(PathEscapeError):
        await tools.list_dir("../..")

    lines = _read_audit_lines(temp_workspace)
    deny = [l for l in lines if l["allowed"] is False]
    assert len(deny) == 1, f"expected 1 deny line, got {len(deny)}: {lines}"
    assert deny[0]["tool_name"] == "list_dir"
    # The closed set per tool-sandbox spec; classifier biases toward the
    # most specific signal so `../..` may land as `trailing_whitespace`
    # (trailing `.`) rather than `path_escape`. Either is a valid deny.
    assert deny[0]["denial_reason"] in {
        "path_escape",
        "symlink_outside",
        "unc_path",
        "long_path_prefix_invalid",
        "case_variant",
        "trailing_whitespace",
    }


async def test_args_summary_uses_whitelist(
    tool_context: ToolContext, explorer_state, temp_workspace: Path
) -> None:
    """Secret-bearing args MUST NOT reach args_summary; path-bearing args do."""
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    await tools.search("secret_KEYWORD_7777")
    await tools.read_file("app.py")

    lines = _read_audit_lines(temp_workspace)
    search_lines = [l for l in lines if l["tool_name"] == "search"]
    read_lines = [l for l in lines if l["tool_name"] == "read_file"]

    assert search_lines, "search invocation MUST produce one audit line"
    for l in search_lines:
        assert "secret_KEYWORD_7777" not in json.dumps(l["args_summary"]), (
            f"raw keyword leaked into args_summary: {l['args_summary']}"
        )

    assert read_lines, "read_file invocation MUST produce one audit line"
    for l in read_lines:
        # read_file whitelists path + line_range; path must appear.
        assert l["args_summary"].get("path") == "app.py"
