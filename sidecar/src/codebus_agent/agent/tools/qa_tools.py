"""QATools — Q&A Agent's seven-tool surface.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: QATools exposes seven tools with audit_fields declared

Per Decision 1 (`module-8-qa-p0` design): Q&A is a sibling of Folder-mode
Explorer rather than a subclass — five read tools delegate to FolderTools
semantics by composition (not inheritance), and two new Q&A-specific
tools (`kb_search` / `add_to_kb`) compose with the same `ToolContext`
surface so a single ReAct loop can dispatch to all seven via
`getattr(tools, call.name)`.
"""
from __future__ import annotations

from typing import Any

from codebus_agent.agent.tools.add_to_kb import AddToKBArgs, add_to_kb as _add_to_kb
from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.tools.kb_search import KBSearchArgs, kb_search as _kb_search


__all__ = ["QATools"]


class QATools:
    """Concrete Q&A-mode tools — five read + two write, per session.

    The five read tools (`search` / `list_dir` / `read_file` /
    `trace_import` / `find_callers`) delegate to a private
    `FolderTools` instance so semantics + audit lines stay identical
    to Folder-mode Explorer. The two new tools wrap the module-level
    `kb_search` / `add_to_kb` async functions and re-expose their
    `audit_fields` lists so `ToolSandbox` registration succeeds.
    """

    # `audit_fields` for the seven tools — declared at class scope so
    # the sandbox can introspect without instantiating QATools.
    search_audit_fields: list[str] = []
    list_dir_audit_fields: list[str] = ["path"]
    read_file_audit_fields: list[str] = ["path", "line_range"]
    trace_import_audit_fields: list[str] = []
    find_callers_audit_fields: list[str] = []
    kb_search_audit_fields: list[str] = ["query", "top_k", "station_filter"]
    add_to_kb_audit_fields: list[str] = ["source", "reason", "related_stations"]

    def __init__(self, *, folder_tools: FolderTools, ctx: Any) -> None:
        # `folder_tools` carries the FolderTools instance so the five
        # read tools delegate to a single instance whose audit-line
        # bookkeeping (sanitize_audit / tool_audit) stays consistent.
        self._folder = folder_tools
        self._ctx = ctx

    # -- five reused read tools (delegate semantics) ------------------------

    async def search(self, keyword: str):
        return await self._folder.search(keyword)

    search.audit_fields = []  # type: ignore[attr-defined]

    async def list_dir(self, path: str):
        return await self._folder.list_dir(path)

    list_dir.audit_fields = ["path"]  # type: ignore[attr-defined]

    async def read_file(self, path: str, line_range=None):
        return await self._folder.read_file(path, line_range)

    read_file.audit_fields = ["path", "line_range"]  # type: ignore[attr-defined]

    async def trace_import(self, symbol: str):
        return await self._folder.trace_import(symbol)

    trace_import.audit_fields = []  # type: ignore[attr-defined]

    async def find_callers(self, symbol: str):
        return await self._folder.find_callers(symbol)

    find_callers.audit_fields = []  # type: ignore[attr-defined]

    # -- two new Q&A-specific tools -----------------------------------------

    async def kb_search(self, **kwargs) -> str:
        args = KBSearchArgs(**kwargs)
        return await _kb_search(args, self._ctx)

    kb_search.audit_fields = ["query", "top_k", "station_filter"]  # type: ignore[attr-defined]

    async def add_to_kb(self, **kwargs) -> str:
        args = AddToKBArgs(**kwargs)
        return await _add_to_kb(args, self._ctx)

    add_to_kb.audit_fields = ["source", "reason", "related_stations"]  # type: ignore[attr-defined]

    def tool_specs(self) -> list[dict]:
        """Tool specs for the Q&A render prompt.

        Reuses FolderTools' five read tool specs and adds the two
        Q&A-specific tools. Caller (Q&A render_qa_prompt) consumes the
        flat list of {name, description, parameters} dicts.
        """
        folder_specs = self._folder.tool_specs()
        # Drop FolderTools' `mark_station` (Q&A doesn't have stations).
        kept = [s for s in folder_specs if s.get("name") != "mark_station"]
        kept.append(
            {
                "name": "kb_search",
                "description": "Search the workspace KB by query. Optional station_filter restricts hits to chunks tagged with any of the supplied stable station ids.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "top_k": {"type": "integer", "default": 5},
                        "station_filter": {
                            "type": "array",
                            "items": {"type": "string"},
                        },
                    },
                    "required": ["query"],
                },
            }
        )
        kept.append(
            {
                "name": "add_to_kb",
                "description": "Persist worth-to-remember chunks into the KB. Each chunk passes Pass 3 sanitize + dedup + growth-log audit. Use sparingly — only for reusable / stable / non-duplicative facts.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "chunks": {
                            "type": "array",
                            "items": {"type": "object"},
                        },
                        "source": {"type": "string"},
                        "reason": {"type": "string"},
                    },
                    "required": ["chunks"],
                },
            }
        )
        return kept
