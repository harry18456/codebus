## ADDED Requirements

### Requirement: Empty and Zero-Width Extra Pattern Safety

The regex-based scanner SHALL be immune to empty and zero-width user-supplied extra patterns, which would otherwise produce a match at every character position and make scanning a large file pathologically slow.

At construction, the scanner SHALL skip any `patterns_extra` entry that is empty or whitespace-only after trimming: such an entry SHALL NOT be compiled and SHALL NOT become a rule. Skipping an empty entry SHALL NOT be treated as a compile failure (it is distinct from a malformed regex, which SHALL still fail fast at construction). Custom-pattern labels SHALL be assigned only to the retained non-empty entries, numbered contiguously starting at `custom-0`.

During scanning, the scanner SHALL NOT emit a match whose start offset equals its end offset (a zero-width match). This guard SHALL apply to every rule, so any pattern capable of matching zero-width input — an empty string, `a*`, `\b`, or `.*` against an empty region — cannot produce a per-character flood of matches.

#### Scenario: Empty extra pattern produces no rule and no matches

- **WHEN** the scanner is constructed with `patterns_extra` containing a single empty string AND is given non-empty content
- **THEN** construction SHALL succeed AND the empty entry SHALL NOT become a rule AND scanning SHALL return zero matches attributable to that entry

#### Scenario: Zero-width-capable pattern does not flood matches

- **WHEN** the scanner is constructed with an extra pattern that can match zero-width input such as `a*` AND is given content that does not contain a non-empty run of that pattern
- **THEN** scanning SHALL return zero matches for that pattern rather than one match per character position

##### Example: empty pattern on large content

- **GIVEN** `patterns_extra` is `[""]` AND content is a 250000-character string containing no secret-shaped tokens
- **WHEN** the scanner scans the content
- **THEN** the returned match list SHALL be empty, not one match per character position

#### Scenario: Non-empty custom pattern keeps contiguous numbering

- **WHEN** the scanner is constructed with `patterns_extra` equal to `["", "\\bINTERNAL-\\d{6}\\b"]` AND scans content containing `INTERNAL-123456`
- **THEN** the empty entry SHALL be skipped AND the single resulting custom match SHALL carry `pattern_name` equal to `custom-0`
