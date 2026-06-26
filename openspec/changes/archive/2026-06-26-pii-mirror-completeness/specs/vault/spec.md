## MODIFIED Requirements

### Requirement: Raw Mirror with PII Scanner

The system SHALL mirror source files from the repository root into `.codebus/raw/code/` preserving directory structure. The system SHALL skip the top-level entries `.codebus/` and `.env` AND SHALL skip any directory whose path segment equals `.git` at any depth (so a nested `.git/` introduced by a git submodule or an embedded repository is excluded just like the repository's own root `.git/`). The system SHALL honor `<repo>/.gitignore` patterns. The system SHALL skip files larger than 5 mebibytes (5 × 1024 × 1024 bytes); for every such oversized skip the system SHALL increment the sync summary's `oversized_skipped_files` counter by exactly one AND SHALL ATTEMPT to emit one warn line in the format `mirror skip: oversized at <relative_path> (<N> bytes > 5 MiB limit)` (the relative path SHALL be normalised to forward-slash separators so output is consistent across Windows AND Unix). Warn-line emission is best-effort observability: when the warn sink returns an I/O error (for example `BrokenPipe` / Windows `ERROR_NO_DATA` when stderr is a closed pipe under a GUI host such as Tauri), the system SHALL silently swallow the write error AND SHALL still perform the skip AND SHALL still increment the counter — a failing warn sink SHALL NOT abort the surrounding `sync_with_scanner` invocation. The counter is the load-bearing observable surface; the warn line is the convenience surface. The oversized skip warning SHALL surface only from the mirror-writer path (the user-facing sync); the drift-detection signal walk (consumed by `goal` to compute `source_signal`) SHALL continue to skip oversized files silently because that path performs no I/O on the warn sink AND its caller does not surface per-file warnings. In addition to the per-skip counter and warn line, the mirror-writer path SHALL surface oversized skips to the agent that reads the raw mirror: after all entries have been processed, when one or more files were skipped as oversized during this sync invocation, the system SHALL write an aggregated oversized-files manifest into the mirror destination at `<raw_code_dir>/_codebus-oversized.md` (i.e. `.codebus/raw/code/_codebus-oversized.md`); when zero files were skipped as oversized during this sync invocation, the system SHALL NOT create or leave such a manifest. Because the mirror destination is fully recreated at the start of every sync, a manifest written by a previous sync SHALL NOT persist into a later sync that has no oversized files. The manifest SHALL begin with a header indicating that the listed files have content omitted because they exceed the 5 MiB limit AND are listed for structural awareness, followed by one entry line per skipped file; each entry line SHALL contain the skipped file's repository-relative path normalised to forward-slash separators AND its size in bytes. Entry lines SHALL be ordered by path so the manifest is deterministic across platforms. The manifest is an additional agent-facing structural-awareness surface; writing it SHALL NOT alter, replace, or suppress the per-skip warn line or the `oversized_skipped_files` counter, AND the manifest SHALL NOT contain any byte of the skipped files' content. The system SHALL invoke a configured `PiiScanner` against the contents of every mirrored regular file. When a file's bytes are valid UTF-8 the system SHALL scan the UTF-8 content directly. When a file's bytes are not valid UTF-8 but carry a UTF-16 LE, UTF-16 BE, or UTF-8 byte-order mark, the system SHALL decode the content to UTF-8 before scanning so that secrets in BOM-marked UTF-16 text files are detected (a clean decoded file is still mirrored byte-identically from its original bytes; a decoded file containing a Critical match is mirrored as masked UTF-8). When a file's bytes are not valid UTF-8 and carry none of these byte-order marks (a true binary file), the system SHALL mirror it byte-identically without scanning AND SHALL increment the sync summary's `unscanned_files` counter by exactly one so that the count of files that could not be scanned is observable; the system SHALL NOT write a per-file manifest for unscanned binary files (to avoid drowning the structural surface in image / asset noise). When the scanner reports any match for a file under the default on-hit policy `Warn`, the system SHALL emit one stderr line per match in the format `pii warn: <pattern_name> at <relative_path>:<byte_offset>` AND SHALL include that file in the mirror with content unchanged. The system SHALL NOT include the matched substring text in the warning line. The system SHALL aggregate the total count of PII matches observed across all mirrored files AND SHALL expose that count through the raw mirror summary value returned to callers.

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

#### Scenario: Oversized skip writes an agent-visible manifest listing path and size

- **WHEN** a single sync invocation skips one oversized source file `dist/bundle.js` of `N` bytes (`N` > 5 × 1024 × 1024) alongside one small file `small.txt`
- **THEN** the file `.codebus/raw/code/_codebus-oversized.md` SHALL exist AND SHALL contain an entry line referencing the forward-slash path `dist/bundle.js` together with the byte count `N` AND SHALL contain a header indicating content is omitted because the file exceeds the 5 MiB limit AND SHALL NOT contain any content from `dist/bundle.js` AND `small.txt` SHALL be mirrored AND the sync summary's `oversized_skipped_files` field SHALL equal one

##### Example: two oversized files listed in path order

- **GIVEN** a sync skips `vendor/big.tar` (7340032 bytes) AND `assets/dataset.csv` (6291456 bytes)
- **WHEN** the manifest `.codebus/raw/code/_codebus-oversized.md` is written
- **THEN** it lists `assets/dataset.csv` before `vendor/big.tar` (entries ordered by path) AND each entry pairs the forward-slash path with its byte count

#### Scenario: No oversized files leaves no manifest

- **WHEN** a sync invocation encounters no source file larger than 5 × 1024 × 1024 bytes
- **THEN** the file `.codebus/raw/code/_codebus-oversized.md` SHALL NOT exist after the sync completes

#### Scenario: A later oversized-free sync does not leave a stale manifest

- **WHEN** a first sync skips one oversized file (writing `.codebus/raw/code/_codebus-oversized.md`) AND a subsequent sync into the same mirror destination encounters no oversized file
- **THEN** after the subsequent sync the file `.codebus/raw/code/_codebus-oversized.md` SHALL NOT exist (the stale manifest from the first sync SHALL NOT persist)

#### Scenario: PII match emits stderr warning and still mirrors file

- **WHEN** a source file at `src/aws.py` contains the substring `AKIAIOSFODNN7EXAMPLE` (an AWS access key shape) AND is mirrored
- **THEN** stderr SHALL contain a line beginning with `pii warn: aws-access-key at src/aws.py:` AND the file `.codebus/raw/code/src/aws.py` SHALL exist with the original content unchanged

#### Scenario: BOM-marked UTF-16 text file is decoded and scanned

- **WHEN** a source file `secrets.txt` whose bytes are a UTF-16 LE byte-order mark followed by UTF-16 LE encoded text containing an AWS access key shape is mirrored under the default on-hit policy
- **THEN** the mirrored file `.codebus/raw/code/secrets.txt` SHALL contain `[REDACTED:aws-access-key]` substituted for the AWS key (the UTF-16 content having been decoded to UTF-8 and the Critical match masked per the security floor) AND the original AWS key characters SHALL NOT appear in the mirrored file

#### Scenario: True binary file is mirrored verbatim and counted as unscanned

- **WHEN** a source file `logo.png` whose bytes are not valid UTF-8 and carry no UTF-16 or UTF-8 byte-order mark is mirrored
- **THEN** the mirrored file `.codebus/raw/code/logo.png` SHALL be a byte-identical copy of the source AND the sync summary's `unscanned_files` field SHALL be incremented by exactly one AND no `pii warn:` line SHALL be emitted for that file AND no per-file unscanned manifest SHALL be written

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
- **THEN** the walk SHALL exclude that file from its file count AND aggregate byte total (matching the mirror writer's skip rule so `source_signal` stays consistent) AND SHALL NOT emit any stderr warning for the oversized file (the walk is an internal drift-detection helper with no warn sink AND no per-file user-facing surface) AND SHALL NOT write an oversized-files manifest (the manifest is produced only by the mirror-writer path)

#### Scenario: Sync survives a failing warn sink on oversized skip

- **WHEN** the raw-mirror sync runs against a source repository that contains an oversized file AND the warn sink returned to `sync_with_scanner_into` returns `Err(io::ErrorKind::BrokenPipe)` on every write attempt (modelling a Tauri-host stderr that has been closed)
- **THEN** `sync_with_scanner_into` SHALL return `Ok(SyncSummary)` (NOT `Err`) AND the returned summary's `oversized_skipped_files` field SHALL equal one AND the oversized file SHALL NOT appear in the raw mirror destination AND other unaffected files SHALL still be mirrored normally (the warn-write failure SHALL NOT cascade into a sync abort)
