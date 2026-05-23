## MODIFIED Requirements

### Requirement: Raw Mirror with PII Scanner

The system SHALL mirror source files from the repository root into `.codebus/raw/code/` preserving directory structure. The system SHALL skip the top-level entries `.codebus/` and `.env` AND SHALL skip any directory whose path segment equals `.git` at any depth (so a nested `.git/` introduced by a git submodule or an embedded repository is excluded just like the repository's own root `.git/`). The system SHALL honor `<repo>/.gitignore` patterns. The system SHALL skip files larger than 5 mebibytes (5 × 1024 × 1024 bytes); for every such oversized skip the system SHALL increment the sync summary's `oversized_skipped_files` counter by exactly one AND SHALL ATTEMPT to emit one warn line in the format `mirror skip: oversized at <relative_path> (<N> bytes > 5 MiB limit)` (the relative path SHALL be normalised to forward-slash separators so output is consistent across Windows AND Unix). Warn-line emission is best-effort observability: when the warn sink returns an I/O error (for example `BrokenPipe` / Windows `ERROR_NO_DATA` when stderr is a closed pipe under a GUI host such as Tauri), the system SHALL silently swallow the write error AND SHALL still perform the skip AND SHALL still increment the counter — a failing warn sink SHALL NOT abort the surrounding `sync_with_scanner` invocation. The counter is the load-bearing observable surface; the warn line is the convenience surface. The oversized skip warning SHALL surface only from the mirror-writer path (the user-facing sync); the drift-detection signal walk (consumed by `goal` to compute `source_signal`) SHALL continue to skip oversized files silently because that path performs no I/O on the warn sink AND its caller does not surface per-file warnings. The system SHALL invoke a configured `PiiScanner` against the contents of every mirrored regular file. When the scanner reports any match for a file under the default on-hit policy `Warn`, the system SHALL emit one stderr line per match in the format `pii warn: <pattern_name> at <relative_path>:<byte_offset>` AND SHALL include that file in the mirror with content unchanged. The system SHALL NOT include the matched substring text in the warning line. The system SHALL aggregate the total count of PII matches observed across all mirrored files AND SHALL expose that count through the raw mirror summary value returned to callers.

#### Scenario: Mirror preserves directory structure and skips top-level dot directories

- **WHEN** a repository contains `src/main.rs`, `nested/lib.rs`, `.git/config`, `.env`, and `.codebus/manifest.yaml`
- **THEN** the system SHALL mirror `src/main.rs` AND `nested/lib.rs` into `.codebus/raw/code/src/main.rs` AND `.codebus/raw/code/nested/lib.rs` respectively AND SHALL NOT mirror `.git/config`, `.env`, or `.codebus/manifest.yaml`

#### Scenario: Mirror honors source repo .gitignore

- **WHEN** the source repository's `.gitignore` contains `target/` AND a file `target/debug/foo.rs` exists
- **THEN** the system SHALL NOT mirror `target/debug/foo.rs` into the vault

#### Scenario: Mirror skips files exceeding the size limit and emits a stderr warning

- **WHEN** a source file `big.bin` is larger than 5 × 1024 × 1024 bytes
- **THEN** the system SHALL skip that file (no mirror entry created) AND the stderr warn sink SHALL contain exactly one line beginning with `mirror skip: oversized at big.bin (` followed by the file size in bytes AND containing the substring `> 5 MiB limit` AND the sync summary's `oversized_skipped_files` field SHALL equal one AND the system SHALL continue processing remaining files

#### Scenario: Oversized counter aggregates across multiple skipped files

- **WHEN** a single sync invocation encounters two source files `a.bin` AND `b.bin` both larger than 5 × 1024 × 1024 bytes alongside one small file `small.txt`
- **THEN** the sync summary's `oversized_skipped_files` field SHALL equal two AND the warn sink SHALL contain exactly two `mirror skip: oversized at ...` lines (one per oversized file) AND `small.txt` SHALL be mirrored

#### Scenario: PII match emits stderr warning and still mirrors file

- **WHEN** a source file at `src/aws.py` contains the substring `AKIAIOSFODNN7EXAMPLE` (an AWS access key shape) AND is mirrored
- **THEN** stderr SHALL contain a line beginning with `pii warn: aws-access-key at src/aws.py:` AND the file `.codebus/raw/code/src/aws.py` SHALL exist with the original content unchanged

#### Scenario: Mirror skips nested .git directories at any depth

- **WHEN** a repository contains `vendor/foo/.git/config` (a git submodule's directory, or any embedded repository's git data) AND `vendor/foo/src/main.rs`
- **THEN** the system SHALL NOT mirror any file under `vendor/foo/.git/` into `.codebus/raw/code/` AND SHALL mirror `vendor/foo/src/main.rs` into `.codebus/raw/code/vendor/foo/src/main.rs`

#### Scenario: Nested .codebus directories at deeper depths are user content and are mirrored

- **WHEN** a repository contains `docs/.codebus/notes.md` (a user-authored sub-directory that happens to share the name)
- **THEN** the system SHALL mirror `docs/.codebus/notes.md` into `.codebus/raw/code/docs/.codebus/notes.md`

#### Scenario: Source signal walk excludes nested .git identically to mirror

- **WHEN** the source signal walk (consumed by `goal` drift detection) traverses a repository that contains `vendor/foo/.git/config`
- **THEN** the walk SHALL NOT include `vendor/foo/.git/config` (or any file beneath `vendor/foo/.git/`) in its file count or aggregate byte total, matching the exclusion the mirror writer applies

#### Scenario: Source signal walk silently skips oversized files without warn

- **WHEN** the source signal walk (consumed by `goal` drift detection) traverses a repository that contains a 6 MiB file
- **THEN** the walk SHALL exclude that file from its file count AND aggregate byte total (matching the mirror writer's skip rule so `source_signal` stays consistent) AND SHALL NOT emit any stderr warning for the oversized file (the walk is an internal drift-detection helper with no warn sink AND no per-file user-facing surface)

#### Scenario: Sync survives a failing warn sink on oversized skip

- **WHEN** the raw-mirror sync runs against a source repository that contains an oversized file AND the warn sink returned to `sync_with_scanner_into` returns `Err(io::ErrorKind::BrokenPipe)` on every write attempt (modelling a Tauri-host stderr that has been closed)
- **THEN** `sync_with_scanner_into` SHALL return `Ok(SyncSummary)` (NOT `Err`) AND the returned summary's `oversized_skipped_files` field SHALL equal one AND the oversized file SHALL NOT appear in the raw mirror destination AND other unaffected files SHALL still be mirrored normally (the warn-write failure SHALL NOT cascade into a sync abort)
