"""RED tests for FolderTools.trace_import.

Backs SHALL clauses in
openspec/changes/explorer-tools-p1/specs/explorer-tools/spec.md
  Requirement: trace_import resolves symbols to definition paths via regex

trace_import 的任務：給 symbol 名 → 回定義站（definition site）所在的
workspace-relative path，或找不到時回 None。策略走 regex，不引 AST。
"""
from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from codebus_agent.sandbox import ToolContext


def _read_audit_lines(ws: Path) -> list[dict]:
    audit = ws / ".codebus" / "tool_audit.jsonl"
    if not audit.exists():
        return []
    return [json.loads(line) for line in audit.read_text("utf-8").splitlines() if line]


# ---------------------------------------------------------------------------
# 2.1 Python def / class resolve
# ---------------------------------------------------------------------------


async def test_python_def_resolves_to_source_path(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    kb_dir = temp_workspace / "src" / "kb"
    kb_dir.mkdir(parents=True)
    (kb_dir / "base.py").write_text(
        "# header line 1\n"
        "# header line 2\n"
        "\n"
        "\n"
        "\n"
        "\n"
        "\n"
        "\n"
        "\n"
        "\n"
        "\n"
        "class KnowledgeBase:\n"
        "    pass\n",
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("KnowledgeBase")

    assert result == "src/kb/base.py", (
        f"trace_import MUST resolve to workspace-relative POSIX path; got {result!r}"
    )


# ---------------------------------------------------------------------------
# 2.2 TypeScript export function resolve
# ---------------------------------------------------------------------------


async def test_typescript_export_function_resolves(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    ts_dir = temp_workspace / "web" / "src"
    ts_dir.mkdir(parents=True)
    (ts_dir / "providers.ts").write_text(
        "// banner\n"
        "import type { Foo } from './foo';\n"
        "\n"
        "\n"
        "export function makeProvider(config: Foo): void {\n"
        "  return;\n"
        "}\n",
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("makeProvider")

    assert result == "web/src/providers.ts", (
        f"trace_import MUST resolve TS export function to its file; got {result!r}"
    )


# ---------------------------------------------------------------------------
# 2.3 Rust pub async fn resolve
# ---------------------------------------------------------------------------


async def test_rust_pub_async_fn_resolves(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    rs_dir = temp_workspace / "crates" / "server" / "src"
    rs_dir.mkdir(parents=True)
    (rs_dir / "lib.rs").write_text(
        "use std::net::TcpListener;\n"
        "\n"
        "pub async fn handle_request(req: Request) -> Response {\n"
        "    Response::default()\n"
        "}\n",
        encoding="utf-8",
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("handle_request")

    assert result == "crates/server/src/lib.rs", (
        f"trace_import MUST resolve Rust pub async fn; got {result!r}"
    )


# ---------------------------------------------------------------------------
# 2.4 Symbol not defined → None
# ---------------------------------------------------------------------------


async def test_symbol_not_defined_returns_none(
    tool_context: ToolContext,
    explorer_state,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("Zzz_NotDefined")

    assert result is None, (
        f"trace_import MUST return None for unknown symbol; got {result!r}"
    )


# ---------------------------------------------------------------------------
# 2.5 Multiple definitions → shortest path_depth wins
# ---------------------------------------------------------------------------


async def test_multiple_definitions_pick_shortest_path_depth(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    (temp_workspace / "src").mkdir()
    (temp_workspace / "src" / "util.py").write_text(
        "class Util:\n    pass\n", encoding="utf-8"
    )
    helpers = temp_workspace / "tests" / "helpers"
    helpers.mkdir(parents=True)
    (helpers / "util.py").write_text(
        "class Util:\n    pass\n", encoding="utf-8"
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("Util")

    assert result == "src/util.py", (
        f"Shallower path_depth MUST win over deeper; got {result!r}"
    )


# ---------------------------------------------------------------------------
# 2.6 Regex metacharacters are escaped
# ---------------------------------------------------------------------------


async def test_symbol_with_regex_metacharacters_safe(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Seed a file that would match `foo.bar` when the dot is interpreted as
    # regex wildcard (foo_bar has `_` in the dot's position). With
    # re.escape(symbol), the dot is literal and MUST NOT match `foo_bar`.
    (temp_workspace / "sneaky.py").write_text(
        "def foo_bar():\n    pass\n", encoding="utf-8"
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    # MUST NOT raise a regex error, MUST NOT match foo_bar.
    result = await tools.trace_import("foo.bar")
    assert result is None, (
        f"symbol 'foo.bar' MUST NOT match 'foo_bar' as wildcard; got {result!r}"
    )


# ---------------------------------------------------------------------------
# 2.7 Tool audit written on allowed path
# ---------------------------------------------------------------------------


async def test_tool_audit_line_written_on_allowed_path(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    (temp_workspace / "src").mkdir()
    (temp_workspace / "src" / "mod.py").write_text(
        "class Tracy:\n    pass\n", encoding="utf-8"
    )

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("Tracy")
    assert result == "src/mod.py"

    lines = _read_audit_lines(temp_workspace)
    trace_lines = [l for l in lines if l["tool_name"] == "trace_import"]
    assert trace_lines, "trace_import MUST write at least one tool_audit line"
    assert any(l["allowed"] is True for l in trace_lines), (
        f"allowed=true line MUST appear for successful trace_import; saw {trace_lines}"
    )


# ---------------------------------------------------------------------------
# 6.1 Symlink escape → discarded + deny audit
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    os.name == "nt",
    reason="symlink creation on Windows requires elevated privileges",
)
async def test_symlink_escape_discarded(
    tool_context: ToolContext,
    explorer_state,
    temp_workspace: Path,
    tmp_path: Path,
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Target file OUTSIDE the workspace that contains a definition.
    outside = tmp_path / "outside"
    outside.mkdir()
    target = outside / "evil.py"
    target.write_text("class ExternalSymbol:\n    pass\n", encoding="utf-8")

    # Symlink inside workspace pointing at the outside target.
    link = temp_workspace / "link_to_outside.py"
    os.symlink(target, link)

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.trace_import("ExternalSymbol")

    assert result is None, (
        "symlink target outside workspace MUST be treated as absent"
    )

    lines = _read_audit_lines(temp_workspace)
    trace_lines = [l for l in lines if l["tool_name"] == "trace_import"]
    deny_lines = [l for l in trace_lines if l["allowed"] is False]
    assert deny_lines, (
        f"symlink escape MUST leave at least one allowed=false audit line; "
        f"saw trace_lines={trace_lines}"
    )
    assert deny_lines[0]["denial_reason"] in {
        "path_escape",
        "symlink_outside",
        "unc_path",
        "long_path_prefix_invalid",
        "case_variant",
        "trailing_whitespace",
    }
