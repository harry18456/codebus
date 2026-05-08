## ADDED Requirements

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

### Requirement: Lint emits warnings for structural and Obsidian-compatibility violations

The system SHALL emit warning-severity issues (not errors) for the following conditions, on the principle that they degrade vault usability but do not corrupt the wiki:

- A page lives directly under `wiki/` (not a special file) instead of a type folder
- A page's frontmatter `type` does not match its containing folder per the canonical type↔folder map
- Two or more pages across different type folders share the same slug filename
- Any of `overview.md`, `index.md`, or `log.md` is missing at `wiki/` root
- A body wikilink in a knowledge page or nav file points to a slug not in the catalog

#### Scenario: Page in wiki/ root is flagged

- **WHEN** a `.md` file other than `overview.md` / `index.md` / `log.md` exists directly under `wiki/`
- **THEN** lint emits one warning-severity issue keyed to `<filename>` with message indicating the page MUST live in one of the 5 type folders

#### Scenario: Folder/type mismatch is flagged

- **WHEN** a file at `wiki/concepts/foo.md` has frontmatter `type: module`
- **THEN** lint emits one warning-severity issue with message containing "folder/type mismatch" and listing the expected type ('concept')

#### Scenario: Duplicate slug across type folders is flagged on every occurrence

- **WHEN** both `wiki/concepts/cart.md` and `wiki/entities/cart.md` exist
- **THEN** lint emits two warning-severity issues (one per occurrence) each containing "duplicate slug 'cart'"

#### Scenario: Missing special file is flagged once per file

- **WHEN** `wiki/overview.md` does not exist but `wiki/index.md` and `wiki/log.md` do
- **THEN** lint emits one warning-severity issue keyed to `overview.md` with message containing "missing"

#### Scenario: Body wikilink to nonexistent slug is flagged at warn severity

- **WHEN** a knowledge page body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue (not error) keyed to that page's relative path with message containing "broken wikilink in body"

### Requirement: Wikilink catalog includes nav files and goal guides as valid targets

When constructing the slug catalog used to validate `[[wikilink]]` references, the system SHALL include slugs from all four sources: (a) every `.md` under the 5 type folders, (b) `overview.md` / `index.md` / `log.md` at `wiki/` root if they exist, (c) every `.md` under `wiki/goals/`. Slugs are derived as the filename without the `.md` extension. If a special file does not exist, its slug SHALL NOT be added to the catalog (so links to missing specials are correctly flagged as broken).

#### Scenario: [[overview]] from a knowledge page resolves when overview.md exists

- **WHEN** `wiki/overview.md` exists AND a knowledge page body contains `[[overview]]`
- **THEN** lint does NOT flag the link as broken

#### Scenario: [[overview]] is flagged broken when overview.md does not exist

- **WHEN** `wiki/overview.md` does not exist AND a knowledge page body contains `[[overview]]`
- **THEN** lint emits one error-severity issue (if the link is in `related`) or warn-severity issue (if in body) for the broken link, in addition to the existing "missing special file" warning

#### Scenario: [[goal-slug]] resolves when goals/<slug>.md exists

- **WHEN** `wiki/goals/project-purpose.md` exists AND any page or nav file contains `[[project-purpose]]`
- **THEN** lint does NOT flag the link as broken

### Requirement: Lint scans body wikilinks in nav files and goal guides

In addition to scanning bodies of pages under the 5 type folders, the system SHALL scan the body of every existing nav file (`wiki/overview.md`, `wiki/index.md`, `wiki/log.md`) and every `.md` under `wiki/goals/`, validating each `[[wikilink]]` reference against the catalog. Broken links in nav-file bodies SHALL be reported at warn severity (same as knowledge-page body links).

#### Scenario: Broken wikilink in index.md body is flagged

- **WHEN** `wiki/index.md` body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue keyed to `index.md` with message containing "broken wikilink in body"

#### Scenario: Broken wikilink in goal guide body is flagged

- **WHEN** `wiki/goals/foo.md` body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue keyed to `goals/foo.md` with message containing "broken wikilink in body"

### Requirement: Lint result schema and report format

`lintWiki` SHALL return a result object containing: `pagesScanned` (count of knowledge pages with successfully parsed frontmatter under the 5 type folders), `navFilesScanned` (count of existing nav files actually read: overview/index/log that exist plus every `.md` under goals/), `issues` (array of `{path, severity, message}` records), `errorCount`, and `warnCount`. The system SHALL print coverage as `N page(s) + M nav file(s) scanned` so the absence of issues is honest about what was inspected.

#### Scenario: pagesScanned counts only successfully parsed knowledge pages

- **WHEN** wiki has 2 knowledge pages with valid frontmatter and 1 page with malformed frontmatter
- **THEN** `pagesScanned` equals 2 and `errorCount` is at least 1

#### Scenario: navFilesScanned counts existing specials plus every goal guide

- **WHEN** wiki has overview.md and index.md (no log.md), plus 2 goal guides
- **THEN** `navFilesScanned` equals 4 (2 specials + 2 goal guides)

#### Scenario: Clean run reports both counts in the no-issues line

- **WHEN** lint runs on a clean vault with 3 knowledge pages and 4 nav files (3 specials + 1 goal guide)
- **THEN** the printed report's no-issues line contains "3 pages + 4 nav files scanned, no issues"
