## ADDED Requirements

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

### Requirement: File classification by extension and content sniffing

The scanner SHALL classify each non-symlink entry into exactly one of `{text, binary, oversized, lockfile, generated}` and populate `FileEntry.kind` accordingly. Classification MUST apply the following rules in order; the first matching rule wins:

1. `generated` if the filename matches `*.min.js`, `*.min.css`, or `*.bundle.js`.
2. `lockfile` if the filename matches `*-lock.json`, `yarn.lock`, `poetry.lock`, `Cargo.lock`, `uv.lock`, or `Gemfile.lock`.
3. `binary` if the extension is in a declared binary-extension set (including but not limited to `.png`, `.jpg`, `.pdf`, `.zip`, `.exe`, `.dll`, `.so`, `.dylib`, `.woff`, `.woff2`, `.ttf`).
4. `binary` if the first 8 KB of file content contains a null byte (`\x00`) or decoding against the entire fallback chain fails.
5. `oversized` if the file size exceeds `max_file_size_kb` (default 512 KB) and the file was not classified as binary by rules 1-4.
6. `text` otherwise.

For `binary`, `lockfile`, and `generated` entries, `FileEntry.content` MUST be `null` and `FileEntry.encoding` MUST be `null`. For `oversized` entries, `FileEntry.content` MUST be `null` and `FileEntry.oversized_preview` MAY contain up to the first 200 lines of the decoded head.

#### Scenario: PNG classified as binary without reading full content

- **WHEN** the workspace contains `logo.png`
- **THEN** the corresponding `FileEntry` has `kind="binary"`, `content=null`, and the scanner does not read the full file bytes into memory.

#### Scenario: uv.lock classified as lockfile and content omitted

- **WHEN** the workspace contains `uv.lock`
- **THEN** the corresponding `FileEntry` has `kind="lockfile"` and `content=null`; the file size is still recorded in `FileEntry.size`.

#### Scenario: Minified JS classified as generated

- **WHEN** the workspace contains `app.min.js`
- **THEN** the corresponding `FileEntry` has `kind="generated"` and `content=null`.

#### Scenario: Small plain text file classified as text with content

- **WHEN** the workspace contains `README.md` of size 2 KB containing UTF-8 text
- **THEN** the corresponding `FileEntry` has `kind="text"`, `content` equals the decoded file body, and `encoding="utf-8"`.

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

### Requirement: Sandbox boundary enforcement

The scanner SHALL invoke `ensure_in_workspace(path, ctx)` from the `tool-sandbox` capability before reading or stat-ing any filesystem entry. Entries that fail this check MUST be skipped and logged in `ScanResult.warnings`; they MUST NOT appear in `ScanResult.files` or `ScanResult.symlinks`.

#### Scenario: Path escape attempt skipped and warned

- **WHEN** a resolved entry path would fall outside `workspace_root` (e.g. via a symlink or a `..`-bearing ignore negation)
- **THEN** the entry is omitted from `ScanResult.files` and `ScanResult.symlinks`, and a human-readable warning is appended to `ScanResult.warnings` identifying the offending path.

### Requirement: Deferred subsystem schema preservation

Skeleton scans MUST emit the complete `ScanResult` schema with stable default values for subsystems not yet implemented, so that downstream consumers can be written against the final contract:

- `FileEntry.sanitize_stats` MUST be an empty dict `{}`.
- `ScanResult.git` MUST be `null`.
- `ScanResult.is_monorepo` MUST be `false`, `ScanResult.monorepo_type` MUST be `null`, and `ScanResult.sub_packages` MUST be an empty list `[]`.

The skeleton `/scan` output MUST NOT be consumed by any code path that forwards data into the LLM call chain until the sanitizer orchestration change lands; this constraint applies at the capability boundary, not via runtime enforcement.

#### Scenario: Sanitize stats default empty

- **WHEN** a scan completes on any folder workspace during the skeleton change
- **THEN** every `FileEntry.sanitize_stats` equals `{}`.

#### Scenario: Git metadata default null

- **WHEN** a scan completes on a workspace that contains a `.git/` directory during the skeleton change
- **THEN** `ScanResult.git` equals `null` and no `pygit2` call is made.

#### Scenario: Monorepo fields default inactive

- **WHEN** a scan completes on a workspace that declares a `pnpm-workspace.yaml` during the skeleton change
- **THEN** `ScanResult.is_monorepo=false`, `ScanResult.monorepo_type=null`, and `ScanResult.sub_packages=[]`.

### Requirement: Synchronous response without SSE progress events

The skeleton `POST /scan` endpoint SHALL return the full `ScanResult` in a single HTTP response body with `Content-Type: application/json`. It MUST NOT open a server-sent-events stream or emit intermediate progress events; the progress-event contract defined in `docs/sidecar-api.md` §四 is reserved for a later change.

#### Scenario: Single JSON response body

- **WHEN** a client sends `POST /scan` and the scan succeeds
- **THEN** the response has `Content-Type: application/json`, a single JSON body, and no `Content-Type: text/event-stream` alternate.
