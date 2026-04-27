"""TDD red test for D2.14 — cross-cutting `pass_num to source-type invariant`.

Backs Requirement `SanitizerAuditLogger appends each replacement to JSONL`
(sanitizer capability), new Scenario `pass_num to source-type invariant`:
  * pass=1 lines MUST carry a file-source shape
  * pass=2 lines MUST carry a message-source shape
  * pass=3 lines MAY carry either

This is the sanitizer-layer invariant. It catches drift the per-tool
tests miss — e.g., a future tool that adds Pass 1 audit lines with
the wrong source shape.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.types import ExplorerState
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine


def _build_state() -> ExplorerState:
    return ExplorerState(task="trace", budget_steps_left=5, budget_tokens_left=1000)


def _is_file_source_shape(source: Any) -> bool:
    if isinstance(source, dict):
        return "path" in source
    if isinstance(source, str):
        return source.startswith("file:")
    return False


def _is_message_source_shape(source: Any) -> bool:
    if isinstance(source, str):
        return source.startswith("message:")
    return False


@pytest.mark.asyncio
async def test_pass1_lines_carry_file_source_only(tmp_path: Path) -> None:
    """D2.14 cross-cutting invariant: every pass=1 audit line MUST carry
    a file-source shape; NO `message:` shape may appear on pass=1.

    Setup: workspace with a secret-containing file, exercise both
    `read_file` and `find_callers` (the two tools whose Pass 1 audit
    lines were drifting). Then walk every line in `sanitize_audit.jsonl`
    and assert the invariant holds.
    """
    secret_file = tmp_path / "src" / "config.py"
    secret_file.parent.mkdir(parents=True)
    secret_file.write_text(
        'AWS_KEY = "AKIAIOSFODNN7EXAMPLE"\n'
        'def authorize(token):\n'
        '    pass\n'
        '\n'
        'def main():\n'
        '    authorize("AKIAIOSFODNN7EXAMPLE")\n',
        encoding="utf-8",
    )

    ctx = ToolContext(
        workspace_root=tmp_path,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
        kb=None,
        session_id="sess-invariant",
    )
    state = _build_state()
    tools = FolderTools(ctx=ctx, state=state)

    # Trigger Pass 1 sanitize via two distinct tool paths.
    await tools.read_file("src/config.py")
    await tools.find_callers("authorize")

    audit_path = tmp_path / ".codebus" / "sanitize_audit.jsonl"
    assert audit_path.exists(), "sanitize_audit.jsonl MUST exist"

    lines = [
        json.loads(line)
        for line in audit_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    pass1_lines = [line for line in lines if line.get("pass") == 1]
    assert pass1_lines, f"expected pass=1 audit lines from both tools; got {lines!r}"

    for entry in pass1_lines:
        src = entry["source"]
        assert _is_file_source_shape(src), (
            f"pass=1 audit line MUST carry file-source shape; "
            f"got source={src!r} (full line={entry!r})"
        )
        assert not _is_message_source_shape(src), (
            f"pass=1 audit line MUST NOT carry message-source shape; "
            f"got source={src!r}"
        )
