## MODIFIED Requirements

### Requirement: Skill Bundle Layout

The system SHALL create four skill bundles at TWO locations for each verb:

- **Vault-internal location** at `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix,chat}/` — discovered when an agentic CLI runs with cwd at the vault root (`<repo>/.codebus/`). Used by the `codebus goal`, `codebus query`, `codebus fix`, and `codebus chat` subcommands when they spawn agents.
- **Repo-root location** at `<repo>/.claude/skills/codebus-{goal,query,fix,chat}/` — discovered when a user opens a Claude Code session with cwd at the source repository root and invokes `/codebus-goal`, `/codebus-query`, `/codebus-fix`, or `/codebus-chat` interactively.

Each bundle directory at each location SHALL contain at minimum a `SKILL.md` file at its root. The SKILL.md content SHALL be byte-identical between the vault-internal and repo-root copies for each verb.

The system SHALL NOT create a `codebus-lint` skill bundle at either location (lint is a direct CLI subcommand and does not require an agentic skill). The system SHALL NOT write skill bundles into `~/.claude/skills/codebus-*/` (user-global location) — bundles remain per-repository to avoid cross-vault version conflicts.

The repo-root skill bundle directories SHALL be added to the source repository's `.gitignore` file by the init source-gitignore mutation step, so the bundles are not accidentally committed to the source repository's history.

#### Scenario: Init creates skill bundle directories at both vault and repo-root locations

- **WHEN** init runs against `<repo>` with no existing skill bundles at either location
- **THEN** the system SHALL create `<repo>/.codebus/.claude/skills/codebus-goal/`, `<repo>/.codebus/.claude/skills/codebus-query/`, `<repo>/.codebus/.claude/skills/codebus-fix/`, `<repo>/.codebus/.claude/skills/codebus-chat/`, `<repo>/.claude/skills/codebus-goal/`, `<repo>/.claude/skills/codebus-query/`, `<repo>/.claude/skills/codebus-fix/`, AND `<repo>/.claude/skills/codebus-chat/` AND each SHALL contain a `SKILL.md` file

#### Scenario: Vault and repo-root SKILL.md content are byte-identical

- **WHEN** init runs against `<repo>` and writes both the vault-internal and repo-root copies of the SKILL.md for any of the four verbs
- **THEN** for each verb, the bytes of `<repo>/.codebus/.claude/skills/codebus-{verb}/SKILL.md` SHALL equal the bytes of `<repo>/.claude/skills/codebus-{verb}/SKILL.md`

#### Scenario: Init does not create codebus-lint bundle at either location

- **WHEN** init runs against `<repo>`
- **THEN** the system SHALL NOT create `<repo>/.codebus/.claude/skills/codebus-lint/` AND SHALL NOT create `<repo>/.claude/skills/codebus-lint/`

#### Scenario: Init does not write to user-global skills directory

- **WHEN** init runs against `<repo>`
- **THEN** the system SHALL NOT create or modify any path under `~/.claude/skills/codebus-*/`

#### Scenario: Init adds repo-root skill bundle directories to source gitignore

- **WHEN** init runs against `<repo>` and reaches the source-gitignore mutation step
- **THEN** the source repository's `.gitignore` SHALL include patterns that exclude `<repo>/.claude/skills/codebus-goal/`, `<repo>/.claude/skills/codebus-query/`, `<repo>/.claude/skills/codebus-fix/`, AND `<repo>/.claude/skills/codebus-chat/` from source version control

#### Scenario: Skill bundle directory creation handles missing parents

- **WHEN** init runs against `<repo>` whose `.codebus/.claude/` and `<repo>/.claude/` parent chains do not yet exist
- **THEN** the system SHALL create both parent chains as needed before writing the SKILL.md files

#### Scenario: Codebus-chat bundle materialized at both locations

- **WHEN** init runs against `<repo>` with no existing `codebus-chat` skill bundle at either location
- **THEN** the system SHALL create `<repo>/.codebus/.claude/skills/codebus-chat/SKILL.md` AND `<repo>/.claude/skills/codebus-chat/SKILL.md` with byte-identical content AND the content SHALL satisfy the `Chat Skill Bundle Content` requirement defined in the `chat-verb` capability
