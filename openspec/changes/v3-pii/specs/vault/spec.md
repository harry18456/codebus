## RENAMED Requirements

- FROM: `### Requirement: Raw Mirror with NullScanner`
- TO: `### Requirement: Raw Mirror with PII Scanner`

## MODIFIED Requirements

### Requirement: Raw Mirror with NullScanner

The system SHALL mirror source files from the repository root into `.codebus/raw/code/` preserving directory structure. The system SHALL skip top-level entries `.codebus/`, `.git/`, and `.env`. The system SHALL honor `<repo>/.gitignore` patterns. The system SHALL skip files larger than 5 mebibytes (5 × 1024 × 1024 bytes). The system SHALL invoke a configured `PiiScanner` against the contents of every mirrored regular file. When the scanner reports any match for a file under the default on-hit policy `Warn`, the system SHALL emit one stderr line per match in the format `pii warn: <pattern_name> at <relative_path>:<byte_offset>` and SHALL include that file in the mirror with content unchanged. The system SHALL NOT include the matched substring text in the warning line. The system SHALL aggregate the total count of PII matches observed across all mirrored files and SHALL expose that count through the raw mirror summary value returned to callers.

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

#### Scenario: stderr warning omits matched text

- **WHEN** a mirrored source file contains the substring `alice@example.com`
- **THEN** the corresponding stderr `pii warn:` line SHALL NOT contain the substring `alice@example.com`

#### Scenario: Multiple matches in one file produce multiple warning lines

- **WHEN** a single mirrored source file contains both `alice@example.com` and `192.168.1.42`
- **THEN** stderr SHALL contain two distinct `pii warn:` lines whose `<pattern_name>` segments are `email` and `ipv4` respectively

#### Scenario: Raw mirror summary aggregates total PII match count

- **WHEN** the raw mirror processes a repository where exactly N PII matches are reported across all mirrored files combined
- **THEN** the summary value returned to callers SHALL expose a PII match count field equal to N
