## ADDED Requirements

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


### Requirement: find_callers returns sanitized call-site FileMatches

The sidecar SHALL expose `codebus_agent.agent.tools.folder_tools.FolderTools.find_callers(symbol: str) -> list[FileMatch]`. The method MUST scan the same text-file extension allowlist used by `trace_import` for whole-word occurrences of `symbol` (pattern `\b<escaped_symbol>\b`) and MUST return each occurrence as a `FileMatch(path: str, line: int, snippet: str)` object. `path` MUST be relative to `ctx.workspace_root`. `line` MUST be 1-indexed. `snippet` MUST be the occurrence's source line passed through `ctx.sanitizer.sanitize(...)` (Pass 1) and truncated at 200 characters.

The returned list MUST exclude the line returned by `trace_import(symbol)` when that method produces a non-`None` path (definition-site exclusion). The list MUST be capped at 100 entries globally and at 5 entries per distinct file. The list MUST be sorted deterministically by `(path_depth, path, line)`.

`find_callers` MUST fail loud (`ValueError`) when `ctx.sanitizer is None`, matching the invariant established by `read_file`. Every Pass 1 hit produced while sanitizing snippets MUST append one line to `sanitize_audit.jsonl` with `pass_num=1`. Every invocation MUST write one `tool_audit.jsonl` line via `sandbox.append_tool_audit_line` recording the tool name, symbol argument, and `allowed` outcome.

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
- **AND** `sanitize_audit.jsonl` MUST have at least one new line with `pass_num=1`

#### Scenario: Missing sanitizer fails loud

- **WHEN** `find_callers("anything")` is invoked with `ctx.sanitizer=None`
- **THEN** the call MUST raise `ValueError` naming the missing sanitizer
- **AND** the Explorer loop's `_execute_one` MUST capture the error into `ToolResult.error` without raw source content leaking into `output`

#### Scenario: Symbol with zero matches returns empty list

- **WHEN** `find_callers("ZzzNoSuchName")` is invoked on an otherwise-populated workspace
- **THEN** the method MUST return `[]`
- **AND** the method MUST NOT raise


## MODIFIED Requirements

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
