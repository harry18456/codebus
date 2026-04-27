"""TDD red tests for D2.15 — `_search_via_grep` snippet MUST go through Pass 1
sanitize before being returned to the Agent.

Backs Requirement `search consults KB first then falls back to grep`
(explorer-tools capability), new Scenarios:
  * `Grep fallback hit snippet sanitized through Pass 1`
  * `Grep fallback fails loud when sanitizer missing`

Pre-fix the grep fallback path returns raw snippets — KB path is
already sanitized at build time but workspaces without a populated
KB expose secrets via `SearchHit.snippet`.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.types import ExplorerState
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine


def _build_state() -> ExplorerState:
    return ExplorerState(task="trace", budget_steps_left=5, budget_tokens_left=1000)


def _read_audit_lines(workspace: Path) -> list[dict]:
    path = workspace / ".codebus" / "sanitize_audit.jsonl"
    if not path.exists():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


@pytest.mark.asyncio
async def test_search_via_grep_sanitizes_hit_snippets(tmp_path: Path) -> None:
    """D2.15 Scenario `Grep fallback hit snippet sanitized through Pass 1`.

    Setup: fixture workspace with a real secret in `src/secrets.py`,
    `ctx.kb=None` forces grep fallback. Expected behavior after fix:
      * SearchHit.snippet contains `<REDACTED:` placeholder
      * SearchHit.snippet does NOT contain raw secret literal
      * sanitize_audit.jsonl gains a `pass=1` line whose source
        reflects `FileSource(path="src/secrets.py", pass_="grep_search")`
    """
    secret_file = tmp_path / "src" / "secrets.py"
    secret_file.parent.mkdir(parents=True)
    secret_file.write_text(
        "def main():\n"
        "    pass\n"
        "\n"
        '    authorize("AKIAIOSFODNN7EXAMPLE")\n',
        encoding="utf-8",
    )

    ctx = ToolContext(
        workspace_root=tmp_path,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
        kb=None,
        session_id="sess-test-d215",
    )
    tools = FolderTools(ctx=ctx, state=_build_state())

    hits = await tools.search("authorize")
    assert hits, f"grep should find at least one hit; got {hits!r}"
    hit = next((h for h in hits if "secrets.py" in h.path), None)
    assert hit is not None, f"expected hit on src/secrets.py; got {hits!r}"
    assert "<REDACTED:" in hit.snippet, (
        f"snippet MUST be sanitized; got {hit.snippet!r}"
    )
    assert "AKIAIOSFODNN7EXAMPLE" not in hit.snippet, (
        f"raw secret MUST NOT leak; got {hit.snippet!r}"
    )

    lines = _read_audit_lines(tmp_path)
    pass1_lines = [line for line in lines if line.get("pass") == 1]
    assert pass1_lines, f"expected pass=1 audit line; got {lines!r}"

    grep_lines = [
        line
        for line in pass1_lines
        if isinstance(line["source"], dict)
        and line["source"].get("pass") == "grep_search"
    ]
    assert grep_lines, (
        f"expected at least one pass_='grep_search' audit line; "
        f"got pass1={pass1_lines!r}"
    )
    src = grep_lines[0]["source"]
    assert src["path"].replace("\\", "/") == "src/secrets.py", (
        f"source.path MUST be workspace-relative; got {src!r}"
    )


@pytest.mark.asyncio
async def test_search_via_grep_fails_loud_when_sanitizer_missing(
    tmp_path: Path,
) -> None:
    """D2.15 Scenario `Grep fallback fails loud when sanitizer missing`.

    `ctx.kb=None` AND `ctx.sanitizer=None` → grep fallback MUST raise
    `ValueError` naming the missing sanitizer rather than silently
    returning raw snippets.
    """
    (tmp_path / "src.py").write_text("def x(): pass\n", encoding="utf-8")

    ctx = ToolContext(
        workspace_root=tmp_path,
        workspace_type="folder",
        sanitizer=None,
        kb=None,
        session_id="sess-test-d215",
    )
    tools = FolderTools(ctx=ctx, state=_build_state())

    with pytest.raises(ValueError) as excinfo:
        await tools.search("anything")
    assert "sanitizer" in str(excinfo.value).lower(), (
        f"error message MUST name 'sanitizer'; got {excinfo.value!s}"
    )
