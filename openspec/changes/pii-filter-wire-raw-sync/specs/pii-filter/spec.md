## ADDED Requirements

### Requirement: Invoke PiiScanner on each candidate text file before mirroring

When `raw_sync` would mirror a candidate file (one that has already passed the always-skip, gitignore, and size-limit filters), the system SHALL first attempt to read the file as UTF-8 text and, if successful, SHALL invoke `PiiScanner::scan(content, rel_path)` against the configured scanner. If the file cannot be decoded as UTF-8, the system SHALL fall through and copy the original bytes without scanning.

#### Scenario: UTF-8 text file is scanned before mirror

- **WHEN** raw_sync encounters a UTF-8 text file `src/secrets.py` containing `KEY=AKIAIOSFODNN7EXAMPLE`
- **AND** the configured scanner is `RegexBasicScanner` with default builtin patterns
- **THEN** the scanner is invoked with the file's contents and the relative path `src/secrets.py`
- **AND** the scan returns at least one match labeled `aws-access-key`

#### Scenario: Non-UTF-8 binary file is mirrored without scanning

- **WHEN** raw_sync encounters a file `assets/logo.png` whose bytes are not valid UTF-8
- **THEN** the scanner is NOT invoked
- **AND** the file is copied byte-for-byte to the destination

#### Scenario: Empty file is mirrored without producing matches

- **WHEN** raw_sync encounters an empty UTF-8 file
- **THEN** the scanner is invoked with empty content and returns zero matches
- **AND** the file is mirrored as an empty file

### Requirement: OnHit::Warn writes a stderr line per match and still mirrors the file

When the scanner returns one or more matches and the configured `on_hit` mode is `Warn`, the system SHALL emit one stderr line per match in the form `warning: PII match in <rel_path>: <pattern_name> at offset <byte_start>`, and SHALL mirror the file's original contents to the destination unchanged.

#### Scenario: Single match warns and mirrors

- **WHEN** scan returns one match `(pattern_name=anthropic-api-key, start=42)` for file `src/llm.py`
- **AND** `on_hit` is `Warn`
- **THEN** stderr contains exactly one line `warning: PII match in src/llm.py: anthropic-api-key at offset 42`
- **AND** the destination file `<raw_dir>/src/llm.py` exists with byte-for-byte the original content

#### Scenario: Multiple matches in one file produce one stderr line per match

- **WHEN** scan returns three matches for `docs/contact.md` (two `email`, one `ipv4`)
- **AND** `on_hit` is `Warn`
- **THEN** stderr contains three lines, one per match, in ascending offset order

### Requirement: OnHit::Skip omits the file from the mirror and writes a stderr line

When the scanner returns one or more matches and the configured `on_hit` mode is `Skip`, the system SHALL NOT write the file to the destination, and SHALL emit one stderr line in the form `skipped: <rel_path> (reason: pii hit <pattern_name>)` naming the first match's pattern.

#### Scenario: Match causes file to be skipped

- **WHEN** scan returns at least one match for `secrets.env`
- **AND** `on_hit` is `Skip`
- **THEN** the destination `<raw_dir>/secrets.env` does NOT exist
- **AND** stderr contains a line `skipped: secrets.env (reason: pii hit <pattern_name>)`

#### Scenario: Skipped file does not block sibling files

- **WHEN** raw_sync encounters two files: `clean.txt` (zero matches) and `dirty.txt` (one match)
- **AND** `on_hit` is `Skip`
- **THEN** `<raw_dir>/clean.txt` exists with original content
- **AND** `<raw_dir>/dirty.txt` does NOT exist

### Requirement: OnHit::Mask replaces matched substrings with a labeled placeholder

When the scanner returns one or more matches and the configured `on_hit` mode is `Mask`, the system SHALL replace each matched byte range in the file content with the literal string `[REDACTED:<pattern_name>]`, and SHALL write the resulting content to the destination. Replacement SHALL proceed from the highest offset to the lowest so earlier offsets are not invalidated. When two matches overlap, only the later (higher-offset) match SHALL be replaced; the earlier match SHALL be skipped.

#### Scenario: Single match is replaced in place

- **WHEN** scan returns one match `(pattern_name=email, start=10, end=27)` against content `contact: alice@example.com\n`
- **AND** `on_hit` is `Mask`
- **THEN** the destination file content equals `contact: [REDACTED:email]\n`

#### Scenario: Multiple non-overlapping matches all replaced

- **WHEN** scan returns matches for both `email` and `ipv4` in the same file at distinct offsets
- **AND** `on_hit` is `Mask`
- **THEN** the destination contains both `[REDACTED:email]` and `[REDACTED:ipv4]` placeholders, with all other bytes preserved

#### Scenario: Mask preserves line count

- **WHEN** scan returns matches whose substrings do NOT contain `\n`
- **AND** `on_hit` is `Mask`
- **THEN** the destination file has the same number of lines as the source file

#### Scenario: Mask mode emits no stderr output for matches

- **WHEN** scan returns one or more matches and `on_hit` is `Mask`
- **THEN** stderr contains no `warning:` or `skipped:` lines for those matches

### Requirement: Default scanner configuration preserves 0.2.0 behavior

The system SHALL treat an absent `~/.codebus/config.yaml`, an absent `pii` section, or `pii.scanner: null` as a request for the no-op `NullScanner` so that raw mirror output is byte-for-byte equivalent to a build that has no PII filter wired in.

#### Scenario: Missing config produces a byte-equal mirror

- **WHEN** no `~/.codebus/config.yaml` is present
- **AND** raw_sync runs against a source repo containing files with PII-like content
- **THEN** every text file is mirrored byte-for-byte to the destination
- **AND** stderr contains no `warning:` or `skipped:` lines

#### Scenario: pii.scanner null is identical to absent section

- **WHEN** `~/.codebus/config.yaml` contains `pii: { scanner: "null" }`
- **THEN** raw_sync output equals the output from an absent `pii` section under the same source repo

### Requirement: User-supplied patterns_extra entries trigger matches alongside builtin patterns

When `~/.codebus/config.yaml` `pii.patterns_extra` contains user-supplied regex strings, the system SHALL register each entry as an additional rule labeled `custom-<index>` and SHALL emit matches for those patterns according to the configured `on_hit` mode in the same manner as builtin matches.

#### Scenario: Custom regex hits trigger configured on_hit

- **WHEN** `pii.patterns_extra` contains `INTERNAL-\d{6}`
- **AND** `pii.on_hit` is `Warn`
- **AND** raw_sync mirrors a file containing `ticket INTERNAL-123456 closed`
- **THEN** stderr contains a line referencing `custom-0` for that file
