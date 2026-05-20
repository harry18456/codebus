## MODIFIED Requirements

### Requirement: Raw Mirror with PII Scanner

The system SHALL mirror source files from the repository root into `.codebus/raw/code/` preserving directory structure. The system SHALL skip the top-level entries `.codebus/` and `.env` and SHALL skip any directory whose path segment equals `.git` at any depth (so a nested `.git/` introduced by a git submodule or an embedded repository is excluded just like the repository's own root `.git/`). The system SHALL honor `<repo>/.gitignore` patterns. The system SHALL skip files larger than 5 mebibytes (5 × 1024 × 1024 bytes). The system SHALL invoke a configured `PiiScanner` against the contents of every mirrored regular file. When the scanner reports any match for a file under the default on-hit policy `Warn`, the system SHALL emit one stderr line per match in the format `pii warn: <pattern_name> at <relative_path>:<byte_offset>` and SHALL include that file in the mirror with content unchanged. The system SHALL NOT include the matched substring text in the warning line. The system SHALL aggregate the total count of PII matches observed across all mirrored files and SHALL expose that count through the raw mirror summary value returned to callers.

#### Scenario: Mirror preserves directory structure and skips top-level dot directories

- **WHEN** a repository contains `src/main.rs`, `nested/lib.rs`, `.git/config`, `.env`, and `.codebus/manifest.yaml`
- **THEN** the system SHALL mirror `src/main.rs` and `nested/lib.rs` into `.codebus/raw/code/src/main.rs` and `.codebus/raw/code/nested/lib.rs` respectively AND SHALL NOT mirror `.git/config`, `.env`, or `.codebus/manifest.yaml`

#### Scenario: Mirror honors source repo .gitignore

- **WHEN** the source repository's `.gitignore` contains `target/` AND a file `target/debug/foo.rs` exists
- **THEN** the system SHALL NOT mirror `target/debug/foo.rs` into the vault

#### Scenario: Mirror skips files exceeding the size limit

- **WHEN** a source file is larger than 5 × 1024 × 1024 bytes
- **THEN** the system SHALL skip that file (no mirror entry created) and continue processing remaining files

#### Scenario: PII match emits stderr warning and still mirrors file

- **WHEN** a source file at `src/aws.py` contains the substring `AKIAIOSFODNN7EXAMPLE` (an AWS access key shape) and is mirrored
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
