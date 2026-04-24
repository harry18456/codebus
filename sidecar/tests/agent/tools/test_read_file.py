"""RED tests for FolderTools.read_file.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: read_file sanitizes output via Pass 1 before returning to Agent
  Requirement: list_dir and read_file enforce ensure_in_workspace
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.sandbox import PathEscapeError, ToolContext


async def test_pass1_runs_on_every_read_file_call(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
    aws_key_literal: str,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    text = await tools.read_file("secret.py")

    assert aws_key_literal not in text, (
        f"raw AWS key MUST NOT leak into read_file output; got: {text[:200]!r}"
    )
    assert "<REDACTED:" in text, "placeholder markers MUST appear in the output"

    audit_path = temp_workspace / ".codebus" / "sanitize_audit.jsonl"
    assert audit_path.exists(), "sanitize_audit.jsonl MUST be created by read_file"
    lines = audit_path.read_text(encoding="utf-8").splitlines()
    pass1_entries = [json.loads(line) for line in lines if line]
    # Audit schema writes `"pass"` (not `"pass_num"`) per SanitizerAuditLogger
    assert any(e.get("pass") == 1 for e in pass1_entries), (
        f"at least one Pass 1 entry MUST be logged; saw passes: "
        f"{[e.get('pass') for e in pass1_entries]}"
    )


async def test_missing_sanitizer_fails_loud(
    temp_workspace: Path, aws_key_literal: str, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Build a ToolContext explicitly WITHOUT sanitizer
    ctx_no_sanitizer = ToolContext(
        workspace_root=temp_workspace,
        workspace_type="folder",
    )
    tools = FolderTools(ctx=ctx_no_sanitizer, state=explorer_state)

    with pytest.raises(ValueError) as exc_info:
        await tools.read_file("secret.py")

    msg = str(exc_info.value)
    assert "sanitizer" in msg.lower()
    # The raw key MUST NOT leak through the exception message
    assert aws_key_literal not in msg


async def test_line_range_slices_before_sanitize(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Seed a 10-line file where lines 5-7 contain an email
    path = temp_workspace / "ranged.py"
    lines = [f"# line {i}\n" for i in range(1, 11)]
    lines[4] = "# bob@example.com line 5\n"  # line 5 (1-indexed)
    lines[5] = "# carol@example.com line 6\n"
    lines[6] = "# dave@example.com line 7\n"
    path.write_text("".join(lines), encoding="utf-8")

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    out = await tools.read_file("ranged.py", line_range=(5, 7))

    # Output is exactly the 3 sliced lines (sanitize redacts emails)
    out_lines = out.splitlines()
    assert len(out_lines) == 3, f"expected 3 lines, got {len(out_lines)}: {out!r}"
    assert "bob@example.com" not in out and "carol@example.com" not in out
    assert "<REDACTED:" in out
    # Line 1-4 / 8-10 markers do NOT appear
    assert "line 1" not in out and "line 10" not in out


async def test_large_file_truncation(
    tool_context: ToolContext, explorer_state, temp_workspace: Path
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # > 12000 char file
    big = "# repeated-safe-text line\n" * 1000  # ≈ 27000 chars, no sanitize triggers
    (temp_workspace / "big.py").write_text(big, encoding="utf-8")

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    out = await tools.read_file("big.py")

    assert len(out) <= 12500, f"truncated output MUST be ≤ ~12000 chars, got {len(out)}"
    assert "[... truncated ...]" in out or "truncated" in out.lower(), (
        "truncation marker MUST be present"
    )


async def test_read_file_parent_escape_rejected(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    with pytest.raises(PathEscapeError):
        await tools.read_file("../../etc/passwd")
