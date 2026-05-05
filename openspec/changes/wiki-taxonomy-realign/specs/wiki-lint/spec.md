## MODIFIED Requirements

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

### Requirement: Lint scans body wikilinks in nav files and goal guides

In addition to scanning bodies of pages under the 5 type folders, the system SHALL scan the body of every existing nav file (`wiki/index.md`, `wiki/log.md`), validating each `[[wikilink]]` reference against the catalog. Broken links in nav-file bodies SHALL be reported at warn severity (same as knowledge-page body links). The system SHALL NOT scan `wiki/overview.md` (no longer a recognized special file) and SHALL NOT scan any file under `wiki/goals/` (no longer a recognized directory). The trailing "and goal guides" in the requirement title is retained for traceability against the prior version of this requirement; the goal-guide scan is explicitly removed by this revision.

#### Scenario: Broken wikilink in index.md body is flagged

- **WHEN** `wiki/index.md` body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue keyed to `index.md` with message containing "broken wikilink in body"

#### Scenario: Broken wikilink in log.md body is flagged

- **WHEN** `wiki/log.md` body contains `[[ghost]]` and no page named `ghost.md` exists in any folder
- **THEN** lint emits one warning-severity issue keyed to `log.md` with message containing "broken wikilink in body"

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
