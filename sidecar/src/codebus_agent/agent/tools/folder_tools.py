"""FolderTools — concrete Folder-mode Explorer tool surface.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools
  Requirement: search consults KB first then falls back to grep
  Requirement: read_file sanitizes output via Pass 1 before returning to Agent
  Requirement: list_dir and read_file enforce ensure_in_workspace
  Requirement: mark_station mutates state without calling LLM

This class implements four concrete P0 tools (`search`, `list_dir`,
`read_file`, `mark_station`) AND also satisfies the abstract
``ExplorerTools`` Protocol seams (`primary_search` / `fetch` /
`follow_reference`) so Q&A Agent / Topic-mode impls can plug into the
same loop without touching this file.

The Explorer loop dispatches by concrete method name — ``getattr(tools,
call.name)`` lands directly on one of the four P0 methods.
"""
from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING, Any

from codebus_agent.agent.protocols import Content, Target
from codebus_agent.agent.tools.schemas import DirEntry, SearchHit
from codebus_agent.agent.types import ExplorerState, Station
from codebus_agent.sandbox import (
    PathEscapeError,
    ToolContext,
    _classify_denial,
    append_tool_audit_line,
    ensure_in_workspace,
)
from codebus_agent.sanitizer import (
    MessageSource,
    SanitizerAuditLogger,
    SanitizerEngine,
)

if TYPE_CHECKING:
    pass


__all__ = ["FolderTools"]


_STATION_RELEVANCE_P0: float = 0.8  # hardcoded per spec Non-Goals; tuned by explorer-golden-sample-p0
_READ_FILE_TRUNCATE_LIMIT: int = 12000  # chars; heuristic proxy for ≈ 3000 tokens
_TRUNCATE_MARKER: str = "\n[... truncated ...]\n"
_SANITIZE_RULES_VERSION: str = "2026-04-20-1"  # kept in sync with sanitizer/config.py _BUILTIN_RULES_VERSION

# Audit field whitelist per tool — keep keyword/why out of args_summary
# since they carry Agent free-form text that can accidentally echo
# sensitive snippets. Path-like args stay whitelisted so auditors can
# reconstruct tool dispatch history without reading the raw log. Aligns
# with openspec/specs/tool-sandbox/spec.md `Tools declare their auditable
# field whitelist`.
_AUDIT_FIELDS: dict[str, list[str]] = {
    "search": [],
    "list_dir": ["path"],
    "read_file": ["path", "line_range"],
    "mark_station": ["path", "role"],
}


class FolderTools:
    """Concrete Folder-mode tools. Constructed per Explorer session."""

    def __init__(self, *, ctx: ToolContext, state: ExplorerState) -> None:
        self._ctx = ctx
        self._state = state
        # Lazily-built audit logger for Pass 1 hits. The canonical path is
        # `{workspace}/.codebus/sanitize_audit.jsonl` (same file Pass 2 +
        # scanner Pass 1 append to). Callers don't need to plumb a logger
        # through ToolContext — the workspace root is authoritative.
        self._sanitize_audit: SanitizerAuditLogger | None = None
        # tool_audit.jsonl lives alongside sanitize_audit.jsonl.
        self._tool_audit_path = (
            ctx.workspace_root / ".codebus" / "tool_audit.jsonl"
        )

    # ------------------------------------------------------------------
    # Audit helpers — dispatch through the shared writer so FolderTools
    # and ToolSandbox produce identical line shapes.
    # ------------------------------------------------------------------

    def _audit_allow(
        self, tool_name: str, args: dict[str, Any], resolved_path: Path | None
    ) -> None:
        append_tool_audit_line(
            audit_path=self._tool_audit_path,
            lock=None,
            tool_name=tool_name,
            args_summary={
                k: args[k]
                for k in _AUDIT_FIELDS.get(tool_name, [])
                if k in args
            },
            resolved_path=str(resolved_path) if resolved_path is not None else None,
            allowed=True,
            denial_reason=None,
            ctx=self._ctx,
        )

    def _audit_deny(
        self, tool_name: str, args: dict[str, Any], requested_path: str
    ) -> None:
        append_tool_audit_line(
            audit_path=self._tool_audit_path,
            lock=None,
            tool_name=tool_name,
            args_summary={
                k: args[k]
                for k in _AUDIT_FIELDS.get(tool_name, [])
                if k in args
            },
            resolved_path=None,
            allowed=False,
            denial_reason=_classify_denial(requested_path),
            ctx=self._ctx,
        )

    # ------------------------------------------------------------------
    # Optional Protocol method — advertises the concrete tool surface
    # ------------------------------------------------------------------

    def tool_specs(self) -> list[dict]:
        """Return tool specs consumed by `render_explorer_prompt`.

        Descriptions align with `docs/agent-explorer-spec.md §三` so the
        prompt advertises the same signatures the LLM's tool_calls will
        use. `parameters` follows a loose JSON-schema-like shape — full
        JSON Schema emission lands with the P1 (`explorer-tools-p1`)
        change when `trace_import` / `find_callers` arrive.
        """
        return [
            {
                "name": "search",
                "description": "Find files matching a keyword. Prefers KB vector search when available, falls back to grep across code/doc extensions.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keyword": {"type": "string"},
                    },
                    "required": ["keyword"],
                },
            },
            {
                "name": "list_dir",
                "description": "List one level of entries under the given workspace-relative path.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                    },
                    "required": ["path"],
                },
            },
            {
                "name": "read_file",
                "description": "Read a file under the workspace. Output is sanitized via Pass 1 before returning; a line_range slices the view first.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "line_range": {
                            "type": "array",
                            "items": {"type": "integer"},
                            "minItems": 2,
                            "maxItems": 2,
                        },
                    },
                    "required": ["path"],
                },
            },
            {
                "name": "mark_station",
                "description": "Mark a file as a learning station with an Agent-chosen role and rationale.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "role": {"type": "string"},
                        "why": {"type": "string"},
                    },
                    "required": ["path", "role", "why"],
                },
            },
        ]

    # ------------------------------------------------------------------
    # ExplorerTools Protocol compliance (Q&A / Topic-mode future use)
    # ------------------------------------------------------------------

    async def primary_search(self, query: str) -> list[SearchHit]:
        """Protocol-level alias — delegates to `search` so shared agent
        code written against the abstract Protocol also reaches the real
        impl without needing to know tool names."""
        return await self.search(query)

    async def fetch(self, target: Target) -> Content:
        """Protocol-level fetch — P0 fold: if target.kind == 'file', map
        through `read_file`; otherwise return an empty Content so the
        shared Protocol stays satisfiable."""
        if target.kind == "file":
            path = target.args.get("path", "")
            text = await self.read_file(path)
            return Content(path=path, text=text)
        return Content(path="", text="")

    async def follow_reference(self, symbol: str) -> list[Target]:
        """P0: no reference-following yet (trace_import / find_callers
        land in `explorer-tools-p1`). Return empty list so the Protocol
        stays duck-typed-satisfied."""
        return []

    # ------------------------------------------------------------------
    # P0 concrete tools — Sections 9, 11, 13, 15 GREEN fill these in
    # ------------------------------------------------------------------

    async def search(self, keyword: str) -> list[SearchHit]:
        # search has no path arg → always allowed (no path-escape risk).
        # args_summary is empty because `keyword` is free-form Agent text
        # that can echo secrets (handled by `_AUDIT_FIELDS["search"] = []`).
        if self._ctx.kb is not None:
            hits = await self._search_via_kb(keyword)
        else:
            hits = self._search_via_grep(keyword)
        self._audit_allow("search", {"keyword": keyword}, None)
        return hits

    async def _search_via_kb(self, keyword: str) -> list[SearchHit]:
        kb_hits = await self._ctx.kb.query(keyword)
        results: list[SearchHit] = []
        ws_root = self._ctx.workspace_root
        for h in kb_hits:
            abs_path = h.payload.file_path or ""
            rel: str
            if abs_path:
                try:
                    rel = str(Path(abs_path).relative_to(ws_root))
                except ValueError:
                    # KB might store already-relative paths (Scanner's normal
                    # output) — fall through to using the stored value.
                    rel = abs_path
            else:
                rel = ""
            snippet = h.payload.text or ""
            if len(snippet) > 400:
                snippet = snippet[:400] + "..."
            # Clamp KB score defensively (should already be in [0, 1] but
            # Qdrant distance functions differ across backends).
            score = max(0.0, min(1.0, float(h.score)))
            results.append(SearchHit(path=rel, snippet=snippet, score=score))
        return results

    def _search_via_grep(self, keyword: str) -> list[SearchHit]:
        """Filesystem fallback — walks workspace, filters to text-file
        extensions the Scanner also keeps, caps to 100 hits."""
        allowed = {".py", ".md", ".ts", ".tsx", ".rs", ".go", ".js", ".jsx"}
        # Borrow Scanner's oversized threshold so grep results mirror the
        # file set KB would have indexed (binary + oversize are excluded).
        max_bytes = 512 * 1024
        results: list[SearchHit] = []
        ws_root = self._ctx.workspace_root
        for p in sorted(ws_root.rglob("*")):
            if len(results) >= 100:
                break
            if not p.is_file():
                continue
            if p.suffix.lower() not in allowed:
                continue
            # Exclude .codebus housekeeping
            try:
                rel_parts = p.relative_to(ws_root).parts
            except ValueError:
                continue
            if rel_parts and rel_parts[0] == ".codebus":
                continue
            try:
                size = p.stat().st_size
            except OSError:
                continue
            if size > max_bytes:
                continue
            try:
                text = p.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            occurrences = text.count(keyword)
            if occurrences == 0:
                continue
            # Build a single snippet around the first occurrence
            idx = text.find(keyword)
            snippet_start = max(idx - 40, 0)
            snippet_end = min(idx + len(keyword) + 120, len(text))
            snippet = text[snippet_start:snippet_end].replace("\n", " ")
            if len(snippet) > 400:
                snippet = snippet[:400] + "..."
            # Deterministic score: occurrence density clamped to (0, 1].
            density = occurrences / max(size, 1)
            score = max(0.0, min(1.0, density * 200))  # scale to sensible range
            if score == 0.0:
                score = 0.01  # occurrences > 0 should never be zero relevance
            rel = str(p.relative_to(ws_root)).replace("\\", "/")
            results.append(SearchHit(path=rel, snippet=snippet, score=score))
        return results

    async def list_dir(self, path: str) -> list[DirEntry]:
        try:
            resolved = ensure_in_workspace(path, self._ctx)
        except PathEscapeError:
            self._audit_deny("list_dir", {"path": path}, path)
            raise
        entries: list[DirEntry] = []
        for child in sorted(resolved.iterdir(), key=lambda p: p.name):
            # Skip the workspace audit directory so the LLM doesn't see
            # `.codebus/` as a first-class target. The directory exists
            # for sidecar housekeeping (sanitize_audit.jsonl /
            # tool_audit.jsonl) and is meaningless to the Agent.
            if child.name == ".codebus":
                continue
            try:
                size = child.stat().st_size if child.is_file() else 0
            except OSError:
                size = 0
            kind = "dir" if child.is_dir() else "file"
            entries.append(DirEntry(name=child.name, kind=kind, size=size))
        self._audit_allow("list_dir", {"path": path}, resolved)
        return entries

    async def read_file(
        self, path: str, line_range: tuple[int, int] | None = None
    ) -> str:
        try:
            resolved = ensure_in_workspace(path, self._ctx)
        except PathEscapeError:
            audit_args: dict[str, Any] = {"path": path}
            if line_range is not None:
                audit_args["line_range"] = list(line_range)
            self._audit_deny("read_file", audit_args, path)
            raise
        if self._ctx.sanitizer is None:
            # Fail-loud per spec scenario `Missing sanitizer fails loud`.
            # Do NOT include file content in the message — the whole point
            # of invariant #3 is that raw content never escapes unsanitized.
            raise ValueError(
                f"read_file requires ctx.sanitizer to be configured "
                f"(path={path!r}); raw content MUST NOT reach the Agent "
                f"without Pass 1 redaction"
            )

        # Load text with a forgiving encoding fallback — Scanner's
        # `encoding.decode_text` is the authoritative helper, but depending
        # on module layout that import may be heavy here. Use UTF-8 strict
        # first then replace-error fallback (binary files are filtered out
        # upstream by the Scanner / `.codebus` exclusion; tool output is
        # still resilient to a single stray byte).
        try:
            raw = resolved.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            raw = resolved.read_text(encoding="utf-8", errors="replace")

        # Slice BEFORE sanitize so line_range semantics mirror the file
        # view the Agent asked for.
        if line_range is not None:
            start, end = line_range
            all_lines = raw.splitlines(keepends=True)
            # 1-indexed inclusive per spec / docs/agent-explorer-spec.md §三
            sliced = all_lines[max(start - 1, 0) : max(end, 0)]
            text = "".join(sliced)
        else:
            text = raw

        # Truncate BEFORE sanitize so the redacted view still fits the
        # LLM-facing limit. Head + tail preserves entry-point signal while
        # respecting the budget.
        text = self._truncate_if_large(text)

        # Sanitize Pass 1 — engine emits placeholders; audit logger pins
        # each hit to sanitize_audit.jsonl with pass_num=1.
        result = self._ctx.sanitizer.sanitize(
            text, source=MessageSource(message_id=f"read_file:{path}")
        )
        audit_logger = self._get_sanitize_audit_logger()
        for entry in result.entries:
            audit_logger.append(
                entry=entry,
                pass_num=1,
                rules_version=_SANITIZE_RULES_VERSION,
                session_id=self._ctx.session_id or "sess-unknown",
            )
        audit_args = {"path": path}
        if line_range is not None:
            audit_args["line_range"] = list(line_range)
        self._audit_allow("read_file", audit_args, resolved)
        return result.text

    def _truncate_if_large(self, text: str) -> str:
        """Head + tail truncation with a marker; no-op when under limit."""
        if len(text) <= _READ_FILE_TRUNCATE_LIMIT:
            return text
        half = (_READ_FILE_TRUNCATE_LIMIT - len(_TRUNCATE_MARKER)) // 2
        return text[:half] + _TRUNCATE_MARKER + text[-half:]

    def _get_sanitize_audit_logger(self) -> SanitizerAuditLogger:
        if self._sanitize_audit is None:
            audit_dir = self._ctx.workspace_root / ".codebus"
            audit_dir.mkdir(exist_ok=True)
            self._sanitize_audit = SanitizerAuditLogger(
                audit_dir / "sanitize_audit.jsonl"
            )
        return self._sanitize_audit

    async def mark_station(self, path: str, role: str, why: str) -> None:
        # Pathsafety first — reject escapes before touching state.
        try:
            resolved = ensure_in_workspace(path, self._ctx)
        except PathEscapeError:
            self._audit_deny(
                "mark_station", {"path": path, "role": role}, path
            )
            raise

        # Idempotency: identical (path, role, why) collapse to one entry
        # so the LLM can over-call without cluttering the exploration
        # route. Matching is exact; a different `why` counts as distinct
        # because the rationale is part of the Agent's intent.
        for existing in self._state.stations:
            if (
                existing.path == path
                and existing.role == role
                and existing.why == why
            ):
                # Idempotent return still counts as a tool invocation —
                # write one audit line so replay counts are accurate.
                self._audit_allow(
                    "mark_station", {"path": path, "role": role}, resolved
                )
                return None

        self._state.stations.append(
            Station(
                path=path,
                role=role,
                relevance=_STATION_RELEVANCE_P0,
                why=why,
                depends_on=[],
            )
        )
        self._audit_allow(
            "mark_station", {"path": path, "role": role}, resolved
        )
        return None
