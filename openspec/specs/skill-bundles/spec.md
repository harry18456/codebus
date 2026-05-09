# skill-bundles Specification

## Purpose

The materialization of three Claude Code skill bundles (`codebus-goal`, `codebus-query`, `codebus-fix`) under both `<vault>/.codebus/.claude/skills/` (CLI-spawn-discovery) and `<repo>/.claude/skills/` (user-direct-discovery) — directory layout, SKILL.md frontmatter format, hard-scope and path-translation rule bodies, write-if-missing preservation of user customizations, and per-verb workflow content. Does NOT cover the agent sandbox flags passed at spawn time (those live in `cli`'s per-verb Subcommand Behavior requirements), the PreToolUse Bash hook configuration (lives in `lint-feedback-loop` Fix Bash Hook Installation), or the source-repo `.gitignore` line for the bundle directories (lives in `vault` Source Repo `.gitignore` Mutation).

## Requirements

### Requirement: Skill Bundle Layout

The system SHALL create three skill bundles at TWO locations for each verb:

- **Vault-internal location** at `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix}/` — discovered when an agentic CLI runs with cwd at the vault root (`<repo>/.codebus/`). Used by the `codebus goal`, `codebus query`, and `codebus fix` subcommands when they spawn agents.
- **Repo-root location** at `<repo>/.claude/skills/codebus-{goal,query,fix}/` — discovered when a user opens a Claude Code session with cwd at the source repository root and invokes `/codebus-goal`, `/codebus-query`, or `/codebus-fix` interactively.

Each bundle directory at each location SHALL contain at minimum a `SKILL.md` file at its root. The SKILL.md content SHALL be byte-identical between the vault-internal and repo-root copies for each verb.

The system SHALL NOT create a `codebus-lint` skill bundle at either location (lint is a direct CLI subcommand and does not require an agentic skill). The system SHALL NOT write skill bundles into `~/.claude/skills/codebus-*/` (user-global location) — bundles remain per-repository to avoid cross-vault version conflicts.

The repo-root skill bundle directories SHALL be added to the source repository's `.gitignore` file by the init source-gitignore mutation step, so the bundles are not accidentally committed to the source repository's history.

#### Scenario: Init creates skill bundle directories at both vault and repo-root locations

- **WHEN** init runs against `<repo>` with no existing skill bundles at either location
- **THEN** the system SHALL create `<repo>/.codebus/.claude/skills/codebus-goal/`, `<repo>/.codebus/.claude/skills/codebus-query/`, `<repo>/.codebus/.claude/skills/codebus-fix/`, `<repo>/.claude/skills/codebus-goal/`, `<repo>/.claude/skills/codebus-query/`, AND `<repo>/.claude/skills/codebus-fix/` AND each SHALL contain a `SKILL.md` file

#### Scenario: Vault and repo-root SKILL.md content are byte-identical

- **WHEN** init runs against `<repo>` and writes both the vault-internal and repo-root copies of the SKILL.md for any of the three verbs
- **THEN** for each verb, the bytes of `<repo>/.codebus/.claude/skills/codebus-{verb}/SKILL.md` SHALL equal the bytes of `<repo>/.claude/skills/codebus-{verb}/SKILL.md`

#### Scenario: Init does not create codebus-lint bundle at either location

- **WHEN** init runs against `<repo>`
- **THEN** the system SHALL NOT create `<repo>/.codebus/.claude/skills/codebus-lint/` AND SHALL NOT create `<repo>/.claude/skills/codebus-lint/`

#### Scenario: Init does not write to user-global skills directory

- **WHEN** init runs against `<repo>`
- **THEN** the system SHALL NOT create or modify any path under `~/.claude/skills/codebus-*/`

#### Scenario: Init adds repo-root skill bundle directories to source gitignore

- **WHEN** init runs against `<repo>` and reaches the source-gitignore mutation step
- **THEN** the source repository's `.gitignore` SHALL include patterns that exclude `<repo>/.claude/skills/codebus-goal/`, `<repo>/.claude/skills/codebus-query/`, AND `<repo>/.claude/skills/codebus-fix/` from source version control

#### Scenario: Skill bundle directory creation handles missing parents

- **WHEN** init runs against `<repo>` whose `.codebus/.claude/` and `<repo>/.claude/` parent chains do not yet exist
- **THEN** the system SHALL create both parent chains as needed before writing the SKILL.md files


<!-- @trace
source: v3-lint
updated: 2026-05-09
code:
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/mod.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/wiki/fix/session.rs
  - codebus-core/src/wiki/lint/locate.rs
  - codebus-core/src/wiki/lint/rules/root_page.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/wiki/lint/rules/missing_nav.rs
  - codebus-core/src/wiki/lint/rules/broken_wikilink.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/wiki/mod.rs
  - codebus-core/src/wiki/fix/prompt.rs
  - codebus-core/src/wiki/lint/rules/duplicate_slug.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/lint/rules/mod.rs
  - codebus-core/src/wiki/lint/output.rs
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/wiki/lint/rules/frontmatter_integrity.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/wiki/lint/factory.rs
  - codebus-core/src/wiki/lint/mod.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/wiki/frontmatter.rs
  - codebus-core/src/wiki/lint/rule.rs
  - codebus-core/src/wiki/types.rs
tests:
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/cli_routing.rs
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

For each skill bundle SKILL.md target path — at both the vault-internal location (`<repo>/.codebus/.claude/skills/codebus-{verb}/SKILL.md`) and the repo-root location (`<repo>/.claude/skills/codebus-{verb}/SKILL.md`) — the system SHALL write the bundle content ONLY when the file at that specific location does not exist. When the file already exists at a given location, the system SHALL NOT modify it (preserving any user customization).

The two locations SHALL be evaluated independently — if the vault-internal copy exists but the repo-root copy is missing, the system SHALL write only the missing repo-root copy. The system SHALL NOT propagate content from one location to the other when the target already exists.

#### Scenario: Write-if-missing skips existing file at vault location

- **WHEN** init runs against `<repo>` and the vault-internal SKILL.md for some verb already exists with custom content
- **THEN** the system SHALL NOT modify the existing vault-internal SKILL.md AND its content SHALL be byte-identical before and after init

#### Scenario: Write-if-missing skips existing file at repo-root location

- **WHEN** init runs against `<repo>` and the repo-root SKILL.md for some verb already exists with custom content
- **THEN** the system SHALL NOT modify the existing repo-root SKILL.md AND its content SHALL be byte-identical before and after init

#### Scenario: Write-if-missing fills only missing locations

- **WHEN** init runs against `<repo>` where the vault-internal codebus-goal SKILL.md exists but the repo-root codebus-goal SKILL.md does not exist
- **THEN** the system SHALL create only the repo-root codebus-goal SKILL.md AND SHALL NOT modify the existing vault-internal one


<!-- @trace
source: v3-lint
updated: 2026-05-09
code:
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/mod.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/wiki/fix/session.rs
  - codebus-core/src/wiki/lint/locate.rs
  - codebus-core/src/wiki/lint/rules/root_page.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/wiki/lint/rules/missing_nav.rs
  - codebus-core/src/wiki/lint/rules/broken_wikilink.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/wiki/mod.rs
  - codebus-core/src/wiki/fix/prompt.rs
  - codebus-core/src/wiki/lint/rules/duplicate_slug.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/lint/rules/mod.rs
  - codebus-core/src/wiki/lint/output.rs
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/wiki/lint/rules/frontmatter_integrity.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/wiki/lint/factory.rs
  - codebus-core/src/wiki/lint/mod.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/wiki/frontmatter.rs
  - codebus-core/src/wiki/lint/rule.rs
  - codebus-core/src/wiki/types.rs
tests:
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/cli_routing.rs
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