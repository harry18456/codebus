## ADDED Requirements

### Requirement: Folder-mode Explorer exposes four P0 tools

The sidecar SHALL expose a `codebus_agent.agent.tools.folder_tools.FolderTools` class whose instance methods implement the four P0 Folder-mode tools defined in `docs/agent-explorer-spec.md §三` P0 subset: `search(keyword: str) -> list[SearchHit]`, `list_dir(path: str) -> list[DirEntry]`, `read_file(path: str, line_range: tuple[int, int] | None = None) -> str`, and `mark_station(path: str, role: str, why: str) -> None`. The class MUST satisfy the existing `codebus_agent.agent.protocols.ExplorerTools` Protocol AND carry these four additional methods so the Explorer loop's `getattr(tools, call.name)` dispatch reaches them directly from `ExplorerAction.tool_calls[*].name`.

`FolderTools` MUST be constructed with a workspace-scoped `ToolContext` and MUST NOT hold mutable state between iterations other than the `ExplorerState.stations` it updates via `mark_station` (supplied by the caller as a reference so the Explorer loop sees appends).

#### Scenario: FolderTools satisfies ExplorerTools structurally

- **WHEN** a `FolderTools` instance is passed to `run_explorer(tools=...)`
- **THEN** `isinstance(tools, ExplorerTools)` MUST return True via the existing `runtime_checkable` decorator
- **AND** the loop MUST accept it as the `tools` argument without type error

#### Scenario: Tool dispatch by ExplorerAction.tool_calls name

- **WHEN** an `ExplorerAction` from `_think` carries `tool_calls=[ToolCall(name="search", arguments={"keyword": "KnowledgeBase"})]`
- **THEN** `_execute_one(call, tools)` MUST invoke `tools.search(keyword="KnowledgeBase")` and wrap the result into a `ToolResult`
- **AND** the same dispatch path MUST work for `list_dir`, `read_file`, `mark_station`

#### Scenario: Unknown tool name yields ToolResult.error without raising

- **WHEN** an `ExplorerAction` emits `ToolCall(name="trace_import", ...)` before the P1 change lands
- **THEN** `_execute_one` MUST return a `ToolResult` with `error` set to a message naming the missing tool
- **AND** the Explorer loop MUST proceed to the next iteration without raising

---

### Requirement: search consults KB first then falls back to grep

The `FolderTools.search(keyword)` method SHALL first attempt a KB query via `ctx.kb.query(keyword)` (using the existing Module 2 `KnowledgeBase` client on the `ToolContext`) when `ctx.kb is not None`. Each returned KB match MUST be mapped to a `SearchHit(path, snippet, score)` where `path` is relative to `ctx.workspace_root`, `snippet` is the embedded chunk text (≤ 400 chars), and `score` is the KB's similarity score clamped to `[0, 1]`.

When `ctx.kb is None`, `search` SHALL fall back to a filesystem grep across text-file extensions (`.py`, `.md`, `.ts`, `.tsx`, `.rs`, `.go`, `.js`, `.jsx`) within the workspace. The fallback MUST cap results at 100 hits and MUST derive `score` as a deterministic heuristic (e.g. occurrence count normalized by file size). Grep fallback MUST NOT scan files rejected by the existing Scanner text-file filter (binary / too-large).

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

---

### Requirement: read_file sanitizes output via Pass 1 before returning to Agent

`FolderTools.read_file(path, line_range=None)` SHALL pass the loaded file content through `ctx.sanitizer.sanitize(...)` (Pass 1, the same `SanitizerEngine` the Scanner uses) before returning the string to the Agent. The return value MUST be the post-sanitize text. Each sanitize hit MUST append one entry to `sanitize_audit.jsonl` via the existing `SanitizerAuditLogger` wiring carried on the `ToolContext`.

When `ctx.sanitizer is None`, `read_file` MUST NOT silently return raw content — it SHALL raise `ValueError` with a message naming the missing engine. This fail-loud rule aligns with invariant #3 (`LLM 看到的一定是 Sanitize 過的`) from `CLAUDE.md`.

If `line_range=(start, end)` is provided (1-indexed inclusive), `read_file` MUST slice lines FIRST then sanitize the slice. Files exceeding ~3000 tokens (heuristic: > 12000 chars when no line_range given) MUST be truncated to a head + tail window summing ≤ 12000 chars, with a `[... truncated ...]` marker between segments; truncation MUST occur BEFORE sanitize so the returned content still reflects the redacted view of each surviving segment.

#### Scenario: Pass 1 runs on every read_file call

- **WHEN** `read_file("src/app.py")` is invoked on a file containing a detected secret (e.g. `AKIA...`)
- **THEN** the returned string MUST contain `<REDACTED:` placeholder(s) and MUST NOT contain the raw secret
- **AND** `sanitize_audit.jsonl` MUST have at least one new line with `pass_num=1`

#### Scenario: Missing sanitizer fails loud

- **WHEN** `FolderTools.read_file(...)` runs with `ctx.sanitizer=None`
- **THEN** the call MUST raise `ValueError` naming the missing sanitizer
- **AND** the Explorer loop's `_execute_one` MUST capture the error into `ToolResult.error` without the raw file content leaking to the returned `output`

#### Scenario: Line range slices before sanitize

- **WHEN** `read_file("src/app.py", line_range=(10, 20))` is invoked
- **THEN** the returned string MUST contain only the sanitized content of lines 10 through 20 inclusive
- **AND** the sanitize_audit.jsonl entries MUST reflect only hits within that slice

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
