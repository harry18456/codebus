# skill-bundles Specification Delta — v3-lint

## MODIFIED Requirements

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
