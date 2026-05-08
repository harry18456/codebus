## MODIFIED Requirements

### Requirement: Lint emits warnings for structural and Obsidian-compatibility violations

The system SHALL emit warning-severity issues (not errors) for the following conditions, on the principle that they degrade vault usability but do not corrupt the wiki:

- A page lives directly under `wiki/` (not a special file) instead of a type folder
- Two or more pages across different type folders share the same slug filename
- Either of `index.md` or `log.md` is missing at `wiki/` root
- A body wikilink in a knowledge page or nav file points to a slug not in the catalog
- A page exceeds the per-type-folder byte-size threshold (page-size threshold)
- A non-`.md` file or a nested sub-folder exists inside one of the 5 type folders, or an unrecognized folder exists directly under `wiki/` (unexpected-file detection)

The system SHALL apply the following per-file-type byte-size thresholds, comparing strictly greater-than (`>`) the threshold to the file's UTF-8 byte length:

- `wiki/index.md` SHALL warn when size exceeds 1024 bytes (1 KiB)
- `wiki/synthesis/<slug>.md` SHALL warn when size exceeds 5120 bytes (5 KiB)
- `wiki/{concepts,entities,modules,processes}/<slug>.md` SHALL warn when size exceeds 8192 bytes (8 KiB)
- `wiki/log.md` SHALL NOT trigger a page-size warning regardless of size, because log.md is chronological-by-design and grows unboundedly

The page-size warning message SHALL contain the literal substring `size N bytes` (with N replaced by the actual size) and the literal substring `threshold M bytes` (with M replaced by the threshold), so callers can extract both values without reparsing the message.

For unexpected-file detection, the system SHALL apply the following rules when scanning the vault root and the 5 type folders:

- A directory entry directly under `wiki/` whose name is not in the recognized set (5 type folders plus `goals/`) and is not a hidden entry (does not start with `.`) SHALL emit a warning keyed to the entry name with message containing `unrecognized folder under wiki/`
- A directory entry directly under any of the 5 type folders SHALL emit a warning keyed to `<folder>/<name>` with message containing `nested sub-folder in type folder`
- A non-directory entry directly under any of the 5 type folders whose extension is not `.md` and whose name does not start with `.` SHALL emit a warning keyed to `<folder>/<name>` with message containing `non-.md file in type folder`
- Any entry whose name starts with `.` (e.g., `.obsidian`, `.gitkeep`, `.DS_Store`) SHALL be skipped silently and SHALL NOT trigger an unexpected-file warning

The system SHALL NOT emit a warning when a page's frontmatter `type` does not match its containing type folder. The folder layout is an organizational hint for Obsidian sidebar rendering, not a normative contract; frontmatter `type` is the authoritative metadata.

The system SHALL NOT emit a warning when `wiki/overview.md` is absent. `overview.md` is no longer a special file. If `overview.md` exists at `wiki/` root, it is treated as a regular page in `wiki/` root and SHALL be flagged by the "page lives in wiki/ root" rule above (the same way any non-special root file is flagged).

#### Scenario: Page in wiki/ root is flagged

- **WHEN** a `.md` file other than `index.md` / `log.md` exists directly under `wiki/`
- **THEN** lint emits one warning-severity issue keyed to `<filename>` with message indicating the page MUST live in one of the 5 type folders

#### Scenario: Folder/type mismatch is no longer flagged

- **WHEN** a file at `wiki/concepts/foo.md` has frontmatter `type: module`
- **THEN** lint emits zero warnings for the folder/type relationship (the page is permitted to be flagged for other reasons such as broken wikilinks)

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

#### Scenario: Oversized index.md is flagged

- **WHEN** `wiki/index.md` is 1500 bytes (above the 1024-byte threshold)
- **THEN** lint emits exactly one warning-severity issue keyed to `index.md` with message containing both `size 1500 bytes` and `threshold 1024 bytes`

#### Scenario: Oversized synthesis page is flagged

- **WHEN** `wiki/synthesis/cart-flow.md` is 6000 bytes (above the 5120-byte threshold)
- **THEN** lint emits exactly one warning-severity issue keyed to `synthesis/cart-flow.md` with message containing both `size 6000 bytes` and `threshold 5120 bytes`

#### Scenario: Oversized concepts page is flagged

- **WHEN** `wiki/concepts/foo.md` is 9000 bytes (above the 8192-byte threshold)
- **THEN** lint emits exactly one warning-severity issue keyed to `concepts/foo.md` with message containing both `size 9000 bytes` and `threshold 8192 bytes`

#### Scenario: Oversized log.md is not flagged

- **WHEN** `wiki/log.md` is 50000 bytes
- **THEN** lint emits zero page-size warnings for `log.md`

#### Scenario: Page exactly at threshold is not flagged

- **WHEN** `wiki/concepts/foo.md` is exactly 8192 bytes
- **THEN** lint emits zero page-size warnings for `foo.md` (the comparison is strictly greater-than)

#### Scenario: Page below threshold is not flagged

- **WHEN** `wiki/concepts/foo.md` is 4000 bytes
- **THEN** lint emits zero page-size warnings for `foo.md`

#### Scenario: Non-.md file in type folder is flagged

- **WHEN** `wiki/concepts/foo.txt` exists alongside valid `.md` pages
- **THEN** lint emits exactly one warning-severity issue keyed to `concepts/foo.txt` with message containing `non-.md file in type folder`

#### Scenario: Nested sub-folder in type folder is flagged

- **WHEN** `wiki/modules/legacy/` directory exists (containing any files)
- **THEN** lint emits exactly one warning-severity issue keyed to `modules/legacy` with message containing `nested sub-folder in type folder`

#### Scenario: Unrecognized folder under wiki/ is flagged

- **WHEN** `wiki/scratch/` directory exists alongside the 5 recognized type folders
- **THEN** lint emits exactly one warning-severity issue keyed to `scratch` with message containing `unrecognized folder under wiki/`

#### Scenario: Hidden entries are skipped silently

- **WHEN** `wiki/.obsidian/` directory and `wiki/.gitkeep` file exist
- **THEN** lint emits zero unexpected-file warnings for either entry (hidden entries starting with `.` are excluded)
