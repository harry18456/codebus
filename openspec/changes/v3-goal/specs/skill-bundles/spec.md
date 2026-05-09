## ADDED Requirements

### Requirement: Goal Bundle Workflow Content

The `codebus-goal/SKILL.md` file written by init SHALL contain a `## Workflow` section (or equivalently-named heading containing the substring `Workflow`) that documents a five-step ingest procedure for the goal verb. The five steps SHALL be presented in order and SHALL describe: (1) exploring the raw mirror under `raw/code/` to find sources relevant to the goal; (2) planning which wiki pages to create or update across the five taxonomy folders (`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`); (3) writing each page's frontmatter and body; (4) establishing wikilinks between pages using filename references; (5) emitting a brief summary line (counts of new and modified pages) to stdout at the end.

The workflow content SHALL NOT inline the schema rules (taxonomy definitions, frontmatter format details, wikilink resolution rules, stop criteria). Schema rules SHALL be referenced by pointing the agent to the cwd-relative `CLAUDE.md` file; the workflow body SHALL NOT duplicate the schema content. References to `CLAUDE.md` as the schema source-of-truth are permitted in workflow steps.

The other two bundles (`codebus-query/SKILL.md`, `codebus-fix/SKILL.md`) SHALL retain their stub workflow content from v3-init until subsequent changes (#6 v3-query, #8 v3-fix) replace them. Their existing requirements (Skill Bundle Layout, Stub Bundle Content Format, Write-If-Missing Semantics) SHALL continue to apply.

#### Scenario: codebus-goal SKILL.md contains five-step workflow markers

- **WHEN** init runs against a repository with no existing `<repo>/.codebus/.claude/skills/codebus-goal/SKILL.md`
- **THEN** the resulting file SHALL contain a `## Workflow` heading AND the body under that heading SHALL contain at least five distinct numbered list items (lines beginning with `1.`, `2.`, `3.`, `4.`, `5.`)

#### Scenario: codebus-goal workflow references raw/code and wiki cwd-relatively

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body SHALL contain the substring `raw/code/` AND SHALL contain the substring `wiki/` AND SHALL NOT contain the substring `.codebus/raw/code/` AND SHALL NOT contain the substring `.codebus/wiki/`

#### Scenario: codebus-goal workflow defers schema rules to CLAUDE.md

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body SHALL contain the substring `CLAUDE.md` (the schema reference) AND SHALL NOT contain inline taxonomy definitions enumerating concepts, entities, modules, processes, and synthesis as the five page types in a single sentence (the schema's authoritative enumeration belongs in `CLAUDE.md` only)

#### Scenario: codebus-goal workflow mentions all five taxonomy folder names

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body SHALL mention each of `concepts`, `entities`, `modules`, `processes`, `synthesis` at least once (so the agent knows which folders are valid page locations) but the mention SHALL be brief enumeration only (e.g., as a parenthetical list within a single step), not a definition of each type

#### Scenario: codebus-query and codebus-fix bundles retain stub workflow

- **WHEN** init runs against a repository with no existing skill bundles
- **THEN** the resulting `<repo>/.codebus/.claude/skills/codebus-query/SKILL.md` SHALL retain stub workflow content (no five-step ingest expansion) AND `<repo>/.codebus/.claude/skills/codebus-fix/SKILL.md` SHALL likewise retain stub workflow content
