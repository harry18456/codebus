## MODIFIED Requirements

### Requirement: Vault Layout

The system SHALL create a `.codebus/` vault under the source repository root containing the following subdirectories: `wiki/concepts/`, `wiki/entities/`, `wiki/modules/`, `wiki/processes/`, `wiki/synthesis/`, `raw/code/`, and `log/`. The system SHALL NOT create `output/` or `goals.jsonl` inside the vault.

The system SHALL also materialize two nav files at the wiki root — `wiki/index.md` and `wiki/log.md` — when they do not already exist. Each nav file SHALL begin with a YAML frontmatter block (delimited by `---` lines) carrying the required fields per the wiki schema (`title: <human-readable string>`, `type: synthesis`, `sources: []`, `goals: []`, `created: <UTC YYYY-MM-DD>`, `updated: <UTC YYYY-MM-DD>`, `related: []`, `stale: false`), followed by a single short placeholder line of body text. The placeholder body SHALL NOT contain any wikilink syntax (no `[[…]]` token) so the existing `broken-wikilink` lint rule cannot misfire on a freshly-inited vault. The `created` / `updated` dates SHALL be the UTC date observed at init time.

Nav file materialization SHALL use write-if-missing semantics: when the target nav file already exists, the system SHALL NOT modify it (preserving any prior user or agent customization), and the two files SHALL be evaluated independently. The materialization step SHALL be idempotent across re-runs of init.

The existing `nav-missing` lint rule defined in `lint-feedback-loop` remains in force — it triggers when the nav files are absent (e.g., a user manually deletes them). On a freshly-inited vault, the rule SHALL NOT fire because both files exist.

#### Scenario: Init creates the seven required subdirectories under .codebus

- **WHEN** init is invoked against a repository with no existing `.codebus/`
- **THEN** the system SHALL create `.codebus/wiki/concepts/`, `.codebus/wiki/entities/`, `.codebus/wiki/modules/`, `.codebus/wiki/processes/`, `.codebus/wiki/synthesis/`, `.codebus/raw/code/`, and `.codebus/log/` AND SHALL NOT create `.codebus/output/` or `.codebus/goals.jsonl`

#### Scenario: Re-running init is idempotent for layout

- **WHEN** init is invoked twice in succession against the same repository
- **THEN** both invocations SHALL succeed AND the second SHALL NOT change the directory listing of the seven required subdirectories beyond what the first established

#### Scenario: Init materializes both nav files at the wiki root

- **WHEN** init runs against `<repo>` with no existing `<repo>/.codebus/wiki/index.md` and no existing `<repo>/.codebus/wiki/log.md`
- **THEN** the system SHALL create `<repo>/.codebus/wiki/index.md` AND `<repo>/.codebus/wiki/log.md` AND each file SHALL begin with `---\n` AND each file SHALL contain frontmatter keys `title`, `type`, `sources`, `goals`, `created`, `updated`, `related`, AND `stale` AND each file's `type` value SHALL be `synthesis` AND each file SHALL contain a non-empty body line after the closing `---` frontmatter delimiter

#### Scenario: Nav placeholder body contains no wikilink syntax

- **WHEN** init writes either nav file in the fresh-vault case
- **THEN** the file body (the text after the closing `---` frontmatter delimiter) SHALL NOT contain the substring `[[` AND SHALL NOT contain the substring `]]`, so the `broken-wikilink` lint rule cannot misfire on a freshly-inited vault

#### Scenario: Nav write-if-missing preserves existing files

- **WHEN** init runs against `<repo>` whose `<repo>/.codebus/wiki/index.md` already exists with custom content
- **THEN** the system SHALL NOT modify the existing `index.md` AND its content SHALL be byte-identical before and after init AND init SHALL still independently create `wiki/log.md` when `log.md` is absent

#### Scenario: Re-running init leaves nav files untouched

- **WHEN** init is invoked twice in succession against the same repository
- **THEN** both invocations SHALL succeed AND the second invocation SHALL NOT modify `wiki/index.md` or `wiki/log.md` beyond what the first established AND the bytes of both files SHALL be identical between the two post-init states

#### Scenario: Lint on a freshly-inited vault does not report nav-missing

- **WHEN** the user runs `codebus lint` against a vault that has just been initialized via `codebus init` (no goals run yet, no user edits)
- **THEN** the lint output SHALL NOT contain any issue whose `rule_id` is `nav-missing` (the `wiki/index.md` and `wiki/log.md` files materialized by init satisfy the presence half of the rule)
