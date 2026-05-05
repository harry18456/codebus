# wiki-lint Specification

## Purpose

Validate the structure and Obsidian-compatibility of a `.codebus/wiki/` vault. Lint runs in two modes: (1) auto-lint at the end of every `--goal` ingest in soft mode (warnings surface but never block auto-commit or abort the goal), and (2) a standalone `--check` command that runs lint as a read-only operation against an existing vault, exits 1 when any error-severity issue is reported, and never invokes the LLM provider or writes to disk. Lint enforces frontmatter and `related[]` integrity at error severity, flags structural and Obsidian-compatibility violations (folder/type mismatch, duplicate slugs, missing nav files, broken body wikilinks) at warn severity, and validates `[[wikilink]]` references against a catalog that includes the 5 type folders, the three nav specials (`overview.md` / `index.md` / `log.md`), and every `wiki/goals/<slug>.md` reading guide. The result schema reports `pagesScanned` + `navFilesScanned` so a clean run is honest about what was inspected.

## Requirements

### Requirement: Auto-lint runs at end of every ingest in soft mode

After post-processing (source enrichment + stale-detect) and before auto-commit, the system SHALL run `lintWiki(vaultRoot)` and capture the result on the goal flow's return value. Lint failures SHALL NOT block the auto-commit, abort the goal, or mutate any vault file. If lint itself throws (e.g., wiki/ missing), the system SHALL swallow the error and treat the run as "no lint result to report".

#### Scenario: Successful goal with clean wiki produces no lint output beyond the done banner summary

- **WHEN** the user runs `codebus --repo X --goal "..."` and the resulting wiki passes all lint rules
- **THEN** the goal flow completes, auto-commit runs, and the done-banner sequence emits no warning summary

#### Scenario: Goal with lint warnings still commits and surfaces a one-line summary

- **WHEN** the user runs `codebus --repo X --goal "..."` and the resulting wiki has at least one lint warning
- **THEN** auto-commit still runs, AND the done-banner sequence emits one summary line of the form `! lint: N warning(s) — codebus --check 看詳情` (emoji or symbol per render mode)

#### Scenario: Lint internal failure does not break ingest

- **WHEN** lint throws during goal execution (e.g., wiki/ missing because agent refused)
- **THEN** the goal flow continues to auto-commit and returns `lint: null` instead of propagating the error


<!-- @trace
source: lint-coverage
updated: 2026-05-05
code:
  - src/core/wiki/lint.ts
  - src/ui/lint-report.ts
  - src/commands/goal.ts
  - src/commands/check.ts
  - src/cli.ts
  - src/core/vault/layout.ts
  - src/core/wiki/frontmatter.ts
  - src/core/wiki/types.ts
  - src/schema/claude-md.ts
tests:
  - tests/core/wiki/lint.test.ts
  - tests/commands/goal.test.ts
  - tests/commands/check.test.ts
-->

---
### Requirement: Standalone `--check` command runs lint as a read-only operation

When invoked with `--check` and `--repo <path>`, the system SHALL run lint against the existing vault without invoking the LLM provider, without running init, and without writing or committing any file. The system SHALL print the full lint report and SHALL exit with code 1 if any error-severity issue is reported, otherwise code 0.

#### Scenario: --check on a vault with errors exits 1

- **WHEN** the user runs `codebus --repo X --check` and the vault has at least one error-severity issue
- **THEN** the system prints the full lint report and exits with status code 1

#### Scenario: --check on a clean vault exits 0

- **WHEN** the user runs `codebus --repo X --check` and the vault has no error-severity issues
- **THEN** the system prints a single-line "no issues" summary and exits with status code 0

#### Scenario: --check requires an existing vault

- **WHEN** the user runs `codebus --repo X --check` and `.codebus/` does not exist under X
- **THEN** the system throws an error directing the user to run `codebus --repo X` (init) or `codebus --repo X --goal "..."` (ingest) first, and does NOT auto-init the vault


<!-- @trace
source: lint-coverage
updated: 2026-05-05
code:
  - src/core/wiki/lint.ts
  - src/ui/lint-report.ts
  - src/commands/goal.ts
  - src/commands/check.ts
  - src/cli.ts
  - src/core/vault/layout.ts
  - src/core/wiki/frontmatter.ts
  - src/core/wiki/types.ts
  - src/schema/claude-md.ts
tests:
  - tests/core/wiki/lint.test.ts
  - tests/commands/goal.test.ts
  - tests/commands/check.test.ts
-->

---
### Requirement: Lint enforces frontmatter and related[] integrity at error severity

For each `.md` file under any of the 5 type folders (`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`), the system SHALL parse YAML frontmatter via the project's `parsePage` function and SHALL emit an error-severity issue when parse fails. For each entry in the page's `related` array, the system SHALL emit an error-severity issue when the entry is not in `[[slug]]` format or when the referenced slug does not match any page in the catalog.

#### Scenario: Frontmatter parse failure produces a single error

- **WHEN** a knowledge page has malformed YAML frontmatter
- **THEN** lint emits one error-severity issue keyed to that page's relative path with message containing "frontmatter parse failed"

#### Scenario: related[] entry not in wikilink format produces an error

- **WHEN** a page's `related` array contains an entry like `"[[broken"` or `"plain-text"`
- **THEN** lint emits one error-severity issue with message containing "related[] entry not in [[wikilink]] format"

#### Scenario: related[] entry referencing a non-existent slug produces an error

- **WHEN** a page's `related` array contains `"[[ghost]]"` and no page named `ghost.md` exists under any of the 5 type folders, special files (overview/index/log), or goal guides
- **THEN** lint emits one error-severity issue with message containing "broken wikilink in related"


<!-- @trace
source: lint-coverage
updated: 2026-05-05
code:
  - src/core/wiki/lint.ts
  - src/ui/lint-report.ts
  - src/commands/goal.ts
  - src/commands/check.ts
  - src/cli.ts
  - src/core/vault/layout.ts
  - src/core/wiki/frontmatter.ts
  - src/core/wiki/types.ts
  - src/schema/claude-md.ts
tests:
  - tests/core/wiki/lint.test.ts
  - tests/commands/goal.test.ts
  - tests/commands/check.test.ts
-->

---
### Requirement: Lint emits warnings for structural and Obsidian-compatibility violations

The system SHALL emit warning-severity issues (not errors) for the following conditions, on the principle that they degrade vault usability but do not corrupt the wiki:

- A page lives directly under `wiki/` (not a special file) instead of a type folder
- Two or more pages across different type folders share the same slug filename
- Either of `index.md` or `log.md` is missing at `wiki/` root
- A body wikilink in a knowledge page or nav file points to a slug not in the catalog

The system SHALL NOT emit a warning when a page's frontmatter `type` does not match its containing type folder. The folder layout is an organizational hint for Obsidian sidebar rendering, not a normative contract; frontmatter `type` is the authoritative metadata.

The system SHALL NOT emit a warning when `wiki/overview.md` is absent. `overview.md` is no longer a special file. If `overview.md` exists at `wiki/` root, it is treated as a regular page in `wiki/` root and SHALL be flagged by the "page lives in wiki/ root" rule above (the same way any non-special root file is flagged).

#### Scenario: Page in wiki/ root is flagged

- **WHEN** a `.md` file other than `index.md` / `log.md` exists directly under `wiki/`
- **THEN** lint emits one warning-severity issue keyed to `<filename>` with message indicating the page MUST live in one of the 5 type folders

#### Scenario: Folder/type mismatch is no longer flagged

- **WHEN** a file at `wiki/concepts/foo.md` has frontmatter `type: module`
- **THEN** lint emits zero warnings for the folder/type relationship (the page may still be flagged for other reasons such as broken wikilinks)

#### Scenario: Duplicate slug across type folders is flagged on every occurrence

- **WHEN** both `wiki/concepts/cart.md` and `wiki/entities/cart.md` exist
- **THEN** lint emits two warning-severity issues (one per occurrence) each containing "duplicate slug 'cart'"

#### Scenario: Missing index.md is flagged

- **WHEN** `wiki/index.md` does not exist but `wiki/log.md` does
- **THEN** lint emits one warning-severity issue keyed to `index.md` with message containing "missing"

#### Scenario: Missing log.md is flagged

- **WHEN** `wiki/log.md` does not exist but `wiki/index.md` does
- **THEN** lint emits one warning-severity issue keyed to `log.md` with message containing "missing"

#### Scenario: Missing overview.md is not flagged

- **WHEN** `wiki/overview.md` does not exist (regardless of whether index.md / log.md exist)
- **THEN** lint emits zero warnings related to `overview.md`

#### Scenario: Body wikilink to nonexistent slug is flagged at warn severity

- **WHEN** a knowledge page body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue (not error) keyed to that page's relative path with message containing "broken wikilink in body"


<!-- @trace
source: lint-coverage
updated: 2026-05-05
code:
  - src/core/wiki/lint.ts
  - src/ui/lint-report.ts
  - src/commands/goal.ts
  - src/commands/check.ts
  - src/cli.ts
  - src/core/vault/layout.ts
  - src/core/wiki/frontmatter.ts
  - src/core/wiki/types.ts
  - src/schema/claude-md.ts
tests:
  - tests/core/wiki/lint.test.ts
  - tests/commands/goal.test.ts
  - tests/commands/check.test.ts
-->

---
### Requirement: Wikilink catalog includes nav files and goal guides as valid targets

When constructing the slug catalog used to validate `[[wikilink]]` references, the system SHALL include slugs from these sources: (a) every `.md` under the 5 type folders, (b) `index.md` and `log.md` at `wiki/` root if they exist. Slugs are derived as the filename without the `.md` extension. If a special file does not exist, its slug SHALL NOT be added to the catalog (so links to missing specials are correctly flagged as broken). The catalog SHALL NOT include `overview.md` (no longer a recognized special file) and SHALL NOT include any slugs from `wiki/goals/` (no longer a recognized directory). The trailing "and goal guides" in the requirement title is retained for traceability against the prior version of this requirement; the goal-guide source is explicitly removed by this revision.

#### Scenario: [[index]] from a knowledge page resolves when index.md exists

- **WHEN** `wiki/index.md` exists AND a knowledge page body contains `[[index]]`
- **THEN** lint does NOT flag the link as broken

#### Scenario: [[log]] is flagged broken when log.md does not exist

- **WHEN** `wiki/log.md` does not exist AND a knowledge page body contains `[[log]]`
- **THEN** lint emits one error-severity issue (if the link is in `related`) or warn-severity issue (if in body) for the broken link, in addition to the existing "missing log.md" warning

#### Scenario: [[goal-slug]] is flagged broken even when wiki/goals/<slug>.md exists

- **WHEN** `wiki/goals/project-purpose.md` exists AND any page or nav file contains `[[project-purpose]]`
- **THEN** lint flags the link as broken at warn severity (body) or error severity (related[]), because `wiki/goals/` is no longer scanned for catalog slugs


<!-- @trace
source: lint-coverage
updated: 2026-05-05
code:
  - src/core/wiki/lint.ts
  - src/ui/lint-report.ts
  - src/commands/goal.ts
  - src/commands/check.ts
  - src/cli.ts
  - src/core/vault/layout.ts
  - src/core/wiki/frontmatter.ts
  - src/core/wiki/types.ts
  - src/schema/claude-md.ts
tests:
  - tests/core/wiki/lint.test.ts
  - tests/commands/goal.test.ts
  - tests/commands/check.test.ts
-->

---
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

---
### Requirement: Lint result schema and report format

`lintWiki` SHALL return a result object containing: `pagesScanned` (count of knowledge pages with successfully parsed frontmatter under the 5 type folders), `navFilesScanned` (count of existing nav files actually read: `index.md` and `log.md` at `wiki/` root that exist), `issues` (array of `{path, severity, message}` records), `errorCount`, and `warnCount`. The system SHALL print coverage as `N page(s) + M nav file(s) scanned` so the absence of issues is honest about what was inspected.

#### Scenario: pagesScanned counts only successfully parsed knowledge pages

- **WHEN** wiki has 2 knowledge pages with valid frontmatter and 1 page with malformed frontmatter
- **THEN** `pagesScanned` equals 2 and `errorCount` is at least 1

#### Scenario: navFilesScanned counts only existing index.md and log.md

- **WHEN** wiki has `index.md` (no `log.md`)
- **THEN** `navFilesScanned` equals 1

#### Scenario: navFilesScanned ignores files in wiki/goals/

- **WHEN** wiki has both `index.md` and `log.md`, plus 3 files under `wiki/goals/`
- **THEN** `navFilesScanned` equals 2 (the goal-guide files are not counted)

#### Scenario: Clean run reports both counts in the no-issues line

- **WHEN** lint runs on a clean vault with 3 knowledge pages and 2 nav files (index.md + log.md)
- **THEN** the printed report's no-issues line contains "3 pages + 2 nav files scanned, no issues"


<!-- @trace
source: lint-coverage
updated: 2026-05-05
code:
  - src/core/wiki/lint.ts
  - src/ui/lint-report.ts
  - src/commands/goal.ts
  - src/commands/check.ts
  - src/cli.ts
  - src/core/vault/layout.ts
  - src/core/wiki/frontmatter.ts
  - src/core/wiki/types.ts
  - src/schema/claude-md.ts
tests:
  - tests/core/wiki/lint.test.ts
  - tests/commands/goal.test.ts
  - tests/commands/check.test.ts
-->
