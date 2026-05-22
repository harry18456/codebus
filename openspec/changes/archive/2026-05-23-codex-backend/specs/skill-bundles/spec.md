## ADDED Requirements

### Requirement: Codex Instruction Materialization

When the codex provider is in use, the vault SHALL materialize codex's instruction surface alongside the existing Claude bundles, using the channels verified by the 2026-05-22 spike. The system SHALL write a codex skill bundle for each verb to `<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md` using the identical SKILL.md frontmatter-plus-body format as the `.claude` bundles (codex registers a project-level `.codex/skills/` entry's name and description even under the isolation spawn flags, so the bundle content is reused verbatim). The system SHALL also generate `<vault>/.codebus/AGENTS.md` whose content mirrors the taxonomy, frontmatter rules, and language policy of `<vault>/.codebus/CLAUDE.md` — `AGENTS.md` is codex's always-loaded instruction file (it is loaded regardless of project trust state) and is the authoritative channel codex reads from the vault working directory.

The system SHALL materialize a vault-unique marker file under `<vault>/.codebus/` that the codex backend names in its `project_root_markers` override, so codex pins its project root to the vault and excludes any `.codex/` directory or `AGENTS.md` in the analyzed repository above the vault. Materialization of `AGENTS.md`, the codex skill bundles, and the marker SHALL follow the same write-if-missing rule as the Claude bundles: existing files SHALL NOT be overwritten, preserving user customizations. The existing `.codebus/.claude/skills/` and repo-root `.claude/skills/` materialization SHALL remain unchanged (codex support is purely additive).

#### Scenario: Codex skill bundles written under the vault .codex directory

- **WHEN** the vault is materialized for codex and `<vault>/.codebus/.codex/skills/codebus-goal/SKILL.md` is absent
- **THEN** the system SHALL write that SKILL.md with the same frontmatter-and-body format as the corresponding `.claude` bundle

#### Scenario: AGENTS.md generated mirroring CLAUDE.md

- **WHEN** the vault is materialized for codex and `<vault>/.codebus/AGENTS.md` is absent
- **THEN** the system SHALL generate `AGENTS.md` carrying the taxonomy, frontmatter rules, and language policy that `<vault>/.codebus/CLAUDE.md` defines

#### Scenario: Vault marker present for project-root pinning

- **WHEN** the vault is materialized for codex
- **THEN** a vault-unique marker file SHALL exist under `<vault>/.codebus/` so the codex backend's `project_root_markers` override pins the codex project root to the vault and excludes the analyzed repository's own `.codex/` and `AGENTS.md`

#### Scenario: Existing files are preserved

- **WHEN** `<vault>/.codebus/AGENTS.md` or a codex SKILL.md already exists
- **THEN** the system SHALL NOT overwrite it, preserving any user customization

#### Scenario: Claude bundles unchanged

- **WHEN** the vault is materialized
- **THEN** the existing `.codebus/.claude/skills/` and repo-root `.claude/skills/` bundles SHALL be materialized exactly as before (codex materialization is additive and SHALL NOT alter the Claude paths)
