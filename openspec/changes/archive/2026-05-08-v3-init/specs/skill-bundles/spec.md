## ADDED Requirements

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
