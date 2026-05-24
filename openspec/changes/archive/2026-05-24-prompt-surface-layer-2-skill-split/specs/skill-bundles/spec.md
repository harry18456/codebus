## MODIFIED Requirements

### Requirement: Codex Instruction Materialization

When the codex provider is in use, the vault SHALL materialize codex's instruction surface alongside the existing Claude bundles, using the channels verified by the 2026-05-22 spike. The system SHALL write a codex skill bundle for each verb to `<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md` using the same SKILL.md frontmatter-plus-body format as the `.claude` bundles (codex registers a project-level `.codex/skills/` entry's name and description even under the isolation spawn flags). The body content SHALL be **provider-aware**: the system SHALL produce a body whose mechanism descriptions, trigger guidance, schema-document filename references, and shell-form examples accurately reflect the codex provider's runtime — not the Claude provider's. The system SHALL also generate `<vault>/.codebus/AGENTS.md` whose content mirrors the taxonomy, frontmatter rules, AND language policy of `<vault>/.codebus/CLAUDE.md` — `AGENTS.md` is codex's always-loaded instruction file (it is loaded regardless of project trust state) AND is the authoritative channel codex reads from the vault working directory.

The generated `<vault>/.codebus/AGENTS.md` body SHALL include a sensitive-read soft-constraint paragraph that names the user-home paths `~/.ssh/`, `~/.aws/`, AND `~/.gnupg/`, AND instructs the codex agent to not proactively read those paths or any user-home files outside the vault working directory. The paragraph SHALL acknowledge that codex's `workspace-write` sandbox permits reading outside the workspace by design (this fact SHALL be stated in the paragraph itself, not implicit), AND SHALL state that codebus agents are scoped to the vault. The paragraph is informational guidance (model self-discipline), NOT a hard enforcement — the `lint-feedback-loop` capability's PII Image Read Hook Installation requirement provides hard enforcement on the Claude path via the `codebus hook check-read` subcommand; codex's hard read enforcement is explicitly out of scope for this requirement AND is tracked separately in the project backlog.

The system SHALL materialize a vault-unique marker file under `<vault>/.codebus/` that the codex backend names in its `project_root_markers` override, so codex pins its project root to the vault AND excludes any `.codex/` directory or `AGENTS.md` in the analyzed repository above the vault. Materialization of `AGENTS.md`, the codex skill bundles, AND the marker SHALL follow the same write-if-missing rule as the Claude bundles: existing files SHALL NOT be overwritten, preserving user customizations. The existing `.codebus/.claude/skills/` AND repo-root `.claude/skills/` materialization SHALL remain unchanged in path layout (codex support is additive on the path layout dimension) — body content for the Claude paths SHALL continue to use the Claude-provider-aware body.

**Provider-aware body divergence rules:**

- Claude-path SKILL bodies (`<vault>/.codebus/.claude/skills/codebus-{verb}/SKILL.md` AND `<repo>/.claude/skills/codebus-{verb}/SKILL.md` when present) SHALL reference Claude-specific runtime mechanisms (`PreToolUse` Bash hook, `--tools` flag, `Read` hook, `mcp_*` tool naming family) where appropriate, AND SHALL reference `CLAUDE.md` as the schema document filename in the vault working directory.
- Codex-path SKILL bodies (`<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md`) SHALL NOT reference Claude-specific runtime mechanisms above, SHALL reference `AGENTS.md` as the schema document filename in the vault working directory, AND SHALL describe enforcement in terms of the codex `workspace-write` sandbox AND the AGENTS.md scope-enforcement paragraph (rather than tool-gating hooks).
- For the quiz verb's Mode B (self-validate via `codebus quiz validate ...`), the claude-path body SHALL describe the bash heredoc invocation gated by the `PreToolUse` hook. The codex-path body SHALL NOT describe a runnable Mode B invocation — instead, it SHALL instruct the agent to emit a single line containing `[CODEBUS_QUIZ_NO_VALIDATE] <short reason>` AND skip the self-validation step, because codex's sandbox `-s` levels lack the per-command allowance needed to safely run a single `codebus quiz validate` invocation. (Codex per-command allowance remains an open architectural question tracked in the project backlog.)
- The verb-level taxonomy enumeration (the five `wiki/<type>/` folder names) SHALL NOT be duplicated inside the SKILL workflow body of any verb on either provider path; the SKILL body SHALL reference the schema document (`CLAUDE.md` or `AGENTS.md` per provider) §2 for the canonical taxonomy list.
- The "trigger" language at the top of every verb's SKILL body SHALL describe activation in semantic terms (e.g., "Activate when the user requests <verb action>") AND SHALL NOT name a provider-specific invocation syntax (no `/codebus-<verb>` literal on the claude path, no `$codebus-<verb>` literal on the codex path). Spawn-time invocation syntax is owned by the backend layer, not by the SKILL body.

#### Scenario: Codex skill bundles written under the vault .codex directory

- **WHEN** the vault is materialized for codex AND `<vault>/.codebus/.codex/skills/codebus-goal/SKILL.md` is absent
- **THEN** the system SHALL write that SKILL.md with the same frontmatter-and-body shape (frontmatter block followed by a body section) as the corresponding `.claude` bundle, BUT the body content SHALL be the codex-provider-aware variant — not byte-identical to the `.claude` body

#### Scenario: AGENTS.md generated mirroring CLAUDE.md

- **WHEN** the vault is materialized for codex AND `<vault>/.codebus/AGENTS.md` is absent
- **THEN** the system SHALL generate `AGENTS.md` carrying the taxonomy, frontmatter rules, AND language policy that `<vault>/.codebus/CLAUDE.md` defines

#### Scenario: AGENTS.md contains sensitive-read soft constraint paragraph

- **WHEN** the vault is materialized for codex AND `<vault>/.codebus/AGENTS.md` is absent
- **THEN** the generated `AGENTS.md` body SHALL contain a paragraph that (a) names the literal paths `~/.ssh/`, `~/.aws/`, AND `~/.gnupg/`, (b) acknowledges that codex's workspace-write sandbox permits reading outside the workspace by design, AND (c) instructs the codex agent to stay within vault scope AND not proactively read user-home sensitive files

#### Scenario: Vault marker present for project-root pinning

- **WHEN** the vault is materialized for codex
- **THEN** a vault-unique marker file SHALL exist under `<vault>/.codebus/` so the codex backend's `project_root_markers` override pins the codex project root to the vault AND excludes the analyzed repository's own `.codex/` AND `AGENTS.md`

#### Scenario: Existing files are preserved

- **WHEN** `<vault>/.codebus/AGENTS.md` or a codex SKILL.md already exists
- **THEN** the system SHALL NOT overwrite it, preserving any user customization (the soft-constraint paragraph SHALL only be introduced on fresh materialization)

#### Scenario: Claude bundles path layout unchanged

- **WHEN** the vault is materialized
- **THEN** the existing `.codebus/.claude/skills/` AND repo-root `.claude/skills/` bundle paths SHALL be materialized exactly as before (codex materialization is additive on the path layout dimension AND SHALL NOT alter the Claude paths)

#### Scenario: Claude SKILL body references Claude-specific mechanisms; codex body does not

- **WHEN** the vault is materialized for codex AND both `<vault>/.codebus/.claude/skills/codebus-fix/SKILL.md` AND `<vault>/.codebus/.codex/skills/codebus-fix/SKILL.md` are written
- **THEN** the claude-path SKILL body SHALL contain at least one of the Claude-specific mechanism phrases (`PreToolUse`, `--tools Read,Glob,Grep`, `Read hook`, `mcp_`) AND SHALL reference `CLAUDE.md` as the cwd schema document
- **AND** the codex-path SKILL body SHALL NOT contain any of those Claude-specific mechanism phrases AND SHALL reference `AGENTS.md` as the cwd schema document

##### Example: fix verb body divergence

- **GIVEN** the same `codebus init` run materializes both `<vault>/.codebus/.claude/skills/codebus-fix/SKILL.md` and `<vault>/.codebus/.codex/skills/codebus-fix/SKILL.md`
- **WHEN** the two body texts are diffed
- **THEN** the claude body's Read-Only Invariant section SHALL describe enforcement via `--tools Read,Glob,Grep` flag gating; the codex body's Read-Only Invariant section SHALL describe enforcement via the codex `workspace-write` sandbox AND the AGENTS.md scope paragraph (no `--tools` literal)
- **AND** the claude body's Step 1 SHALL describe the `PreToolUse` Bash hook allowing `codebus lint` invocations; the codex body's Step 1 SHALL describe sandbox-level read-only posture for the same step (no `PreToolUse` literal)

#### Scenario: Codex quiz Mode B emits no-validate marker instead of running validate

- **WHEN** the vault is materialized for codex AND `<vault>/.codebus/.codex/skills/codebus-quiz/SKILL.md` is written
- **THEN** the codex-path quiz SKILL body SHALL NOT contain a bash heredoc invocation of `codebus quiz validate`
- **AND** the codex-path quiz SKILL body SHALL contain the literal marker `[CODEBUS_QUIZ_NO_VALIDATE]` AND SHALL instruct the agent to emit one line of that marker followed by a short reason AND skip Mode B self-validation
- **AND** the claude-path quiz SKILL body SHALL continue to contain a bash heredoc invocation of `codebus quiz validate` gated by the `PreToolUse` hook

#### Scenario: Taxonomy enumeration not duplicated in either provider's SKILL body

- **WHEN** the vault is materialized for either provider
- **THEN** no verb's SKILL body (claude or codex path) SHALL contain the literal five taxonomy folder enumeration `concepts / entities / modules / processes / synthesis` (or the same five names separated by `/`, ` / `, or `,`); each SKILL body SHALL instead reference the schema document `§2 Wiki Structure` for the canonical taxonomy list

#### Scenario: Trigger language is semantic and provider-agnostic on both paths

- **WHEN** the vault is materialized for either provider
- **THEN** the top "trigger" sentence of every verb's SKILL body SHALL describe activation in semantic terms ("Activate when the user requests <verb action>" or equivalent) AND SHALL NOT contain the literal token `/codebus-<verb>` or `$codebus-<verb>` (where `<verb>` is the verb name); the literal invocation syntax SHALL be owned by the backend layer (Claude backend / codex backend), not by the SKILL body
