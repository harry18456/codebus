"""RED tests for FolderTools.search.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: search consults KB first then falls back to grep
"""
from __future__ import annotations

from pathlib import Path

from codebus_agent.sandbox import ToolContext


async def test_kb_path_used_when_kb_is_configured(
    temp_workspace: Path,
    explorer_state,
    sanitizer_for_tools,
    mock_kb,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    engine, _ = sanitizer_for_tools
    ctx_with_kb = ToolContext(
        workspace_root=temp_workspace,
        workspace_type="folder",
        sanitizer=engine,
        kb=mock_kb,
    )
    tools = FolderTools(ctx=ctx_with_kb, state=explorer_state)

    hits = await tools.search("entry")

    assert len(mock_kb.query_calls) == 1, (
        f"KB must be called exactly once; got {mock_kb.query_calls}"
    )
    assert mock_kb.query_calls[0][0] == "entry"
    assert len(hits) >= 1
    # Paths MUST be relative to workspace_root
    for h in hits:
        assert not Path(h.path).is_absolute(), f"path should be relative; got {h.path!r}"


async def test_grep_fallback_when_kb_absent(
    tool_context: ToolContext, explorer_state
) -> None:
    """With ctx.kb=None, search falls back to grep across allowed extensions."""
    from codebus_agent.agent.tools.folder_tools import FolderTools

    assert tool_context.kb is None  # the fixture deliberately omits kb
    tools = FolderTools(ctx=tool_context, state=explorer_state)

    hits = await tools.search("entry")

    assert len(hits) <= 100
    assert len(hits) >= 1
    allowed_exts = {".py", ".md", ".ts", ".tsx", ".rs", ".go", ".js", ".jsx"}
    for h in hits:
        suffix = Path(h.path).suffix
        assert suffix in allowed_exts, f"unexpected extension in hit: {h.path!r}"


async def test_empty_result_when_no_match_found(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    hits = await tools.search("zzzzz_nonexistent_token_xyz")
    assert hits == []


async def test_grep_fallback_skips_binary_and_oversize(
    tool_context: ToolContext, explorer_state, temp_workspace: Path
) -> None:
    """PNG + excessively large files MUST NOT appear in grep results."""
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Craft a PNG containing the keyword as raw bytes (still binary; MUST be skipped).
    (temp_workspace / "evil.png").write_bytes(b"\x89PNG\r\n\x1a\nkeyword-should-not-match")
    # ~1 MB text file holding the keyword — oversized per Scanner default (512 KB threshold).
    (temp_workspace / "big.py").write_text(
        "# filler line\n" * 70000 + "keyword-in-oversize-file\n",
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    hits = await tools.search("keyword-should-not-match")

    paths = {h.path for h in hits}
    assert not any(p.endswith(".png") for p in paths), (
        f"binary hits leaked into grep results: {paths}"
    )

    hits2 = await tools.search("keyword-in-oversize-file")
    paths2 = {h.path for h in hits2}
    assert not any("big.py" in p for p in paths2), (
        f"oversized file hits leaked: {paths2}"
    )
