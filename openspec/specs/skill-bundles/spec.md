# skill-bundles Specification

## Purpose

TBD - created by archiving change 'v3-init'. Update Purpose after archive.

## Requirements

### Requirement: Skill Bundle Layout

The system SHALL create three skill bundles **inside the vault directory** at `<repo>/.codebus/.claude/skills/`: one each at `<repo>/.codebus/.claude/skills/codebus-goal/`, `<repo>/.codebus/.claude/skills/codebus-query/`, and `<repo>/.codebus/.claude/skills/codebus-fix/`. Each bundle SHALL contain at minimum a `SKILL.md` file at its root. The system SHALL NOT create a `codebus-lint` skill bundle (lint is a direct CLI subcommand in path D, not a skill). The system SHALL NOT write skill bundles into `~/.claude/skills/codebus-*/` (user-level) NOR `<repo>/.claude/skills/codebus-*/` (repo-root project-scoped).

This vault-internal placement makes skills discoverable only when an agentic CLI runs with cwd at the vault root (`<repo>/.codebus/`). Combined with cwd-bounded filesystem access, the agent's read scope is naturally constrained to vault contents (`raw/code/`, `wiki/`, `CLAUDE.md`, `manifest.yaml`, `log/`) — it cannot see source files outside the vault.

#### Scenario: Init creates exactly three skill bundle directories under the vault

- **WHEN** init runs against `<repo>` and the three target paths under `<repo>/.codebus/.claude/skills/` do not exist
- **THEN** the system SHALL create `<repo>/.codebus/.claude/skills/codebus-goal/`, `<repo>/.codebus/.claude/skills/codebus-query/`, and `<repo>/.codebus/.claude/skills/codebus-fix/` AND each SHALL contain a `SKILL.md` file AND the system SHALL NOT create `<repo>/.codebus/.claude/skills/codebus-lint/`

#### Scenario: Init does not write to user-level or repo-root skills directories

- **WHEN** init runs against `<repo>`
- **THEN** the system SHALL NOT create or modify any path under `~/.claude/skills/codebus-*/` AND SHALL NOT create or modify any path under `<repo>/.claude/skills/codebus-*/` (i.e., directly under repo root, outside the vault)

#### Scenario: Skill bundle directory creation handles missing parents

- **WHEN** init runs against `<repo>` whose `.codebus/.claude/` does not yet exist
- **THEN** the system SHALL create `<repo>/.codebus/.claude/` and `<repo>/.codebus/.claude/skills/` parent chain as needed


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Stub Bundle Content Format

Each `SKILL.md` written by init in this change SHALL contain a YAML frontmatter block (delimited by `---` lines) followed by a body. The frontmatter SHALL define `name` (matching the bundle directory name, e.g., `codebus-goal`) and `description` (a single-line string under 256 characters describing the verb's purpose). The body SHALL contain at minimum: a one-line trigger hint instructing when to activate the skill; a reference instructing the agent to read `CLAUDE.md` (cwd-relative, the vault's per-repo schema file) for schema rules; a hard-scope rule paragraph; and a path-translation rule paragraph.

Path references inside the body SHALL be **cwd-relative**, NOT `.codebus/`-prefixed, because the agentic CLI is invoked with cwd at the vault root (the `.codebus/` directory). For example, the read scope is `raw/code/` (NOT `.codebus/raw/code/`); the write scope is `wiki/` (NOT `.codebus/wiki/`); the schema reference is `CLAUDE.md` (NOT `.codebus/CLAUDE.md`).

#### Scenario: Each stub SKILL.md has required frontmatter fields

- **WHEN** init writes any of the three skill bundle SKILL.md files
- **THEN** each file SHALL begin with `---\n` AND contain a `name:` line whose value matches the bundle directory name AND contain a `description:` line whose value is a non-empty single-line string

#### Scenario: Each stub body references CLAUDE.md cwd-relatively

- **WHEN** init writes any of the three skill bundle SKILL.md files
- **THEN** the body (everything after the second `---` line) SHALL contain the substring `CLAUDE.md` for the schema rules reference AND SHALL NOT contain the substring `.codebus/CLAUDE.md` (since cwd is already the vault root)

#### Scenario: Each stub body declares hard scope using cwd-relative paths

- **WHEN** init writes any of the three skill bundle SKILL.md files
- **THEN** the body SHALL state that the agent's read scope is `raw/code/` (cwd-relative) AND write scope is `wiki/` (cwd-relative) AND the agent MUST NOT read or write any path that escapes the cwd. The body SHALL NOT use `.codebus/raw/code/` or `.codebus/wiki/` (with `.codebus/` prefix) when describing scope.

#### Scenario: Each stub body declares path translation rule

- **WHEN** init writes any of the three skill bundle SKILL.md files
- **THEN** the body SHALL state that frontmatter `sources[].path` values are repo-relative logical paths AND MUST NOT include the `raw/code/` prefix

#### Scenario: Stub body avoids embedding full verb workflow

- **WHEN** init writes any of the three skill bundle SKILL.md files in this change
- **THEN** the body SHALL be 80 lines or fewer (full per-verb workflow content is added by subsequent changes; this stub is intentionally minimal but must include the four required reference / hard-scope / path-translation paragraphs above)


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Write-If-Missing Semantics

For each skill bundle SKILL.md target path under `<repo>/.codebus/.claude/skills/codebus-{verb}/`, the system SHALL write the stub content ONLY when the file does not exist. When the file already exists, the system SHALL NOT modify it (preserving any user customization or content from a future change that has already populated it).

#### Scenario: First-time init writes all three SKILL.md files

- **WHEN** init runs against `<repo>` where none of the three SKILL.md target paths exist
- **THEN** all three files SHALL be created with stub content

#### Scenario: Re-init preserves user-modified SKILL.md

- **WHEN** the user manually edits `<repo>/.codebus/.claude/skills/codebus-goal/SKILL.md` to add custom workflow text and then runs init again
- **THEN** the system SHALL NOT modify that file AND the user's custom workflow text SHALL be preserved verbatim

#### Scenario: Mixed state writes only missing bundles

- **WHEN** init runs against `<repo>` in a state where `codebus-goal/SKILL.md` exists but `codebus-query/SKILL.md` and `codebus-fix/SKILL.md` do not
- **THEN** the system SHALL leave `codebus-goal/SKILL.md` unchanged AND SHALL create `codebus-query/SKILL.md` and `codebus-fix/SKILL.md` with stub content

<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Goal Bundle Workflow Content

The `codebus-goal/SKILL.md` file written by init SHALL contain a `## Workflow` section (or equivalently-named heading containing the substring `Workflow`) that documents a five-step ingest procedure for the goal verb. The five steps SHALL be presented in order and SHALL describe: (1) exploring the raw mirror under `raw/code/` to find sources relevant to the goal; (2) planning which wiki pages to create or update across the five taxonomy folders (`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/`); (3) writing each page's frontmatter and body; (4) establishing wikilinks between pages using filename references; (5) emitting a brief summary line (counts of new and modified pages) to stdout at the end.

The workflow content SHALL NOT inline the schema rules (taxonomy definitions, frontmatter format details, wikilink resolution rules, stop criteria). Schema rules SHALL be referenced by pointing the agent to the cwd-relative `CLAUDE.md` file; the workflow body SHALL NOT duplicate the schema content. References to `CLAUDE.md` as the schema source-of-truth are permitted in workflow steps.

The remaining bundle (`codebus-fix/SKILL.md`) SHALL retain its stub workflow content from v3-init until subsequent change #8 v3-fix replaces it. Its existing requirements (Skill Bundle Layout, Stub Bundle Content Format, Write-If-Missing Semantics) SHALL continue to apply.

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

#### Scenario: codebus-fix bundle retains stub workflow

- **WHEN** init runs against a repository with no existing skill bundles
- **THEN** the resulting `<repo>/.codebus/.claude/skills/codebus-fix/SKILL.md` SHALL retain stub workflow content (no expanded workflow), pending replacement by v3-fix

#### Scenario: codebus-goal workflow body is written in English

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body (everything under the `## Workflow` heading through end of file) SHALL be written in English: it SHALL NOT contain any character in the CJK Unified Ideographs block (Unicode range U+4E00 through U+9FFF), with the exception of file path components or wikilink slugs that remain ASCII anyway

#### Scenario: Step 5 instruction is abstract, not a literal output template

- **WHEN** init writes the goal bundle SKILL.md
- **THEN** the workflow body's step 5 SHALL describe the desired stdout summary's shape (count of created vs modified pages, language matching the goal text) without including any literal sample summary phrase that the agent could copy verbatim into stdout
- **AND** the step 5 instruction SHALL reference the cwd `CLAUDE.md` Language Policy as the source of truth for the output language
- **AND** the step 5 instruction SHALL include an explicit directive that the agent MUST NOT copy phrasing from this SKILL.md verbatim into the stdout summary


<!-- @trace
source: v3-query
updated: 2026-05-09
code:
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/query_flow.rs
-->

---
### Requirement: Query Bundle Workflow Content

The `codebus-query/SKILL.md` file written by init SHALL contain a `## Workflow` section (or equivalently-named heading containing the substring `Workflow`) that documents a four-step read-only lookup procedure for the query verb. The four steps SHALL be presented in order and SHALL describe: (1) parsing the query intent and identifying which taxonomy types under `wiki/` are likely relevant; (2) globbing `wiki/` for candidate pages, reading frontmatter first as a relevance filter, and only reading body when frontmatter matches; (3) following `[[wikilink]]` references in matched pages to assemble cross-page context, with bounded depth to avoid drift; (4) emitting the answer to stdout in the same natural language as the query text per the cwd `CLAUDE.md` Language Policy, without copying phrasing from the SKILL.md verbatim.

The workflow body SHALL explicitly declare the read-only invariant: the agent MUST NOT use Write or Edit, and MUST NOT mutate any file inside `wiki/`, `raw/`, or anywhere else. The body SHALL note that the toolset is also gated at the binary layer (so a Write attempt will fail at the runtime), but the SKILL.md statement of the invariant is required for defense-in-depth.

The workflow body SHALL be written in English (no characters in the CJK Unified Ideographs block U+4E00..U+9FFF, except inside ASCII-only path or wikilink slug fragments). Step 4's instruction SHALL be abstract — describing the desired output shape rather than providing a literal sample answer phrase that the agent could copy verbatim — and SHALL reference cwd `CLAUDE.md` as the source of truth for output language.

The workflow body SHALL NOT inline schema rules (taxonomy definitions, frontmatter field formats, wikilink resolution rules); these rules belong in cwd `CLAUDE.md` only. References to `CLAUDE.md` from workflow steps are permitted.

#### Scenario: codebus-query SKILL.md contains four-step workflow markers

- **WHEN** init runs against a repository with no existing `<repo>/.codebus/.claude/skills/codebus-query/SKILL.md`
- **THEN** the resulting file SHALL contain a `## Workflow` heading AND the body under that heading SHALL contain at least four distinct numbered list items (lines beginning with `1.`, `2.`, `3.`, `4.`)

#### Scenario: codebus-query workflow declares read-only invariant

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body SHALL contain a substring stating that the agent MUST NOT use Write or Edit (case-insensitive match for the phrase `MUST NOT use Write` or equivalent canonical wording)
- **AND** the workflow body SHALL contain a substring stating that the toolset is gated at the binary layer

#### Scenario: codebus-query workflow body is written in English

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body (everything under the `## Workflow` heading through end of file) SHALL NOT contain any character in the CJK Unified Ideographs block (Unicode range U+4E00 through U+9FFF)

#### Scenario: codebus-query step 4 is abstract, not a literal output template

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body's step 4 SHALL describe the desired stdout answer's shape (language matching the query text, no literal sample) without including any literal sample answer phrase the agent could copy verbatim
- **AND** the step 4 instruction SHALL reference cwd `CLAUDE.md` as the source of truth for output language
- **AND** the step 4 instruction SHALL include an explicit directive that the agent MUST NOT copy phrasing from this SKILL.md verbatim

#### Scenario: codebus-query workflow defers schema rules to CLAUDE.md

- **WHEN** init writes the query bundle SKILL.md
- **THEN** the workflow body SHALL contain the substring `CLAUDE.md` (the schema reference)
- **AND** the workflow body SHALL NOT contain inline taxonomy definitions enumerating concepts, entities, modules, processes, and synthesis as the five page types in a single sentence

<!-- @trace
source: v3-query
updated: 2026-05-09
code:
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/query_flow.rs
-->