## ADDED Requirements

### Requirement: Warn Sink Location Format

When the raw-mirror sync emits a warn line for a PII match, the rendered location SHALL be a 1-based `line:col` position derived from the match start byte offset, formatted as `pii warn: <pattern_name> at <relative_path>:<line>:<col>`. The system SHALL NOT render the raw byte offset in the warn line. The line number SHALL be the 1-based count of `\n`-terminated lines preceding the match start plus one, and the column SHALL be the 1-based count of Unicode scalar values between the start of that line and the match start plus one. The match's `start`/`end` byte offsets are retained internally for masking and SHALL NOT change.

#### Scenario: Match on the first line renders 1-based line and column

- **WHEN** the raw-mirror sync emits a warn line for a match whose start byte offset falls on the first line of the source content (no preceding `\n`)
- **THEN** the warn line SHALL have the form `pii warn: <pattern_name> at <relative_path>:1:<col>` where `<col>` is the 1-based Unicode-scalar column of the match start

##### Example: email at column 9 on line 1

- **GIVEN** source file `docs.md` with content `contact alice@example.com\n`
- **WHEN** the email match (start at byte offset 8) is emitted under policy `Warn`
- **THEN** the warn line SHALL equal `pii warn: email at docs.md:1:9`

#### Scenario: Match on a later line renders the correct 1-based line number

- **WHEN** the raw-mirror sync emits a warn line for a match preceded by N newline characters in the source content
- **THEN** the rendered line number SHALL equal N + 1 AND the rendered column SHALL be relative to the start of that line, not the start of the file

##### Example: email on line 3

- **GIVEN** source file `logs.txt` with content `a\nb\ncontact alice@example.com\n` (two newlines before the match line)
- **WHEN** the email match is emitted under policy `Warn`
- **THEN** the warn line SHALL equal `pii warn: email at logs.txt:3:9`

#### Scenario: No raw byte offset appears in the warn line

- **WHEN** any warn line is emitted for a source file whose match start byte offset is greater than its total line count
- **THEN** the integer following the final colon SHALL be a valid 1-based column within its line AND the warn line SHALL NOT contain the raw byte offset value
