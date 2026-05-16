## MODIFIED Requirements

### Requirement: Skill Bundle Layout

The system SHALL create five skill bundles for each verb at the **vault-internal location** under `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix,chat,quiz}/` by default. This location is discovered when the agentic CLI runs with cwd at the vault root (`<repo>/.codebus/`) — used by the `codebus goal`, `codebus query`, `codebus fix`, `codebus chat`, and `codebus quiz` subcommands and by the `codebus-app` GUI when it spawns agents through `codebus_core::verb::*::run_*`.

The system SHALL ALSO create the same five skill bundles at the **repo-root location** under `<repo>/.claude/skills/codebus-{goal,query,fix,chat,quiz}/` ONLY WHEN the caller explicitly requests it (via `codebus init --with-repo-root-skills`, or programmatically by passing `with_repo_root_skills: true` to `vault::init::run_init`'s `InitOptions`). The repo-root location is discovered when a user opens a Claude Code session with cwd at the source repository root and invokes `/codebus-goal`, `/codebus-query`, `/codebus-fix`, `/codebus-chat`, or `/codebus-quiz` interactively — a power-user workflow distinct from the default GUI / CLI spawn path.

Each bundle directory at each written location SHALL contain at minimum a `SKILL.md` file at its root. When both locations are written in the same init invocation, the SKILL.md content SHALL be byte-identical between the vault-internal and repo-root copies for each verb (the write helper produces the bytes once and writes the same buffer to both targets). When only the vault-internal location is written (the default), no byte-identity claim applies.

The system SHALL NOT create a `codebus-lint` skill bundle at either location (lint is a direct CLI subcommand and does not require an agentic skill). The system SHALL NOT write skill bundles into `~/.claude/skills/codebus-*/` (user-global location) — bundles remain per-repository to avoid cross-vault version conflicts.

The source repository's `.gitignore` mutation step SHALL add `.claude/skills/codebus-*/` exclusion patterns ONLY WHEN repo-root skill bundles are written in that init invocation. When the default vault-only path runs, the mutation step SHALL NOT add those patterns.

#### Scenario: Init default creates only vault-internal skill bundles

- **WHEN** init runs against `<repo>` with no existing skill bundles AND the caller does NOT request repo-root skills (default behavior; e.g., plain `codebus init <path>` or `codebus-app` add-vault flow)
- **THEN** the system SHALL create `<repo>/.codebus/.claude/skills/codebus-goal/SKILL.md`, `<repo>/.codebus/.claude/skills/codebus-query/SKILL.md`, `<repo>/.codebus/.claude/skills/codebus-fix/SKILL.md`, `<repo>/.codebus/.claude/skills/codebus-chat/SKILL.md`, AND `<repo>/.codebus/.claude/skills/codebus-quiz/SKILL.md` AND SHALL NOT create any path under `<repo>/.claude/skills/codebus-*/`

#### Scenario: Init with --with-repo-root-skills creates both locations

- **WHEN** init runs against `<repo>` with no existing skill bundles AND the caller requests repo-root skills (e.g., `codebus init <path> --with-repo-root-skills`)
- **THEN** the system SHALL create the five vault-internal SKILL.md files as in the default case AND SHALL ALSO create `<repo>/.claude/skills/codebus-goal/SKILL.md`, `<repo>/.claude/skills/codebus-query/SKILL.md`, `<repo>/.claude/skills/codebus-fix/SKILL.md`, `<repo>/.claude/skills/codebus-chat/SKILL.md`, AND `<repo>/.claude/skills/codebus-quiz/SKILL.md`

#### Scenario: Vault and repo-root SKILL.md content are byte-identical when both are written

- **WHEN** init runs against `<repo>` with the repo-root-skills opt-in AND writes both the vault-internal and repo-root copies of the SKILL.md for any of the five verbs
- **THEN** for each verb, the bytes of `<repo>/.codebus/.claude/skills/codebus-{verb}/SKILL.md` SHALL equal the bytes of `<repo>/.claude/skills/codebus-{verb}/SKILL.md`

#### Scenario: Init does not create codebus-lint bundle at either location

- **WHEN** init runs against `<repo>` (with or without the repo-root-skills opt-in)
- **THEN** the system SHALL NOT create `<repo>/.codebus/.claude/skills/codebus-lint/` AND SHALL NOT create `<repo>/.claude/skills/codebus-lint/`

#### Scenario: Init does not write to user-global skills directory

- **WHEN** init runs against `<repo>` (with or without the repo-root-skills opt-in)
- **THEN** the system SHALL NOT create or modify any path under `~/.claude/skills/codebus-*/`

## ADDED Requirements

### Requirement: Quiz Skill Bundle Content

The `codebus-quiz` SKILL.md SHALL declare a read scope of `wiki/` only and SHALL forbid reading `raw/`, `log/`, and any path escaping the vault root. It SHALL define two prompt modes selected by the prompt prefix: `plan:` (emit `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` as the first line, then stop) and `generate:` (emit the quiz markdown body). It SHALL require the `[CODEBUS_QUIZ_VIOLATION] <path>` marker when forced toward `raw/`. It SHALL forbid the agent from authoring `quiz_id`, `topic`, or `generation_token_usage`, and forbid wrapping the whole output in a code fence. Markers and structural tokens SHALL always be English; question stems, choices, and explanations SHALL follow the language of the quizzed wiki pages (Language Override).

#### Scenario: Quiz bundle declares wiki-only read scope

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL state that read scope is `wiki/` only AND SHALL explicitly forbid reading `raw/`

#### Scenario: Quiz bundle defines plan and generate modes

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** it SHALL define the `plan:` mode emitting `[CODEBUS_QUIZ_SCOPE]`/`[CODEBUS_QUIZ_NO_MATCH]` and the `generate:` mode emitting the question body without agent-authored frontmatter
