# explorer-tools Specification

## Purpose

TBD - created by archiving change 'explorer-tools-p0'. Update Purpose after archive.

## Requirements

### Requirement: Folder-mode Explorer exposes four P0 tools

The sidecar SHALL expose a `codebus_agent.agent.tools.folder_tools.FolderTools` class whose instance methods implement the four P0 Folder-mode tools defined in `docs/agent-explorer-spec.md §三` P0 subset: `search(keyword: str) -> list[SearchHit]`, `list_dir(path: str) -> list[DirEntry]`, `read_file(path: str, line_range: tuple[int, int] | None = None) -> str`, and `mark_station(path: str, role: str, why: str) -> None`. The class MUST satisfy the existing `codebus_agent.agent.protocols.ExplorerTools` Protocol AND carry these four additional methods so the Explorer loop's `getattr(tools, call.name)` dispatch reaches them directly from `ExplorerAction.tool_calls[*].name`. The class MUST also carry the P1 tools `trace_import` and `find_callers` alongside the P0 four once this change lands; the `_execute_one` dispatch treats any method whose name matches `ToolCall.name` as callable.

`FolderTools` MUST be constructed with a workspace-scoped `ToolContext` and MUST NOT hold mutable state between iterations other than the `ExplorerState.stations` it updates via `mark_station` (supplied by the caller as a reference so the Explorer loop sees appends).

#### Scenario: FolderTools satisfies ExplorerTools structurally

- **WHEN** a `FolderTools` instance is passed to `run_explorer(tools=...)`
- **THEN** `isinstance(tools, ExplorerTools)` MUST return True via the existing `runtime_checkable` decorator
- **AND** the loop MUST accept it as the `tools` argument without type error

#### Scenario: Tool dispatch by ExplorerAction.tool_calls name

- **WHEN** an `ExplorerAction` from `_think` carries `tool_calls=[ToolCall(name="search", arguments={"keyword": "KnowledgeBase"})]`
- **THEN** `_execute_one(call, tools)` MUST invoke `tools.search(keyword="KnowledgeBase")` and wrap the result into a `ToolResult`
- **AND** the same dispatch path MUST work for `list_dir`, `read_file`, `mark_station`, `trace_import`, and `find_callers`

#### Scenario: Unknown tool name yields ToolResult.error without raising

- **WHEN** an `ExplorerAction` emits `ToolCall(name="find_nonexistent", ...)` naming a method that `FolderTools` does not implement
- **THEN** `_execute_one` MUST return a `ToolResult` with `error` set to a message naming the missing tool
- **AND** the Explorer loop MUST proceed to the next iteration without raising


<!-- @trace
source: explorer-tools-p1
updated: 2026-04-24
code:
  - docs/agent-explorer-spec.md
  - sidecar/src/codebus_agent/agent/tools/__init__.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - CLAUDE.md
  - docs/tool-sandbox.md
  - sidecar/src/codebus_agent/agent/tools/schemas.py
tests:
  - sidecar/tests/agent/tools/test_find_callers.py
  - sidecar/tests/agent/tools/test_folder_tools_structural.py
  - sidecar/tests/agent/tools/test_schemas.py
  - sidecar/tests/agent/tools/test_tool_specs.py
  - sidecar/tests/agent/tools/test_trace_import.py
-->

---
### Requirement: search consults KB first then falls back to grep

The `FolderTools.search(keyword)` method SHALL first attempt a KB query via `ctx.kb.query(keyword)` (using the existing Module 2 `KnowledgeBase` client on the `ToolContext`) when `ctx.kb is not None`. Each returned KB match MUST be mapped to a `SearchHit(path, snippet, score)` where `path` is relative to `ctx.workspace_root`, `snippet` is the embedded chunk text (≤ 400 chars), and `score` is the KB's similarity score clamped to `[0, 1]`. The KB-path snippet has already been sanitized by Scanner Pass 1 at KB-build time, so this branch does not need to re-sanitize.

When `ctx.kb is None`, `search` SHALL fall back to a filesystem grep across text-file extensions (`.py`, `.md`, `.ts`, `.tsx`, `.rs`, `.go`, `.js`, `.jsx`) within the workspace. The fallback MUST cap results at 100 hits and MUST derive `score` as a deterministic heuristic (e.g. occurrence count normalized by file size). Grep fallback MUST NOT scan files rejected by the existing Scanner text-file filter (binary / too-large).

The grep fallback path MUST run each hit's snippet through `ctx.sanitizer.sanitize(snippet, source=FileSource(path=<hit_path>, pass_="grep_search"))` (Pass 1) before constructing the `SearchHit`. Each sanitize hit MUST append one entry to `<workspace>/.codebus/sanitize_audit.jsonl` via the existing `SanitizerAuditLogger` wiring carried on the `ToolContext`, with `pass_num=1`. This keeps the grep-fallback path behaviorally consistent with the KB path (both return sanitized snippets) and closes a defense-depth gap where workspaces without a populated KB would expose raw secrets in `SearchHit.snippet`. When `ctx.sanitizer is None` while taking the grep-fallback path, `search` MUST raise `ValueError` naming the missing sanitizer (fail-loud rule, aligns with `read_file` invariant).

`search` MUST NOT raise on empty results — an empty `list[SearchHit]` MUST be returned instead.

#### Scenario: KB path used when KB is configured

- **WHEN** `FolderTools.search("entry")` is invoked with `ctx.kb` bound to a non-None `KnowledgeBase`
- **THEN** the method MUST call `ctx.kb.query(...)` exactly once
- **AND** each returned match MUST appear in the result as a `SearchHit` whose `path` is relative to `ctx.workspace_root`

#### Scenario: Grep fallback when KB is absent

- **WHEN** `FolderTools.search("entry")` is invoked with `ctx.kb is None`
- **THEN** the method MUST NOT raise
- **AND** it MUST return `list[SearchHit]` whose length is at most 100
- **AND** each hit MUST reference a text-file extension from the allowed set

#### Scenario: Empty result when no match found

- **WHEN** `search("zzzzz_nonexistent_token")` is invoked on an otherwise-populated workspace
- **THEN** the return value MUST equal `[]` and MUST NOT raise

#### Scenario: Grep fallback hit snippet sanitized through Pass 1

- **WHEN** `FolderTools.search("authorize")` is invoked with `ctx.kb is None` against a workspace where `src/secrets.py` line 4 contains `authorize("AKIAIOSFODNN7EXAMPLE")`
- **THEN** the returned `SearchHit` for that file MUST have `snippet` containing a `<REDACTED:` placeholder
- **AND** the `snippet` MUST NOT contain the raw `AKIAIOSFODNN7EXAMPLE` literal
- **AND** `<workspace>/.codebus/sanitize_audit.jsonl` MUST have at least one new line with `pass_num=1` whose `source` reflects `FileSource(path="src/secrets.py", pass_="grep_search")` shape

#### Scenario: Grep fallback fails loud when sanitizer missing

- **WHEN** `FolderTools.search("anything")` is invoked with `ctx.kb is None` and `ctx.sanitizer is None`
- **THEN** the call MUST raise `ValueError` naming the missing sanitizer — the grep fallback MUST NOT silently return raw snippets


<!-- @trace
source: agent-defense-depth
updated: 2026-04-27
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/api/kb.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/kb/payload.py
tests:
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
-->

---
### Requirement: read_file sanitizes output via Pass 1 before returning to Agent

`FolderTools.read_file(path, line_range=None)` SHALL pass the loaded file content through `ctx.sanitizer.sanitize(...)` (Pass 1, the same `SanitizerEngine` the Scanner uses) before returning the string to the Agent. The return value MUST be the post-sanitize text. Each sanitize hit MUST append one entry to `<workspace>/.codebus/sanitize_audit.jsonl` via the existing `SanitizerAuditLogger` wiring carried on the `ToolContext`. Each appended audit line MUST carry `pass_num=1` AND a `FileSource(path=<resolved_workspace_relative_path>, pass_="explorer_read_file")` source — the sanitizer's source must reflect the file being read, NOT a `MessageSource(message_id=...)`. This invariant aligns with the `sanitizer` capability cross-cutting Scenario `pass_num to source-type invariant` (Pass 1 audit lines MUST carry file-source; Pass 2 audit lines MUST carry message-source).

When `ctx.sanitizer is None`, `read_file` MUST NOT silently return raw content — it SHALL raise `ValueError` with a message naming the missing engine. This fail-loud rule aligns with invariant #3 (`LLM 看到的一定是 Sanitize 過的`) from `CLAUDE.md`.

If `line_range=(start, end)` is provided (1-indexed inclusive), `read_file` MUST slice lines FIRST then sanitize the slice. Files exceeding ~3000 tokens (heuristic: > 12000 chars when no line_range given) MUST be truncated to a head + tail window summing ≤ 12000 chars, with a `[... truncated ...]` marker between segments; truncation MUST occur BEFORE sanitize so the returned content still reflects the redacted view of each surviving segment.

#### Scenario: Pass 1 runs on every read_file call

- **WHEN** `read_file("src/app.py")` is invoked on a file containing a detected secret (e.g. `AKIA...`)
- **THEN** the returned string MUST contain `<REDACTED:` placeholder(s) and MUST NOT contain the raw secret
- **AND** `<workspace>/.codebus/sanitize_audit.jsonl` MUST have at least one new line with `pass_num=1`

#### Scenario: Pass 1 audit line carries FileSource

- **WHEN** `read_file("src/app.py")` is invoked on a file containing a detected secret
- **THEN** the resulting `<workspace>/.codebus/sanitize_audit.jsonl` line MUST contain a `source` object whose serialized shape reflects `FileSource(path="src/app.py", pass_="explorer_read_file")`
- **AND** the audit line MUST NOT contain a `source` whose shape reflects `MessageSource(message_id=...)`
- **AND** the `pass_num` field MUST equal `1`

#### Scenario: Missing sanitizer fails loud

- **WHEN** `FolderTools.read_file(...)` runs with `ctx.sanitizer=None`
- **THEN** the call MUST raise `ValueError` naming the missing sanitizer
- **AND** the Explorer loop's `_execute_one` MUST capture the error into `ToolResult.error` without the raw file content leaking to the returned `output`

#### Scenario: Line range slices before sanitize

- **WHEN** `read_file("src/app.py", line_range=(10, 20))` is invoked
- **THEN** the returned string MUST contain only the sanitized content of lines 10 through 20 inclusive
- **AND** the sanitize_audit.jsonl entries MUST reflect only hits within that slice


<!-- @trace
source: agent-defense-depth
updated: 2026-04-27
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/api/kb.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/kb/payload.py
tests:
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
-->

---
### Requirement: list_dir and read_file enforce ensure_in_workspace

Both `FolderTools.list_dir(path)` and `FolderTools.read_file(path, ...)` SHALL dispatch every path argument through the existing `ensure_in_workspace(path, ctx)` helper (from `codebus_agent.sandbox`) BEFORE any filesystem I/O. A raised `PathEscapeError` MUST propagate; the Explorer loop's `_execute_one` captures it into `ToolResult.error` so the Agent sees `"ERROR: <reason>"` instead of escaped content.

`mark_station(path, ...)` SHALL also pass `path` through `ensure_in_workspace` so a station with a sandbox-escaping path cannot end up in `state.stations`.

#### Scenario: Parent-directory escape in read_file rejected

- **WHEN** `FolderTools.read_file("../../etc/passwd")` is invoked
- **THEN** `ensure_in_workspace` MUST raise `PathEscapeError`
- **AND** no bytes from `/etc/passwd` MUST reach the return value or any audit log
- **AND** `tool_audit.jsonl` MUST have one line with `allowed=false, denial_reason="path_escape"`

#### Scenario: Symlink escape in list_dir rejected

- **WHEN** `FolderTools.list_dir("link_to_outside")` is invoked where `link_to_outside` is a symlink pointing outside the workspace
- **THEN** `ensure_in_workspace` MUST resolve the symlink and raise `PathEscapeError`
- **AND** no entries from the symlink target MUST appear in the return value

#### Scenario: mark_station with out-of-workspace path rejected

- **WHEN** `FolderTools.mark_station("C:/Windows/System32/kernel32.dll", role="seed", why="...")` is invoked inside a workspace rooted at `D:/codebus`
- **THEN** `ensure_in_workspace` MUST raise `PathEscapeError`
- **AND** `state.stations` MUST NOT grow


<!-- @trace
source: explorer-tools-p0
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/sandbox.py
tests:
  - sidecar/tests/agent/tools/test_list_dir.py
  - sidecar/tests/agent/tools/test_read_file.py
  - sidecar/tests/agent/tools/test_mark_station.py
-->


<!-- @trace
source: explorer-tools-p0
updated: 2026-04-24
code:
  - docs/agent-explorer-spec.md
  - sidecar/src/codebus_agent/agent/protocols.py
  - sidecar/src/codebus_agent/agent/tools/__init__.py
  - docs/tool-sandbox.md
  - sidecar/src/codebus_agent/agent/tools/schemas.py
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
tests:
  - sidecar/tests/agent/tools/test_read_file.py
  - sidecar/tests/agent/tools/test_search.py
  - sidecar/tests/agent/test_explorer_loop_with_real_tools.py
  - sidecar/tests/agent/test_protocols.py
  - sidecar/tests/agent/tools/test_folder_tools_structural.py
  - sidecar/tests/agent/test_explorer_loop.py
  - sidecar/tests/sandbox/test_tool_context_optional_deps.py
  - sidecar/tests/agent/tools/test_mark_station.py
  - sidecar/tests/agent/tools/__init__.py
  - sidecar/tests/agent/tools/test_folder_tools_audit.py
  - sidecar/tests/agent/tools/conftest.py
  - sidecar/tests/agent/tools/test_tool_specs.py
  - sidecar/tests/agent/tools/test_list_dir.py
  - sidecar/tests/agent/tools/test_schemas.py
-->

---
### Requirement: mark_station mutates state without calling LLM

The `FolderTools.mark_station(path, role, why)` method SHALL append a `Station(path, role, relevance=0.8, why, depends_on=[])` entry to the Explorer state's stations list (the `FolderTools` constructor receives the state reference so the method can mutate it) and SHALL return `None`. The method MUST NOT invoke any LLM provider — Agent intent is taken at face value; Judge performs the one-shot relevance scoring separately at the loop level.

The P0 `relevance` default is `0.8` (hardcoded constant). Tuning the relevance score per station lands in the follow-up `explorer-golden-sample-p0` change (step 23).

#### Scenario: mark_station appends to state without LLM

- **WHEN** `FolderTools.mark_station("src/app.py", role="entry", why="main handler")` is invoked
- **THEN** `state.stations` MUST grow by exactly one `Station` entry with `path="src/app.py"`, `role="entry"`, `why="main handler"`, `relevance=0.8`
- **AND** no `provider.chat` call MUST fire from inside `mark_station`
- **AND** the method MUST return `None`

#### Scenario: mark_station is idempotent for identical inputs

- **WHEN** `mark_station("src/app.py", role="entry", why="seed")` is invoked twice in the same session with identical arguments
- **THEN** `state.stations` MUST contain exactly one entry matching those fields
- **AND** no `ValueError` or duplicate-entry warning MUST be raised


<!-- @trace
source: explorer-tools-p0
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/types.py
tests:
  - sidecar/tests/agent/tools/test_mark_station.py
-->

<!-- @trace
source: explorer-tools-p0
updated: 2026-04-24
code:
  - docs/agent-explorer-spec.md
  - sidecar/src/codebus_agent/agent/protocols.py
  - sidecar/src/codebus_agent/agent/tools/__init__.py
  - docs/tool-sandbox.md
  - sidecar/src/codebus_agent/agent/tools/schemas.py
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
tests:
  - sidecar/tests/agent/tools/test_read_file.py
  - sidecar/tests/agent/tools/test_search.py
  - sidecar/tests/agent/test_explorer_loop_with_real_tools.py
  - sidecar/tests/agent/test_protocols.py
  - sidecar/tests/agent/tools/test_folder_tools_structural.py
  - sidecar/tests/agent/test_explorer_loop.py
  - sidecar/tests/sandbox/test_tool_context_optional_deps.py
  - sidecar/tests/agent/tools/test_mark_station.py
  - sidecar/tests/agent/tools/__init__.py
  - sidecar/tests/agent/tools/test_folder_tools_audit.py
  - sidecar/tests/agent/tools/conftest.py
  - sidecar/tests/agent/tools/test_tool_specs.py
  - sidecar/tests/agent/tools/test_list_dir.py
  - sidecar/tests/agent/tools/test_schemas.py
-->

---
### Requirement: trace_import resolves symbols to definition paths via regex

The sidecar SHALL expose `codebus_agent.agent.tools.folder_tools.FolderTools.trace_import(symbol: str) -> str | None`. Given a symbol name, the method MUST scan the workspace under the existing text-file extension allowlist (`.py`, `.md`, `.ts`, `.tsx`, `.rs`, `.go`, `.js`, `.jsx`) using language-neutral definition-site regular expressions and return the first matching file's path relative to `ctx.workspace_root`, or `None` when no file defines `symbol`. The regex set MUST cover Python (`def` / `async def` / `class`), TypeScript / JavaScript (`class` / `function` / `async function` / `const` / `let` / `var`, with optional `export` prefix), Go (`func`, including method receivers; `type`), and Rust (`fn` / `async fn`; `struct` / `enum` / `trait`; with optional `pub` prefix). Every `symbol` input MUST be escaped via `re.escape` before substitution into the pattern templates.

The method MUST call `ensure_in_workspace(candidate_path, ctx)` for each candidate result before returning; a candidate whose resolution escapes the workspace MUST be discarded and MUST NOT appear in the return value. When multiple files contain matching definitions, the method MUST return the deterministic first candidate sorted by `(path_depth, relative_path)` where shorter depth wins; this ordering prevents non-deterministic returns across platforms.

The method MUST write one `tool_audit.jsonl` line per invocation via the shared `sandbox.append_tool_audit_line` writer, recording the tool name, symbol argument, and `allowed` outcome.

#### Scenario: Python def definition resolves to source path

- **WHEN** `trace_import("KnowledgeBase")` is invoked in a workspace containing `src/kb/base.py` with the line `class KnowledgeBase:` at line 12
- **THEN** the method MUST return `"src/kb/base.py"`
- **AND** `tool_audit.jsonl` MUST grow by one line with `tool="trace_import"`, `allowed=true`, and the symbol argument recorded

#### Scenario: TypeScript export function definition resolves

- **WHEN** `trace_import("makeProvider")` is invoked in a workspace whose `web/src/providers.ts` contains `export function makeProvider(` at line 5
- **THEN** the method MUST return `"web/src/providers.ts"`

#### Scenario: Rust pub async fn definition resolves

- **WHEN** `trace_import("handle_request")` is invoked in a workspace whose `crates/server/src/lib.rs` contains `pub async fn handle_request(` at line 30
- **THEN** the method MUST return `"crates/server/src/lib.rs"`

#### Scenario: Symbol not defined anywhere returns None

- **WHEN** `trace_import("Zzz_NotDefined")` is invoked on an otherwise-populated workspace
- **THEN** the method MUST return `None`
- **AND** the method MUST NOT raise

#### Scenario: Multiple definitions resolve to shortest-depth first

- **WHEN** `trace_import("Util")` is invoked in a workspace where `src/util.py` and `tests/helpers/util.py` both declare `class Util:`
- **THEN** the method MUST return `"src/util.py"` because it has a shorter `path_depth`

#### Scenario: Symlink escaping workspace is discarded

- **WHEN** `trace_import("ExternalSymbol")` is invoked in a workspace containing `link_to_outside.py` as a symlink pointing to a file outside the workspace that defines `class ExternalSymbol:`
- **THEN** `ensure_in_workspace` MUST reject the resolved path
- **AND** the method MUST return `None` (treating the symlink target as if it did not exist)
- **AND** `tool_audit.jsonl` MUST include at least one entry with `allowed=false` whose `denial_reason` names the path escape

#### Scenario: Symbol containing regex metacharacters is handled safely

- **WHEN** `trace_import("foo.bar")` is invoked (the dot is a regex metacharacter)
- **THEN** the method MUST NOT raise a regex compilation error
- **AND** the method MUST NOT treat `foo.bar` as a wildcard match for `foo_bar` or `fooXbar`


<!-- @trace
source: explorer-tools-p1
updated: 2026-04-24
code:
  - docs/agent-explorer-spec.md
  - sidecar/src/codebus_agent/agent/tools/__init__.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - CLAUDE.md
  - docs/tool-sandbox.md
  - sidecar/src/codebus_agent/agent/tools/schemas.py
tests:
  - sidecar/tests/agent/tools/test_find_callers.py
  - sidecar/tests/agent/tools/test_folder_tools_structural.py
  - sidecar/tests/agent/tools/test_schemas.py
  - sidecar/tests/agent/tools/test_tool_specs.py
  - sidecar/tests/agent/tools/test_trace_import.py
-->

---
### Requirement: find_callers returns sanitized call-site FileMatches

The sidecar SHALL expose `codebus_agent.agent.tools.folder_tools.FolderTools.find_callers(symbol: str) -> list[FileMatch]`. The method MUST scan the same text-file extension allowlist used by `trace_import` for whole-word occurrences of `symbol` (pattern `\b<escaped_symbol>\b`) and MUST return each occurrence as a `FileMatch(path: str, line: int, snippet: str)` object. `path` MUST be relative to `ctx.workspace_root`. `line` MUST be 1-indexed. `snippet` MUST be the occurrence's source line passed through `ctx.sanitizer.sanitize(...)` (Pass 1) and truncated at 200 characters.

The returned list MUST exclude the line returned by `trace_import(symbol)` when that method produces a non-`None` path (definition-site exclusion). The list MUST be capped at 100 entries globally and at 5 entries per distinct file. The list MUST be sorted deterministically by `(path_depth, path, line)`.

`find_callers` MUST fail loud (`ValueError`) when `ctx.sanitizer is None`, matching the invariant established by `read_file`. Every Pass 1 hit produced while sanitizing snippets MUST append one line to `<workspace>/.codebus/sanitize_audit.jsonl` with `pass_num=1` AND a `FileSource(path=<call_site_path>, pass_="find_callers")` source — the sanitizer's source must reflect the file containing the call site, NOT a `MessageSource(message_id=...)`. Every invocation MUST write one `tool_audit.jsonl` line via `sandbox.append_tool_audit_line` recording the tool name, symbol argument, and `allowed` outcome.

#### Scenario: Multiple call-sites return sanitized snippets

- **WHEN** `find_callers("KnowledgeBase")` is invoked in a workspace where `src/app.py` line 14 contains `kb = KnowledgeBase(path)` and `src/api/routes.py` line 30 contains `return KnowledgeBase.query(...)`
- **THEN** the method MUST return a list whose entries include `FileMatch(path="src/app.py", line=14, ...)` and `FileMatch(path="src/api/routes.py", line=30, ...)`
- **AND** each `snippet` MUST be the sanitized source line truncated at 200 characters

#### Scenario: Whole-word boundary rejects substring matches

- **WHEN** `find_callers("foo")` is invoked in a workspace where the only occurrence is `foobar(x)`
- **THEN** the returned list MUST be empty

#### Scenario: Definition site is excluded from results

- **WHEN** `trace_import("Bar")` would return `src/bar.py` (line 5 = `class Bar:`) and that same file contains `Bar()` on line 20
- **THEN** `find_callers("Bar")` MUST include the line-20 FileMatch and MUST NOT include the line-5 definition match

#### Scenario: Per-file cap limits snippet storm

- **WHEN** `find_callers("MAX")` is invoked in a workspace where `src/constants.py` contains 50 references to `MAX`
- **THEN** at most 5 FileMatches pointing to `src/constants.py` MUST appear in the returned list

#### Scenario: Global cap enforces 100-entry ceiling

- **WHEN** `find_callers("pass")` is invoked in a workspace with more than 100 whole-word occurrences across many files
- **THEN** the returned list MUST have `len(...) <= 100`

#### Scenario: Snippet sanitize redacts secrets before return

- **WHEN** `find_callers("authorize")` matches a line containing `authorize("AKIAIOSFODNN7EXAMPLE")`
- **THEN** the returned FileMatch's `snippet` MUST NOT contain the raw `AKIA...` string
- **AND** the `snippet` MUST contain a `<REDACTED:` placeholder
- **AND** `<workspace>/.codebus/sanitize_audit.jsonl` MUST have at least one new line with `pass_num=1`

#### Scenario: Pass 1 audit line carries FileSource

- **WHEN** `find_callers("authorize")` matches a line in `src/auth/login.py` containing a redacted secret
- **THEN** the resulting `<workspace>/.codebus/sanitize_audit.jsonl` line MUST contain a `source` object whose serialized shape reflects `FileSource(path="src/auth/login.py", pass_="find_callers")`
- **AND** the audit line MUST NOT contain a `source` whose shape reflects `MessageSource(message_id=...)`
- **AND** the `pass_num` field MUST equal `1`

#### Scenario: Missing sanitizer fails loud

- **WHEN** `find_callers("anything")` is invoked with `ctx.sanitizer=None`
- **THEN** the call MUST raise `ValueError` naming the missing sanitizer

<!-- @trace
source: agent-defense-depth
updated: 2026-04-27
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/api/kb.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/kb/payload.py
tests:
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
-->
