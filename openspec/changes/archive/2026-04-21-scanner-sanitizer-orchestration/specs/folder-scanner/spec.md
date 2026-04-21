## ADDED Requirements

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

---

## MODIFIED Requirements

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
