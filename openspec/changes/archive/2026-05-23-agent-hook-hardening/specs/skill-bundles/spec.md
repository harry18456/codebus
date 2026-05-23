## MODIFIED Requirements

### Requirement: Codex Instruction Materialization

When the codex provider is in use, the vault SHALL materialize codex's instruction surface alongside the existing Claude bundles, using the channels verified by the 2026-05-22 spike. The system SHALL write a codex skill bundle for each verb to `<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md` using the identical SKILL.md frontmatter-plus-body format as the `.claude` bundles (codex registers a project-level `.codex/skills/` entry's name and description even under the isolation spawn flags, so the bundle content is reused verbatim). The system SHALL also generate `<vault>/.codebus/AGENTS.md` whose content mirrors the taxonomy, frontmatter rules, AND language policy of `<vault>/.codebus/CLAUDE.md` — `AGENTS.md` is codex's always-loaded instruction file (it is loaded regardless of project trust state) AND is the authoritative channel codex reads from the vault working directory.

The generated `<vault>/.codebus/AGENTS.md` body SHALL include a sensitive-read soft-constraint paragraph that names the user-home paths `~/.ssh/`, `~/.aws/`, AND `~/.gnupg/`, AND instructs the codex agent to not proactively read those paths or any user-home files outside the vault working directory. The paragraph SHALL acknowledge that codex's `workspace-write` sandbox permits reading outside the workspace by design (this fact SHALL be stated in the paragraph itself, not implicit), AND SHALL state that codebus agents are scoped to the vault. The paragraph is informational guidance (model self-discipline), NOT a hard enforcement — the `lint-feedback-loop` capability's PII Image Read Hook Installation requirement provides hard enforcement on the Claude path via the `codebus hook check-read` subcommand; codex's hard read enforcement is explicitly out of scope for this requirement AND is tracked separately in the project backlog.

The system SHALL materialize a vault-unique marker file under `<vault>/.codebus/` that the codex backend names in its `project_root_markers` override, so codex pins its project root to the vault AND excludes any `.codex/` directory or `AGENTS.md` in the analyzed repository above the vault. Materialization of `AGENTS.md`, the codex skill bundles, AND the marker SHALL follow the same write-if-missing rule as the Claude bundles: existing files SHALL NOT be overwritten, preserving user customizations. The existing `.codebus/.claude/skills/` AND repo-root `.claude/skills/` materialization SHALL remain unchanged (codex support is purely additive).

#### Scenario: Codex skill bundles written under the vault .codex directory

- **WHEN** the vault is materialized for codex AND `<vault>/.codebus/.codex/skills/codebus-goal/SKILL.md` is absent
- **THEN** the system SHALL write that SKILL.md with the same frontmatter-and-body format as the corresponding `.claude` bundle

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

#### Scenario: Claude bundles unchanged

- **WHEN** the vault is materialized
- **THEN** the existing `.codebus/.claude/skills/` AND repo-root `.claude/skills/` bundles SHALL be materialized exactly as before (codex materialization is additive AND SHALL NOT alter the Claude paths)
