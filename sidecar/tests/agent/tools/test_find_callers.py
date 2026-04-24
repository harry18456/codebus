"""RED tests for FolderTools.find_callers.

Backs SHALL clauses in
openspec/changes/explorer-tools-p1/specs/explorer-tools/spec.md
  Requirement: find_callers returns sanitized call-site FileMatches

find_callers 掃 workspace 找 `\\b<symbol>\\b` 的所有命中，回
FileMatch(path, line, snippet) 列表。Snippet 必過 Pass 1 sanitize。
單檔 ≤ 5 hits；全域 ≤ 100 hits；排除 trace_import 的 definition site。
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.sandbox import ToolContext


def _read_sanitize_audit_lines(ws: Path) -> list[dict]:
    audit = ws / ".codebus" / "sanitize_audit.jsonl"
    if not audit.exists():
        return []
    return [json.loads(line) for line in audit.read_text("utf-8").splitlines() if line]


# ---------------------------------------------------------------------------
# 4.1 Multiple call-sites return sanitized FileMatches
# ---------------------------------------------------------------------------


async def test_multiple_callsites_return_sanitized_file_matches(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    src = temp_workspace / "src"
    src.mkdir()
    # Build app.py so the KnowledgeBase reference sits at exactly line 14.
    lines_app = ["# pad line\n"] * 13 + ["kb = KnowledgeBase(path)\n"]
    (src / "app.py").write_text("".join(lines_app), encoding="utf-8")

    api = src / "api"
    api.mkdir()
    # routes.py — reference at line 30 exactly.
    lines_routes = ["# pad line\n"] * 29 + ["return KnowledgeBase.query(...)\n"]
    (api / "routes.py").write_text("".join(lines_routes), encoding="utf-8")

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("KnowledgeBase")

    pairs = {(m.path, m.line) for m in matches}
    assert ("src/app.py", 14) in pairs, (
        f"expected src/app.py:14 in matches; got {pairs}"
    )
    assert ("src/api/routes.py", 30) in pairs, (
        f"expected src/api/routes.py:30 in matches; got {pairs}"
    )
    for m in matches:
        assert len(m.snippet) <= 200, (
            f"snippet MUST be truncated at 200 chars; got {len(m.snippet)}"
        )


# ---------------------------------------------------------------------------
# 4.2 Whole-word boundary rejects substring
# ---------------------------------------------------------------------------


async def test_whole_word_boundary_rejects_substring(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    (temp_workspace / "sub.py").write_text(
        "def runner():\n    foobar(x)\n",
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("foo")

    assert matches == [], (
        f"find_callers MUST NOT match substrings; got {matches}"
    )


# ---------------------------------------------------------------------------
# 4.3 Definition site excluded from results
# ---------------------------------------------------------------------------


async def test_definition_site_excluded_from_results(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    src = temp_workspace / "src"
    src.mkdir()
    # line 5 = class Bar: (definition)
    # line 20 = Bar()        (call site)
    lines: list[str] = []
    for i in range(1, 21):
        if i == 5:
            lines.append("class Bar:\n")
        elif i == 20:
            lines.append("Bar()\n")
        else:
            lines.append("# pad\n")
    (src / "bar.py").write_text("".join(lines), encoding="utf-8")

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("Bar")

    lines_in_bar = [m.line for m in matches if m.path == "src/bar.py"]
    assert 5 not in lines_in_bar, (
        f"definition-site line 5 MUST be excluded; got {lines_in_bar}"
    )
    assert 20 in lines_in_bar, (
        f"call-site line 20 MUST appear; got {lines_in_bar}"
    )


# ---------------------------------------------------------------------------
# 4.4 Per-file cap limits snippet storm
# ---------------------------------------------------------------------------


async def test_per_file_cap_limits_snippet_storm(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    src = temp_workspace / "src"
    src.mkdir()
    # 50 separate lines each referencing MAX.
    (src / "constants.py").write_text(
        "\n".join(f"value_{i} = MAX - {i}" for i in range(50)) + "\n",
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("MAX")

    in_constants = [m for m in matches if m.path == "src/constants.py"]
    assert len(in_constants) <= 5, (
        f"per-file cap MUST bound same-file hits at 5; got {len(in_constants)}"
    )


# ---------------------------------------------------------------------------
# 4.5 Global cap enforces 100-entry ceiling
# ---------------------------------------------------------------------------


async def test_global_cap_enforces_100_ceiling(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # 21 files × 5 hits each = 105 raw hits, after per-file cap = 105.
    # Global cap MUST trim to ≤ 100.
    src = temp_workspace / "src"
    src.mkdir()
    for i in range(21):
        (src / f"mod_{i:02d}.py").write_text(
            "pass\n" * 5, encoding="utf-8"
        )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("pass")

    assert len(matches) <= 100, (
        f"global cap MUST bound total hits at 100; got {len(matches)}"
    )


# ---------------------------------------------------------------------------
# 4.6 Snippet sanitize redacts secrets before return
# ---------------------------------------------------------------------------


async def test_snippet_sanitize_redacts_secrets(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
    aws_key_literal: str,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    src = temp_workspace / "src"
    src.mkdir()
    (src / "caller.py").write_text(
        f'def go():\n    authorize("{aws_key_literal}")\n',
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("authorize")

    assert matches, "MUST find at least one call-site"
    for m in matches:
        assert aws_key_literal not in m.snippet, (
            f"raw AWS key MUST NOT appear in snippet; got {m.snippet!r}"
        )
        assert "<REDACTED:" in m.snippet, (
            f"placeholder marker MUST appear in sanitized snippet; got {m.snippet!r}"
        )

    audit_lines = _read_sanitize_audit_lines(temp_workspace)
    pass1 = [l for l in audit_lines if l.get("pass") == 1]
    assert pass1, (
        f"at least one pass_num=1 line MUST be written; got {audit_lines}"
    )


# ---------------------------------------------------------------------------
# 4.7 Missing sanitizer fails loud
# ---------------------------------------------------------------------------


async def test_missing_sanitizer_fails_loud(
    temp_workspace: Path,
    explorer_state,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    ctx_no_sanitizer = ToolContext(
        workspace_root=temp_workspace,
        workspace_type="folder",
    )
    tools = FolderTools(ctx=ctx_no_sanitizer, state=explorer_state)

    with pytest.raises(ValueError) as exc_info:
        await tools.find_callers("anything")
    assert "sanitizer" in str(exc_info.value).lower(), (
        f"error MUST name missing sanitizer; got {exc_info.value!r}"
    )


# ---------------------------------------------------------------------------
# 4.8 Symbol with zero matches → empty list
# ---------------------------------------------------------------------------


async def test_symbol_with_zero_matches_returns_empty_list(
    tool_context: ToolContext,
    explorer_state,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    matches = await tools.find_callers("ZzzNoSuchName")

    assert matches == [], (
        f"zero matches MUST return [] without raising; got {matches}"
    )
