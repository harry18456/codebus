"""TDD red tests for D2.14 — Pass 1 audit lines from `read_file` / `find_callers`
MUST carry `FileSource`, NOT `MessageSource`.

Backs Requirements
  * `read_file sanitizes output via Pass 1 before returning to Agent`
    (explorer-tools capability) — new Scenario `Pass 1 audit line carries
    FileSource`
  * `find_callers returns sanitized call-site FileMatches` — new
    Scenario `Pass 1 audit line carries FileSource`
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.types import ExplorerState
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine


def _build_ctx(workspace: Path) -> ToolContext:
    return ToolContext(
        workspace_root=workspace,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
        kb=None,
        session_id="sess-test-d214",
    )


def _build_tools(ctx: ToolContext) -> FolderTools:
    state = ExplorerState(task="trace", budget_steps_left=5, budget_tokens_left=1000)
    return FolderTools(ctx=ctx, state=state)


def _read_audit_lines(workspace: Path) -> list[dict]:
    path = workspace / ".codebus" / "sanitize_audit.jsonl"
    if not path.exists():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def _is_file_source_shape(source) -> bool:
    """Return True when `source` reflects a FileSource shape.

    Two valid forms (per `_format_source` in `sanitizer/engine.py`):
      * dict with `path` key (FileSource with non-empty `pass_`)
      * string starting with `"file:"` (FileSource with empty `pass_`)
    """
    if isinstance(source, dict):
        return "path" in source
    if isinstance(source, str):
        return source.startswith("file:")
    return False


def _is_message_source_shape(source) -> bool:
    if isinstance(source, str):
        return source.startswith("message:")
    return False


@pytest.mark.asyncio
async def test_read_file_pass1_uses_file_source(tmp_path: Path) -> None:
    """D2.14: `read_file` Pass 1 audit line MUST carry `FileSource(path=..., pass_="explorer_read_file")`.

    Pre-fix audit line carries `MessageSource(message_id="read_file:...")`,
    violating the cross-cutting `pass_num to source-type invariant`
    (Pass 1 → file-source).
    """
    secret_file = tmp_path / "src" / "app.py"
    secret_file.parent.mkdir(parents=True)
    secret_file.write_text(
        'AWS_KEY = "AKIAIOSFODNN7EXAMPLE"\n', encoding="utf-8"
    )

    ctx = _build_ctx(tmp_path)
    tools = _build_tools(ctx)

    out = await tools.read_file("src/app.py")
    assert "<REDACTED:" in out, f"sanitize did not redact secret; got {out!r}"
    assert "AKIAIOSFODNN7EXAMPLE" not in out

    lines = _read_audit_lines(tmp_path)
    pass1_lines = [line for line in lines if line.get("pass") == 1]
    assert pass1_lines, f"expected at least one pass=1 line; got {lines!r}"

    first = pass1_lines[0]
    src = first["source"]
    assert _is_file_source_shape(src), (
        f"pass=1 source MUST reflect FileSource shape; got {src!r}"
    )
    # Verify it specifically reflects pass_="explorer_read_file" (dict form).
    assert isinstance(src, dict), (
        f"pass_='explorer_read_file' MUST yield dict source; got {src!r}"
    )
    assert src["pass"] == "explorer_read_file", (
        f"pass_ field MUST equal 'explorer_read_file'; got {src!r}"
    )
    # Path normalized to forward slashes (Pass 1 source uses workspace-relative path).
    assert src["path"].replace("\\", "/") == "src/app.py", (
        f"path MUST be workspace-relative 'src/app.py'; got {src['path']!r}"
    )
    assert not _is_message_source_shape(src), (
        f"MUST NOT be MessageSource shape; got {src!r}"
    )


@pytest.mark.asyncio
async def test_find_callers_pass1_uses_file_source(tmp_path: Path) -> None:
    """D2.14: `find_callers` Pass 1 audit line MUST carry
    `FileSource(path=<call_site>, pass_="find_callers")`.

    Pre-fix audit line carries `MessageSource(message_id="find_callers:...")`.
    """
    call_site = tmp_path / "src" / "auth" / "login.py"
    call_site.parent.mkdir(parents=True)
    call_site.write_text(
        'def main():\n'
        '    authorize("AKIAIOSFODNN7EXAMPLE")\n',
        encoding="utf-8",
    )

    ctx = _build_ctx(tmp_path)
    tools = _build_tools(ctx)

    matches = await tools.find_callers("authorize")
    assert matches, f"find_callers should return at least one match; got {matches!r}"
    assert all("<REDACTED:" in m.snippet for m in matches), (
        f"snippets MUST be redacted; got {[m.snippet for m in matches]!r}"
    )

    lines = _read_audit_lines(tmp_path)
    pass1_lines = [line for line in lines if line.get("pass") == 1]
    assert pass1_lines, f"expected at least one pass=1 line; got {lines!r}"

    first = pass1_lines[0]
    src = first["source"]
    assert _is_file_source_shape(src), (
        f"pass=1 source MUST reflect FileSource shape; got {src!r}"
    )
    assert isinstance(src, dict), (
        f"pass_='find_callers' MUST yield dict source; got {src!r}"
    )
    assert src["pass"] == "find_callers", (
        f"pass_ field MUST equal 'find_callers'; got {src!r}"
    )
    assert src["path"].replace("\\", "/") == "src/auth/login.py", (
        f"path MUST be workspace-relative call-site path; got {src['path']!r}"
    )
    assert not _is_message_source_shape(src)
