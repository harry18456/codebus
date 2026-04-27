## MODIFIED Requirements

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
