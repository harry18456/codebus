"""FolderTools — concrete Folder-mode Explorer tool surface.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools
  Requirement: search consults KB first then falls back to grep
  Requirement: read_file sanitizes output via Pass 1 before returning to Agent
  Requirement: list_dir and read_file enforce ensure_in_workspace
  Requirement: mark_station mutates state without calling LLM
openspec/changes/explorer-tools-p1/specs/explorer-tools/spec.md
  Requirement: trace_import resolves symbols to definition paths via regex
  Requirement: find_callers returns sanitized call-site FileMatches

This class implements four concrete P0 tools (`search`, `list_dir`,
`read_file`, `mark_station`) plus two P1 differentiated weapons
(`trace_import`, `find_callers`), AND also satisfies the abstract
``ExplorerTools`` Protocol seams (`primary_search` / `fetch` /
`follow_reference`) so Q&A Agent / Topic-mode impls can plug into the
same loop without touching this file.

The Explorer loop dispatches by concrete method name — ``getattr(tools,
call.name)`` lands directly on one of the six methods.
"""
from __future__ import annotations

import re
from pathlib import Path
from typing import TYPE_CHECKING, Any

from codebus_agent._audit_paths import (
    _SANITIZE_AUDIT_FILENAME,
    _TOOL_AUDIT_FILENAME,
    _WORKSPACE_AUDIT_SUBDIR,
)
from codebus_agent.agent.protocols import Content, Target
from codebus_agent.agent.tools.schemas import DirEntry, FileMatch, SearchHit
from codebus_agent.agent.types import ExplorerState, Station
from codebus_agent.sandbox import (
    PathEscapeError,
    ToolContext,
    _classify_denial,
    append_tool_audit_line,
    ensure_in_workspace,
)
from codebus_agent.sanitizer import (
    RULES_VERSION as _SANITIZE_RULES_VERSION,
    FileSource,
    SanitizerAuditLogger,
    SanitizerEngine,
)

if TYPE_CHECKING:
    pass


__all__ = ["FolderTools"]


_STATION_RELEVANCE_P0: float = 0.8  # hardcoded per spec Non-Goals; tuned by explorer-golden-sample-p0
_READ_FILE_TRUNCATE_LIMIT: int = 12000  # chars; heuristic proxy for ≈ 3000 tokens
_TRUNCATE_MARKER: str = "\n[... truncated ...]\n"

# Text-file extensions used by both P1 symbol-navigation tools. Mirrors
# the P0 grep fallback allowlist so ``trace_import`` / ``find_callers``
# see the same file set ``search`` does.
_P1_ALLOWED_EXTS: frozenset[str] = frozenset(
    {".py", ".md", ".ts", ".tsx", ".rs", ".go", ".js", ".jsx"}
)
_P1_MAX_BYTES: int = 512 * 1024  # align with search grep
_FIND_CALLERS_GLOBAL_CAP: int = 100
_FIND_CALLERS_PER_FILE_CAP: int = 5
_FIND_CALLERS_SNIPPET_LIMIT: int = 200  # chars after sanitize

# Language-neutral definition-site regex templates. ``{sym}`` is replaced
# by ``re.escape(symbol)`` before compilation so user-supplied symbols
# carrying regex metacharacters (``foo.bar``) cannot wildcard-match
# unintended names (``foo_bar``). Each template is anchored at
# ``^\s*`` and terminates with ``\b`` so ``Bar`` does not match
# ``BarFoo``. Covers Python / TS / JS / Go / Rust families; markdown
# files participate in iteration but yield no hits unless the symbol
# happens to appear verbatim after a definition keyword.
_DEFINITION_PATTERN_TEMPLATES: tuple[str, ...] = (
    r"^\s*(?:async\s+)?def\s+{sym}\b",                       # Python def / async def
    r"^\s*class\s+{sym}\b",                                  # Python / generic class
    r"^\s*(?:export\s+)?class\s+{sym}\b",                    # TS / JS class (export)
    r"^\s*(?:export\s+)?(?:async\s+)?function\s+{sym}\b",    # TS / JS function
    r"^\s*(?:export\s+)?(?:const|let|var)\s+{sym}\b",        # TS / JS const / let / var
    r"^\s*func\s+(?:\([^)]+\)\s+)?{sym}\b",                  # Go func (with optional receiver)
    r"^\s*type\s+{sym}\b",                                   # Go type
    r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+{sym}\b",             # Rust fn (pub / async)
    r"^\s*(?:pub\s+)?(?:struct|enum|trait)\s+{sym}\b",       # Rust struct / enum / trait
)

# Audit field whitelist per tool — keep keyword/why/symbol out of
# args_summary since they carry Agent free-form text that can
# accidentally echo sensitive snippets. Path-like args stay whitelisted
# so auditors can reconstruct tool dispatch history without reading
# the raw log. Aligns with openspec/specs/tool-sandbox/spec.md
# `Tools declare their auditable field whitelist`.
_AUDIT_FIELDS: dict[str, list[str]] = {
    "search": [],
    "list_dir": ["path"],
    "read_file": ["path", "line_range"],
    "mark_station": ["path", "role"],
    "trace_import": [],
    "find_callers": [],
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
            ctx.workspace_root / _WORKSPACE_AUDIT_SUBDIR / _TOOL_AUDIT_FILENAME
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
        use. `parameters` follows a loose JSON-schema-like shape. The
        P1 entries (``trace_import`` / ``find_callers``) landed with
        `explorer-tools-p1`; they complete the symbol-navigation
        surface alongside the P0 four.
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
            {
                "name": "trace_import",
                "description": "Resolve a symbol name to the workspace-relative path where it is defined. Scans Python / TS / JS / Go / Rust definition-site patterns and returns the first match, or null when the symbol is not defined anywhere.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "symbol": {"type": "string"},
                    },
                    "required": ["symbol"],
                },
            },
            {
                "name": "find_callers",
                "description": "Find every call-site of a symbol across the workspace. Returns FileMatch(path, line, snippet) entries with sanitized snippets; capped at 100 total / 5 per file; definition-site line is excluded.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "symbol": {"type": "string"},
                    },
                    "required": ["symbol"],
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
            hits = await self._search_via_grep(keyword)
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

    async def _search_via_grep(self, keyword: str) -> list[SearchHit]:
        """Filesystem fallback — walks workspace, filters to text-file
        extensions the Scanner also keeps, caps to 100 hits.

        Each hit's snippet is run through Pass 1 sanitize before being
        wrapped in a `SearchHit` so the grep path mirrors the KB path
        (which is sanitized at build time). `ctx.sanitizer is None`
        fails loud — invariant #3 (`LLM 看到的一定是 Sanitize 過的`)
        forbids the fallback from leaking raw snippets.
        """
        if self._ctx.sanitizer is None:
            raise ValueError(
                "search via grep fallback requires ctx.sanitizer to be "
                "configured; raw snippets MUST NOT reach the Agent without "
                "Pass 1 redaction"
            )
        audit_logger = self._get_sanitize_audit_logger()
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
            # Pass 1 sanitize on the grep-fallback snippet — `FileSource`
            # keeps the cross-cutting `pass_num to source-type invariant`
            # (pass=1 → file-source) intact. Each hit costs one sanitize
            # call; grep fallback is a cold path (KB hit short-circuits)
            # so per-hit overhead is acceptable.
            sanitized = await self._ctx.sanitizer.sanitize(
                snippet,
                source=FileSource(path=rel, pass_="grep_search"),
            )
            for entry in sanitized.entries:
                audit_logger.append(
                    entry=entry,
                    pass_num=1,
                    rules_version=_SANITIZE_RULES_VERSION,
                    session_id=self._ctx.session_id or "sess-unknown",
                )
            results.append(
                SearchHit(path=rel, snippet=sanitized.text, score=score)
            )
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
        # each hit to sanitize_audit.jsonl with pass_num=1. The audit line
        # carries `FileSource(path=<workspace-relative>, pass_="explorer_read_file")`
        # so the cross-cutting `pass_num to source-type invariant`
        # (`sanitizer` capability) holds: pass=1 → file-source.
        rel_for_source = (
            str(resolved.relative_to(self._ctx.workspace_root)).replace("\\", "/")
            if resolved.is_absolute()
            else path.replace("\\", "/")
        )
        result = await self._ctx.sanitizer.sanitize(
            text,
            source=FileSource(path=rel_for_source, pass_="explorer_read_file"),
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
            audit_dir = self._ctx.workspace_root / _WORKSPACE_AUDIT_SUBDIR
            audit_dir.mkdir(exist_ok=True)
            self._sanitize_audit = SanitizerAuditLogger(
                audit_dir / _SANITIZE_AUDIT_FILENAME
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

    # ------------------------------------------------------------------
    # P1 differentiated weapons — symbol-level navigation
    # (openspec/changes/explorer-tools-p1)
    # ------------------------------------------------------------------

    async def trace_import(self, symbol: str) -> str | None:
        """Resolve ``symbol`` to its defining file (workspace-relative path).

        Iterates allowed text-file extensions in deterministic order
        (``(path_depth, relative_path)``) and returns the first match of
        any language-neutral definition-site pattern. Symlinks escaping
        the workspace are rejected by ``ensure_in_workspace`` and logged
        to ``tool_audit.jsonl`` with ``allowed=false``. The final
        outcome (match path or ``None``) is logged as one additional
        ``tool_audit.jsonl`` line.
        """
        escaped = re.escape(symbol)
        combined = re.compile(
            "|".join(
                f"(?:{template.format(sym=escaped)})"
                for template in _DEFINITION_PATTERN_TEMPLATES
            ),
            re.MULTILINE,
        )

        ordered = self._iter_allowed_paths_sorted()
        for _depth, rel_str, abs_path in ordered:
            try:
                resolved = ensure_in_workspace(rel_str, self._ctx)
            except PathEscapeError:
                self._audit_deny("trace_import", {"symbol": symbol}, rel_str)
                continue
            text = self._read_text_or_none(resolved)
            if text is None:
                continue
            if combined.search(text):
                self._audit_allow("trace_import", {"symbol": symbol}, resolved)
                return rel_str

        # Exhausted — log one overall-outcome line with no resolved path.
        self._audit_allow("trace_import", {"symbol": symbol}, None)
        return None

    async def find_callers(self, symbol: str) -> list[FileMatch]:
        """Return sanitized call-site FileMatches for ``symbol``.

        Whole-word ``\\b<escaped_symbol>\\b`` match across the P1 allowed
        extensions. Per-file cap of 5, global cap of 100; sort key
        ``(path_depth, path, line)``. Snippets pass through Pass 1
        sanitize and truncate at 200 chars; ``ctx.sanitizer=None`` fails
        loud without touching the filesystem. Definition-site line
        (resolved via ``trace_import``) is excluded from results.
        """
        if self._ctx.sanitizer is None:
            # Fail-loud matches ``read_file`` invariant — raw source
            # MUST NOT leak into the Agent without Pass 1 redaction.
            raise ValueError(
                f"find_callers requires ctx.sanitizer to be configured "
                f"(symbol={symbol!r}); raw call-site snippets MUST NOT "
                f"reach the Agent without Pass 1 redaction"
            )

        definition_path = await self.trace_import(symbol)
        definition_line = (
            self._find_definition_line(definition_path, symbol)
            if definition_path is not None
            else None
        )

        escaped = re.escape(symbol)
        call_pattern = re.compile(rf"\b{escaped}\b")

        # (path_depth, rel_str, line_no, raw_line)
        raw_matches: list[tuple[int, str, int, str]] = []
        for depth, rel_str, abs_path in self._iter_allowed_paths_sorted():
            try:
                resolved = ensure_in_workspace(rel_str, self._ctx)
            except PathEscapeError:
                self._audit_deny("find_callers", {"symbol": symbol}, rel_str)
                continue
            text = self._read_text_or_none(resolved)
            if text is None:
                continue
            per_file_kept = 0
            for line_no, line in enumerate(text.splitlines(), start=1):
                if not call_pattern.search(line):
                    continue
                if (
                    definition_path == rel_str
                    and definition_line is not None
                    and line_no == definition_line
                ):
                    # Skip the definition-site line; no per-file slot consumed.
                    continue
                raw_matches.append((depth, rel_str, line_no, line))
                per_file_kept += 1
                if per_file_kept >= _FIND_CALLERS_PER_FILE_CAP:
                    break

        raw_matches.sort(key=lambda m: (m[0], m[1], m[2]))
        raw_matches = raw_matches[:_FIND_CALLERS_GLOBAL_CAP]

        audit_logger = self._get_sanitize_audit_logger()
        results: list[FileMatch] = []
        for _depth, rel_str, line_no, raw_line in raw_matches:
            # Pass 1 sanitize on each call-site snippet. `FileSource` keeps
            # the cross-cutting `pass_num to source-type invariant` (pass=1 →
            # file-source) intact; the call-site path is the natural source.
            sanitized = await self._ctx.sanitizer.sanitize(
                raw_line,
                source=FileSource(path=rel_str, pass_="find_callers"),
            )
            for entry in sanitized.entries:
                audit_logger.append(
                    entry=entry,
                    pass_num=1,
                    rules_version=_SANITIZE_RULES_VERSION,
                    session_id=self._ctx.session_id or "sess-unknown",
                )
            snippet = sanitized.text
            if len(snippet) > _FIND_CALLERS_SNIPPET_LIMIT:
                snippet = snippet[:_FIND_CALLERS_SNIPPET_LIMIT]
            results.append(
                FileMatch(path=rel_str, line=line_no, snippet=snippet)
            )

        self._audit_allow("find_callers", {"symbol": symbol}, None)
        return results

    # ------------------------------------------------------------------
    # Shared P1 helpers
    # ------------------------------------------------------------------

    def _iter_allowed_paths_sorted(
        self,
    ) -> list[tuple[int, str, Path]]:
        """Return every workspace text-file candidate sorted deterministically.

        Sort key: ``(path_depth, relative_path_str)``. Filter rules:

        - Extension in ``_P1_ALLOWED_EXTS``.
        - ``.codebus`` housekeeping directory excluded.
        - Size ≤ ``_P1_MAX_BYTES`` (mirrors grep fallback).

        The returned triples carry the absolute ``Path`` as-scanned; the
        caller still runs ``ensure_in_workspace`` on the relative string
        to catch symlinks that resolve outside the workspace.
        """
        ws_root = self._ctx.workspace_root
        collected: list[tuple[int, str, Path]] = []
        for p in ws_root.rglob("*"):
            if not p.is_file():
                continue
            if p.suffix.lower() not in _P1_ALLOWED_EXTS:
                continue
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
            if size > _P1_MAX_BYTES:
                continue
            rel_str = "/".join(rel_parts)
            collected.append((len(rel_parts), rel_str, p))
        collected.sort(key=lambda t: (t[0], t[1]))
        return collected

    @staticmethod
    def _read_text_or_none(resolved: Path) -> str | None:
        """Read ``resolved`` as UTF-8, returning ``None`` on failure.

        Mirrors the grep fallback's forgiving stance: files that fail
        decode / stat / open are skipped rather than raising so one
        corrupt file does not break a whole workspace scan.
        """
        try:
            return resolved.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            return None

    def _find_definition_line(
        self, rel_path: str, symbol: str
    ) -> int | None:
        """Return the 1-indexed line number of the first def-site hit in ``rel_path``.

        Used by ``find_callers`` to exclude the same line ``trace_import``
        resolved to. When the file cannot be read or no definition
        pattern matches (e.g. sibling export re-exporting a symbol),
        returns ``None`` so no exclusion happens and every hit survives.
        """
        try:
            resolved = ensure_in_workspace(rel_path, self._ctx)
        except PathEscapeError:
            return None
        text = self._read_text_or_none(resolved)
        if text is None:
            return None
        escaped = re.escape(symbol)
        combined = re.compile(
            "|".join(
                f"(?:{template.format(sym=escaped)})"
                for template in _DEFINITION_PATTERN_TEMPLATES
            )
        )
        for line_no, line in enumerate(text.splitlines(), start=1):
            if combined.search(line):
                return line_no
        return None
