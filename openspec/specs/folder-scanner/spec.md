# folder-scanner Specification

## Purpose

TBD - created by archiving change 'scanner-skeleton'. Update Purpose after archive.

## Requirements

### Requirement: Workspace scan endpoint

The sidecar SHALL expose a `POST /scan` endpoint that accepts a JSON body `{workspace_type, workspace_root}` and returns a synchronous `ScanResult` JSON response. The endpoint SHALL enforce the existing bearer token and loopback-binding constraints defined by the `sidecar-runtime` capability; it MUST NOT bypass the bearer middleware.

#### Scenario: Successful scan of a folder workspace

- **WHEN** a client sends `POST /scan` with `{"workspace_type": "folder", "workspace_root": "<existing local path>"}` and a valid bearer token
- **THEN** the sidecar returns HTTP 200 with a `ScanResult` JSON body whose `workspace_root` matches the resolved absolute path, `files` contains one `FileEntry` per non-ignored file, and `scan_started_at` / `scan_completed_at` are ISO-8601 timestamps with `scan_completed_at >= scan_started_at`.

#### Scenario: Missing bearer token rejected

- **WHEN** a client sends `POST /scan` without the `Authorization: Bearer <token>` header
- **THEN** the sidecar returns HTTP 401 and does not execute any filesystem traversal.

#### Scenario: Nonexistent workspace root rejected

- **WHEN** a client sends `POST /scan` with a `workspace_root` path that does not exist or is unreadable
- **THEN** the sidecar returns HTTP 400 with an error code `SCANNER_WORKSPACE_INVALID` and does not produce a `ScanResult`.

---
### Requirement: Workspace type discriminator routing

The `POST /scan` endpoint SHALL accept `workspace_type` values from the set `{"folder", "topic"}`. Skeleton implementations MUST process `"folder"` and MUST return HTTP 501 for `"topic"`. Any other value MUST be rejected with HTTP 422.

#### Scenario: Folder workspace processed

- **WHEN** a client sends `POST /scan` with `workspace_type="folder"`
- **THEN** the sidecar executes the traversal pipeline and returns a `ScanResult`.

#### Scenario: Topic workspace returns 501

- **WHEN** a client sends `POST /scan` with `workspace_type="topic"`
- **THEN** the sidecar returns HTTP 501 with a response body whose `detail` identifies the unimplemented branch (e.g. `"workspace_type='topic' not implemented in MVP"`).

#### Scenario: Unknown discriminator rejected

- **WHEN** a client sends `POST /scan` with `workspace_type="file"` or any value outside the declared set
- **THEN** the sidecar returns HTTP 422 from Pydantic validation and does not execute traversal.

---
### Requirement: File tree traversal with gitignore stacking

The scanner SHALL traverse the `workspace_root` recursively and MUST apply the following ignore rules in order of precedence:

1. Built-in always-ignore prefixes: `.git/`, `.hg/`, `.svn/`, `node_modules/`, `.venv/`, `venv/`, `__pycache__/`, `.mypy_cache/`, `.pytest_cache/`, `dist/`, `build/`, `target/`, `out/`, `.DS_Store`, `Thumbs.db`.
2. `.gitignore` rules, stacked hierarchically using `pathspec` with the `gitwildmatch` syntax: the effective ignore set at any directory is the union of its parent directory's effective set plus the directory-local `.gitignore`.

Directories whose path matches the effective ignore set MUST NOT be descended into (the entire subtree is skipped). Files whose path matches MUST NOT appear in `ScanResult.files`.

#### Scenario: Built-in directories skipped without gitignore

- **WHEN** the scanner runs on a workspace containing `.git/` and `node_modules/` and no `.gitignore` file
- **THEN** no entries under `.git/` or `node_modules/` appear in `ScanResult.files` and the scanner does not descend into those directories.

#### Scenario: Nested gitignore stacks with parent

- **WHEN** the workspace has a root `.gitignore` with `*.log` and a subdirectory `sub/.gitignore` with `local.tmp`
- **THEN** files named `*.log` anywhere in the tree and `sub/local.tmp` are both excluded from `ScanResult.files`.

#### Scenario: Gitignore negation respected

- **WHEN** a `.gitignore` contains `*.log` followed by `!important.log`
- **THEN** `important.log` appears in `ScanResult.files` while other `*.log` files are excluded.

---
### Requirement: Symlink handling without following

The scanner SHALL NOT follow symbolic links during traversal. For each symlink encountered, the scanner MUST record a `Symlink` entry in `ScanResult.symlinks` with the fields `path`, `target`, and `resolved_in_workspace`, computed as follows:

- `path`: the symlink's path relative to `workspace_root`.
- `target`: the literal target string read from the symlink (not resolved).
- `resolved_in_workspace`: `true` if `Path(path).resolve(strict=False)` is inside `workspace_root`, otherwise `false`.

Symlink targets MUST NOT be read, traversed, or produce a `FileEntry`, regardless of the `resolved_in_workspace` value.

#### Scenario: Symlink pointing inside workspace recorded but not followed

- **WHEN** the workspace contains a symlink `link.py -> src/real.py` and `src/real.py` is a regular file
- **THEN** `ScanResult.symlinks` contains an entry with `resolved_in_workspace=true`, `ScanResult.files` contains exactly one `FileEntry` for `src/real.py` (not for `link.py`), and the scanner does not read `link.py` as a file.

#### Scenario: Symlink pointing outside workspace marked and skipped

- **WHEN** the workspace contains a symlink `escape -> /etc/passwd`
- **THEN** `ScanResult.symlinks` contains an entry with `resolved_in_workspace=false`, `ScanResult.files` contains no entry for `escape`, and the scanner does not read `/etc/passwd`.

---
### Requirement: File classification by extension and content sniffing

The scanner SHALL classify each non-symlink entry into exactly one of `{text, binary, oversized, lockfile, generated}` and populate `FileEntry.kind` accordingly. Classification MUST apply the following rules in order; the first matching rule wins:

1. `generated` if the filename matches `*.min.js`, `*.min.css`, or `*.bundle.js`.
2. `lockfile` if the filename matches `*-lock.json`, `yarn.lock`, `poetry.lock`, `Cargo.lock`, `uv.lock`, or `Gemfile.lock`.
3. `binary` if the extension is in a declared binary-extension set (including but not limited to `.png`, `.jpg`, `.pdf`, `.zip`, `.exe`, `.dll`, `.so`, `.dylib`, `.woff`, `.woff2`, `.ttf`).
4. `binary` if the first 8 KB of file content contains a null byte (`\x00`) or decoding against the entire fallback chain fails.
5. `oversized` if the file size exceeds `max_file_size_kb` (default 512 KB) and the file was not classified as binary by rules 1-4.
6. `text` otherwise.

For `binary`, `lockfile`, and `generated` entries, `FileEntry.content` MUST be `null` and `FileEntry.encoding` MUST be `null`. For `oversized` entries, `FileEntry.content` MUST be `null`; `FileEntry.oversized_preview` MAY contain up to the first 200 lines of the decoded head, and if populated in a future change that preview MUST itself be routed through the Pass 1 sanitizer before being stored.

For `text` entries, `FileEntry.content` MUST hold the **Pass 1 sanitized** string (see the "Pass 1 sanitizer orchestration for text FileEntries" requirement), not the raw decoded body. `FileEntry.encoding` MUST identify the encoding under which the raw bytes were successfully decoded.

#### Scenario: PNG classified as binary without reading full content

- **WHEN** the workspace contains `logo.png`
- **THEN** the corresponding `FileEntry` has `kind="binary"`, `content=null`, and the scanner does not read the full file bytes into memory.

#### Scenario: uv.lock classified as lockfile and content omitted

- **WHEN** the workspace contains `uv.lock`
- **THEN** the corresponding `FileEntry` has `kind="lockfile"` and `content=null`; the file size is still recorded in `FileEntry.size`.

#### Scenario: Minified JS classified as generated

- **WHEN** the workspace contains `app.min.js`
- **THEN** the corresponding `FileEntry` has `kind="generated"` and `content=null`.

#### Scenario: Small plain text file classified as text with sanitized content

- **WHEN** the workspace contains `README.md` of size 2 KB containing UTF-8 text with no sanitizer matches
- **THEN** the corresponding `FileEntry` has `kind="text"`, `FileEntry.content` equals the decoded file body (sanitizer returned it unchanged), `FileEntry.encoding="utf-8"`, and `FileEntry.sanitize_stats` equals `{}`.


<!-- @trace
source: scanner-sanitizer-orchestration
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/scanner/service.py
  - docs/module-1-scanner.md
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sandbox.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/scanner/test_scan_api.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/scanner/fixtures/with-secrets/config.py
  - sidecar/tests/scanner/fixtures/with-secrets/contacts.txt
  - sidecar/tests/scanner/fixtures/with-secrets/README.md
  - sidecar/tests/scanner/test_fixtures_integration.py
-->

---
### Requirement: Encoding detection fallback chain

The scanner SHALL attempt to decode each text-candidate file using the following encodings in order, selecting the first that succeeds: UTF-8 (with or without BOM), UTF-16 LE/BE (only when the corresponding BOM is present), Big5 (CP950), GBK (CP936), Shift_JIS (CP932), and finally `charset-normalizer` best-guess. If every attempt fails, the file MUST be re-classified as `binary` with `FileEntry.content=null`.

#### Scenario: UTF-8 file decoded on first attempt

- **WHEN** a file contains valid UTF-8 bytes
- **THEN** `FileEntry.encoding="utf-8"` and `FileEntry.content` is the correctly decoded string.

#### Scenario: Big5 file decoded after UTF-8 fails

- **WHEN** a file contains Big5-encoded Traditional Chinese text that fails UTF-8 decode
- **THEN** `FileEntry.encoding="big5"` and `FileEntry.content` is the correctly decoded string.

#### Scenario: Unrecognized encoding falls back to binary

- **WHEN** a non-empty file fails every fallback-chain encoding and `charset-normalizer` returns no viable guess
- **THEN** `FileEntry.kind="binary"`, `FileEntry.content=null`, and `FileEntry.encoding=null`.

---
### Requirement: Language identification

The scanner SHALL assign `FileEntry.language` using the following priority:

1. Extension lookup against a declared mapping (e.g. `.py -> python`, `.tsx -> typescript`, `.rs -> rust`, `.md -> markdown`).
2. Shebang line inspection when the file has no extension (e.g. `#!/usr/bin/env python3 -> python`).
3. `null` when neither source yields a mapping.

`FileEntry.language_confidence` MUST be set to `"extension"`, `"shebang"`, or `"unknown"` matching the resolution source.

#### Scenario: Extension resolves language

- **WHEN** a file is named `main.py` and contains Python source code
- **THEN** `FileEntry.language="python"` and `FileEntry.language_confidence="extension"`.

#### Scenario: Shebang resolves language for extensionless file

- **WHEN** a file is named `run` with no extension and starts with `#!/usr/bin/env bash`
- **THEN** `FileEntry.language="bash"` and `FileEntry.language_confidence="shebang"`.

#### Scenario: Unknown extension yields null language

- **WHEN** a file is named `notes.xyz` with an unrecognized extension and no shebang
- **THEN** `FileEntry.language=null` and `FileEntry.language_confidence="unknown"`.

---
### Requirement: Content type summary generation

The scanner SHALL populate `ScanResult.content_summary` as a `ContentTypeSummary` with the fields `total_files`, `kind_counts`, `language_counts`, `category_counts`, `dominant_category`, `dominant_languages`, `has_tests`, `has_docs`, and `is_monorepo`. The `category_counts` MUST classify each file into `code`, `docs`, `config`, `test`, or `other` per the rules defined in `docs/module-1-scanner.md` §十一. `dominant_category` SHALL be the single category exceeding 60% of included files, or `"mixed"` otherwise.

#### Scenario: Python-dominant repo reports code as dominant category

- **WHEN** a workspace contains 8 Python source files and 2 Markdown files
- **THEN** `content_summary.dominant_category="code"`, `content_summary.dominant_languages` starts with `"python"`, and `content_summary.category_counts["code"]=8`.

#### Scenario: Mixed repo reports mixed category

- **WHEN** a workspace contains 5 code files, 4 docs files, and 3 config files with no single category exceeding 60%
- **THEN** `content_summary.dominant_category="mixed"`.

#### Scenario: Tests directory detected

- **WHEN** a workspace contains files under `tests/` or named `*_test.py`
- **THEN** `content_summary.has_tests=true`.

---
### Requirement: Sandbox boundary enforcement

The scanner SHALL invoke `ensure_in_workspace(path, ctx)` from the `tool-sandbox` capability before reading or stat-ing any filesystem entry. Entries that fail this check MUST be skipped and logged in `ScanResult.warnings`; they MUST NOT appear in `ScanResult.files` or `ScanResult.symlinks`.

#### Scenario: Path escape attempt skipped and warned

- **WHEN** a resolved entry path would fall outside `workspace_root` (e.g. via a symlink or a `..`-bearing ignore negation)
- **THEN** the entry is omitted from `ScanResult.files` and `ScanResult.symlinks`, and a human-readable warning is appended to `ScanResult.warnings` identifying the offending path.

---
### Requirement: Deferred subsystem schema preservation

Skeleton scans that have not yet implemented the remaining deferred subsystems MUST emit the complete `ScanResult` schema with stable default values for those subsystems, so that downstream consumers can be written against the final contract:

- `ScanResult.git` MUST be `null` until git metadata collection lands.
- `ScanResult.is_monorepo` MUST be `false`, `ScanResult.monorepo_type` MUST be `null`, and `ScanResult.sub_packages` MUST be an empty list `[]` until monorepo detection lands.
- `FileEntry.oversized_preview` MUST be `null` until the oversized preview enhancement lands.

The `FileEntry.sanitize_stats` field is no longer a deferred stub; its semantics are governed by the "Pass 1 sanitizer orchestration for text FileEntries" requirement. The previous constraint that `/scan` output MUST NOT be consumed by the LLM call chain no longer applies, because Pass 1 sanitization is now enforced at the scanner boundary.

#### Scenario: Git metadata default null

- **WHEN** a scan completes on a workspace that contains a `.git/` directory while git metadata collection is still deferred
- **THEN** `ScanResult.git` equals `null` and no `pygit2` call is made.

#### Scenario: Monorepo fields default inactive

- **WHEN** a scan completes on a workspace that declares a `pnpm-workspace.yaml` while monorepo detection is still deferred
- **THEN** `ScanResult.is_monorepo=false`, `ScanResult.monorepo_type=null`, and `ScanResult.sub_packages=[]`.

#### Scenario: Oversized preview default null

- **WHEN** a scan encounters a file whose size exceeds `max_file_size_kb` while the oversized preview enhancement is still deferred
- **THEN** the corresponding `FileEntry.kind="oversized"`, `FileEntry.content=null`, and `FileEntry.oversized_preview=null`.


<!-- @trace
source: scanner-sanitizer-orchestration
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/scanner/service.py
  - docs/module-1-scanner.md
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sandbox.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/scanner/test_scan_api.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/scanner/fixtures/with-secrets/config.py
  - sidecar/tests/scanner/fixtures/with-secrets/contacts.txt
  - sidecar/tests/scanner/fixtures/with-secrets/README.md
  - sidecar/tests/scanner/test_fixtures_integration.py
-->

---
### Requirement: Synchronous response without SSE progress events

The skeleton `POST /scan` endpoint SHALL return the full `ScanResult` in a single HTTP response body with `Content-Type: application/json`. It MUST NOT open a server-sent-events stream or emit intermediate progress events; the progress-event contract defined in `docs/sidecar-api.md` §四 is reserved for a later change.

#### Scenario: Single JSON response body

- **WHEN** a client sends `POST /scan` and the scan succeeds
- **THEN** the response has `Content-Type: application/json`, a single JSON body, and no `Content-Type: text/event-stream` alternate.

---
### Requirement: Pass 1 sanitizer orchestration for text FileEntries

The scanner SHALL invoke `SanitizerEngine.sanitize(content, FileSource(path=<relative_path>))` for every candidate `FileEntry` whose `kind` resolves to `text` after successful decoding and before the entry is appended to `ScanResult.files`. `FileEntry.content` MUST store the Pass 1 sanitized string returned by the engine, not the raw decoded body. `FileEntry.sanitize_stats` MUST be a mapping from `AuditEntry.kind` (e.g. `"email"`, `"secret"`, `"domain"`) to the integer number of placeholders applied to that file, and MUST be `{}` when the engine reports zero hits.

The scanner MUST construct `FileSource` with `pass_="scanner"` and `path` equal to the file path relative to `workspace_root` (forward slashes, regardless of host OS). The scanner MUST pass the shared `SanitizerEngine` instance provided by `ToolContext.sanitizer`; it MUST NOT construct an ad-hoc engine per call.

#### Scenario: Plain text file with no sanitizer matches

- **WHEN** a workspace contains `notes.md` whose decoded body has no email, secret, or other sanitizer-rule matches
- **THEN** the resulting `FileEntry.content` equals the decoded body byte-for-byte, `FileEntry.sanitize_stats` equals `{}`, and no line is appended to `sanitize_audit.jsonl` for that file.

#### Scenario: Text file containing an email is scrubbed in content and counted in stats

- **WHEN** a workspace contains `contact.txt` whose decoded body contains one email address and the sanitizer email rule is active
- **THEN** `FileEntry.content` contains a `<REDACTED:email#N>` placeholder in place of the email, `FileEntry.sanitize_stats` equals `{"email": 1}`, and the raw email string does not appear anywhere in `ScanResult`.

#### Scenario: Non-text kinds bypass sanitizer

- **WHEN** a workspace contains `logo.png` (binary), `uv.lock` (lockfile), and `app.min.js` (generated)
- **THEN** no sanitize call is issued for these files, `FileEntry.content` stays `null`, and `FileEntry.sanitize_stats` equals `{}` for each.


<!-- @trace
source: scanner-sanitizer-orchestration
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/scanner/service.py
  - docs/module-1-scanner.md
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sandbox.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/scanner/test_scan_api.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/scanner/fixtures/with-secrets/config.py
  - sidecar/tests/scanner/fixtures/with-secrets/contacts.txt
  - sidecar/tests/scanner/fixtures/with-secrets/README.md
  - sidecar/tests/scanner/test_fixtures_integration.py
-->

---
### Requirement: Sanitize audit logging during scan

For every Pass 1 hit (every `AuditEntry` returned by `SanitizerEngine.sanitize`) produced during a single scan, the scanner SHALL persist one line to `sanitize_audit.jsonl` via the existing `SanitizeAuditLogger`. The persisted entry MUST carry `source.pass = "scanner"` and `source.path` equal to the file path relative to `workspace_root`. The scanner MUST NOT batch audit writes in a way that loses entries when a later file fails; each successful sanitize call's entries SHALL be flushed before the next file is processed.

If `SanitizerEngine.sanitize` raises any exception for a given file, that file is treated as quarantined: it MUST NOT appear in `ScanResult.files` under any form, a human-readable warning identifying the offending relative path MUST be appended to `ScanResult.warnings`, and `ScanResult.stats.quarantined_count` MUST be incremented by one. The engine-raised exception MUST NOT propagate out of `POST /scan`; the endpoint SHALL still return HTTP 200 with the remaining files.

#### Scenario: Sanitize audit line written for each hit

- **WHEN** a scan encounters two text files that together produce three sanitizer hits (e.g. two emails and one secret)
- **THEN** `sanitize_audit.jsonl` receives three new lines, each with `source.pass="scanner"` and `source.path` set to the relevant file's workspace-relative path.

#### Scenario: Sanitizer engine failure quarantines the file without failing the scan

- **WHEN** `SanitizerEngine.sanitize` raises an unexpected exception while processing `broken.txt`
- **THEN** `ScanResult.files` contains no entry for `broken.txt`, `ScanResult.warnings` includes a message identifying `broken.txt`, `ScanResult.stats.quarantined_count` is at least `1`, and the overall response is HTTP 200.


<!-- @trace
source: scanner-sanitizer-orchestration
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/scanner/service.py
  - docs/module-1-scanner.md
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sandbox.py
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/scanner/test_scan_api.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/scanner/fixtures/with-secrets/config.py
  - sidecar/tests/scanner/fixtures/with-secrets/contacts.txt
  - sidecar/tests/scanner/fixtures/with-secrets/README.md
  - sidecar/tests/scanner/test_fixtures_integration.py
-->

---
### Requirement: Scanner progress callback hook

The scanner service `scan(...)` function SHALL accept an optional `on_progress` keyword argument typed as `ScannerProgressCallback`, defined as `Callable[[ScannerProgressEvent], Awaitable[None]] | None`. When `on_progress is None` the scanner MUST behave identically to the existing synchronous contract and MUST NOT introduce any await points beyond those already present. When `on_progress` is provided the scanner MUST emit at least one progress event during the directory walk phase and at least one progress event during the sanitizer Pass 1 phase. Each emitted `ScannerProgressEvent` MUST carry the fields `phase: Literal["walking", "sanitizing"]`, `current: int` (non-negative count of files processed so far in the phase), `total: int | None` (total expected count when known, `None` while still discovering), and `current_file: str | None` (path of the most recently processed file when applicable). The scanner MUST NOT emit progress events with negative counts or with `current > total` when `total` is not `None`.

#### Scenario: Synchronous call without callback preserves existing contract

- **WHEN** `scan(...)` is invoked without `on_progress`
- **THEN** the call MUST return a `ScanResult` synchronously and MUST NOT raise due to a missing callback

#### Scenario: Callback receives at least one event per phase

- **WHEN** `scan(...)` is invoked with an `on_progress` callback against a workspace containing at least three files
- **THEN** the callback MUST be awaited at least once with `phase == "walking"` and at least once with `phase == "sanitizing"` before `scan` returns

#### Scenario: Callback exception does not corrupt scan result

- **WHEN** the supplied `on_progress` callback raises during one of its invocations
- **THEN** the scanner MUST surface the exception to the caller without silently swallowing it, and the partially-built scan state MUST NOT leak into a returned `ScanResult`


<!-- @trace
source: sse-progress-skeleton
updated: 2026-04-22
code:
  - sidecar/src/codebus_agent/scanner/models.py
  - CLAUDE.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/scanner/service.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/api/scan.py
  - docs/module-1-scanner.md
  - docs/module-2-kb-builder.md
  - docs/sidecar-api.md
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/__init__.py
  - sidecar/tests/scanner/test_fixtures_integration.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_task_error_containment.py
  - sidecar/tests/api/test_task_registry.py
  - sidecar/tests/api/test_task_result.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/api/test_tasks_sse.py
  - sidecar/tests/scanner/test_progress_callback.py
-->

---
### Requirement: POST /scan opt-in async streaming mode

The `POST /scan` endpoint SHALL preserve its existing synchronous contract when invoked without query parameters: it MUST return the full `ScanResult` JSON in a single response. When the request URL includes the query parameter `stream=true`, the endpoint SHALL instead create a `kind="scan"` task in the sidecar task registry, spawn a background coroutine that invokes `scan(..., on_progress=handle.emit)`, return HTTP `200` with body `{"task_id": "scan_<hex8>"}` immediately, and SHALL NOT block until the scan completes. The `?stream=true` path MUST translate every `ScannerProgressEvent` it receives from the callback into a wire event matching `docs/sidecar-api.md §四` `progress` schema with `phase: "scanning"` (collapsing the scanner's internal `walking`/`sanitizing` distinction). When the background scan completes successfully the task handle's `result` MUST be set to the full `ScanResult` JSON and a `done` event MUST be emitted; when it raises, the error containment path defined by `sidecar-runtime` MUST apply.

#### Scenario: Sync mode unchanged when stream query absent

- **WHEN** a client calls `POST /scan` without query parameters and a valid bearer token, body `{"workspace_type": "folder", "workspace_root": "<path>"}`
- **THEN** the response MUST be HTTP `200` containing the full `ScanResult` JSON, with no `task_id` field present

#### Scenario: Stream mode returns task_id and starts background work

- **WHEN** a client calls `POST /scan?stream=true` against the same workspace
- **THEN** the response MUST return immediately with body `{"task_id": "scan_<hex8>"}` and a subsequent `GET /tasks/<task_id>/events` subscription MUST eventually receive at least one `progress` event with `phase: "scanning"` followed by a `done` event

#### Scenario: Stream done event triggers result endpoint readiness

- **WHEN** a client subscribes to a stream-mode scan task and the stream emits `done`
- **THEN** an immediately following `GET /tasks/<task_id>/result` MUST return HTTP `200` with the full `ScanResult` JSON

<!-- @trace
source: sse-progress-skeleton
updated: 2026-04-22
code:
  - sidecar/src/codebus_agent/scanner/models.py
  - CLAUDE.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/scanner/service.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/api/scan.py
  - docs/module-1-scanner.md
  - docs/module-2-kb-builder.md
  - docs/sidecar-api.md
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/__init__.py
  - sidecar/tests/scanner/test_fixtures_integration.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_task_error_containment.py
  - sidecar/tests/api/test_task_registry.py
  - sidecar/tests/api/test_task_result.py
  - sidecar/tests/scanner/test_service.py
  - sidecar/tests/api/test_tasks_sse.py
  - sidecar/tests/scanner/test_progress_callback.py
-->
