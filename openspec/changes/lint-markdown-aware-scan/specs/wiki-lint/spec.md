## MODIFIED Requirements

### Requirement: Lint scans body wikilinks in nav files and goal guides

In addition to scanning bodies of pages under the 5 type folders, the system SHALL scan the body of every existing nav file (`wiki/index.md`, `wiki/log.md`), validating each `[[wikilink]]` reference against the catalog. Broken links in nav-file bodies SHALL be reported at warn severity (same as knowledge-page body links). The system SHALL NOT scan `wiki/overview.md` (no longer a recognized special file) and SHALL NOT scan any file under `wiki/goals/` (no longer a recognized directory). The trailing "and goal guides" in the requirement title is retained for traceability against the prior version of this requirement; the goal-guide scan is explicitly removed by this revision.

When scanning bodies, the system SHALL ignore `[[wikilink]]` occurrences that appear inside markdown code regions, because Obsidian renders such occurrences as literal text rather than as links. A markdown code region is defined as either:

- An inline code span delimited by single backticks on the same line: `` `...` ``.
- A fenced code block delimited by triple backticks on their own lines: ``` ``` ... ``` ```.

The system SHALL parse `[[slug|alias]]` references where the alias separator may be either a literal pipe `|` or a backslash-escaped pipe `\|`, because the latter is the standard markdown table escape used to prevent the pipe from being interpreted as a column delimiter. The captured slug SHALL be the substring before the alias separator and SHALL NOT include the backslash. References whose slug is in the catalog SHALL NOT be flagged regardless of which alias separator was used.

The system SHALL NOT treat the body fragment immediately preceding `\|` (within a wikilink) as part of the slug; specifically, the slug-character class SHALL exclude the backslash so that table-escaped aliases parse correctly.

#### Scenario: Broken wikilink in index.md body is flagged

- **WHEN** `wiki/index.md` body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue keyed to `index.md` with message containing "broken wikilink in body"

#### Scenario: Broken wikilink in log.md body is flagged

- **WHEN** `wiki/log.md` body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue keyed to `log.md` with message containing "broken wikilink in body"

#### Scenario: Wikilink inside inline code is not flagged

- **WHEN** a knowledge page body contains the line `透過 \`[[wikilink]]\` 互相串接` and no page named `wikilink.md` exists in any folder
- **THEN** lint emits zero warnings about that occurrence (the backtick-delimited region is treated as literal text)

#### Scenario: Wikilink inside fenced code block is not flagged

- **WHEN** a knowledge page body contains a fenced code block whose contents include `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits zero warnings about that occurrence

#### Scenario: Table-cell wikilink with escaped alias separator resolves to the bare slug

- **WHEN** a knowledge page body contains `[[resolver-resolve\|Resolver]]` inside a markdown table cell AND a page named `resolver-resolve.md` exists in some type folder
- **THEN** lint emits zero warnings about that occurrence (slug parses as `resolver-resolve`, alias as `Resolver`)

#### Scenario: Table-cell wikilink with escaped separator still flags broken slug

- **WHEN** a knowledge page body contains `[[ghost\|alias]]` inside a markdown table cell AND no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue with message containing "broken wikilink in body" and the reported slug is `ghost` (without the backslash)
