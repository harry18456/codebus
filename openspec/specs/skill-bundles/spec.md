# skill-bundles Specification

## Purpose

The materialization of five Claude Code skill bundles (`codebus-goal`, `codebus-query`, `codebus-fix`, `codebus-chat`, `codebus-quiz`) under both `<vault>/.codebus/.claude/skills/` (CLI-spawn-discovery) and `<repo>/.claude/skills/` (user-direct-discovery), plus the codex provider's parallel materialization under `<vault>/.codebus/.codex/skills/` + `<vault>/.codebus/AGENTS.md` — directory layout, SKILL.md frontmatter format, hard-scope and path-translation rule bodies, write-if-missing preservation of user customizations, and per-verb workflow content. Does NOT cover the agent sandbox flags passed at spawn time (those live in `cli`'s per-verb Subcommand Behavior requirements), the PreToolUse Bash hook configuration (lives in `lint-feedback-loop` Fix Bash Hook Installation), or the source-repo `.gitignore` line for the bundle directories (lives in `vault` Source Repo `.gitignore` Mutation).

## Requirements

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


<!-- @trace
source: v3-app-quiz
updated: 2026-05-16
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/spike-artifacts/quiz-fixture-vault/manifest.yaml
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/jwt-token-lifecycle.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/index.md
  - docs/spike-artifacts/spike-quiz-7-F5.jsonl
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/config/quiz.rs
  - docs/spike-artifacts/spike-quiz-7-F1.jsonl
  - codebus-app/src-tauri/src/ipc/config.rs
  - docs/2026-05-15-v3-app-quiz-spike-plan.md
  - docs/spike-artifacts/spike-quiz-7-F6.jsonl
  - docs/spike-artifacts/spike-quiz-8-E3.jsonl
  - docs/spike-artifacts/spike-quiz-9-S1.jsonl
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-cli/src/commands/mod.rs
  - codebus-core/src/config/claude_code.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run2.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC1.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/user-store.md
  - docs/spike-artifacts/spike-quiz-10-R1-run1.jsonl
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-core/src/skill_bundle/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/wiki/log.md
  - docs/spike-artifacts/spike-quiz-7-F2.jsonl
  - docs/spike-artifacts/spike-quiz-8-E4.jsonl
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - docs/2026-05-15-v3-app-quiz-discussion.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/session-vs-token.md
  - docs/spike-artifacts/spike-quiz-8-E5.jsonl
  - codebus-cli/src/commands/quiz.rs
  - docs/spike-artifacts/spike-quiz-9-S3.jsonl
  - codebus-core/src/config/mod.rs
  - codebus-core/src/log/events/sink.rs
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - docs/spike-artifacts/spike-quiz-runbook.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/verb/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/CLAUDE.md
  - codebus-core/src/verb/event.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - docs/spike-artifacts/spike-quiz-8-E2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/raw/code/auth.py
  - docs/spike-artifacts/spike-quiz-8-E1.jsonl
  - docs/spike-artifacts/spike-quiz-7-F3.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/auth-middleware.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/processes/login-flow.md
  - docs/spike-artifacts/spike-quiz-9-S2.jsonl
  - codebus-core/src/vault/source_gitignore.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run3.jsonl
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/spike-artifacts/spike-quiz-7-F4.jsonl
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-core/tests/verb_library_surface.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/workspace/QuizAnswering.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
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

---
### Requirement: Quiz Skill Bundle Content

The `codebus-quiz` SKILL.md SHALL declare a read scope of `wiki/` only and SHALL forbid reading `raw/`, `log/`, and any path escaping the vault root. It SHALL define two prompt modes selected by the prompt prefix: `plan:` (emit `[CODEBUS_QUIZ_SCOPE]` or `[CODEBUS_QUIZ_NO_MATCH]` as the first line, then stop) and `generate:` (emit the quiz markdown body). It SHALL require the `[CODEBUS_QUIZ_VIOLATION] <path>` marker when forced toward `raw/`. It SHALL forbid the agent from authoring `quiz_id`, `topic`, or `generation_token_usage`, and forbid wrapping the whole output in a code fence. Markers and structural tokens SHALL always be English; question stems, choices, and explanations SHALL follow the language of the quizzed wiki pages (Language Override).

The `generate:` mode SHALL additionally instruct the agent to self-validate and self-repair before emitting its final body: after drafting the quiz, the agent SHALL invoke `codebus quiz validate` on its draft via its Bash tool, SHALL correct the questions reported by the findings, and SHALL re-run the validator, repeating up to a fixed internal iteration cap stated explicitly in the SKILL body; when the cap is reached the agent SHALL emit its best current body rather than looping further. The SKILL SHALL reference the validator as the authority for structural and citation correctness and SHALL NOT restate the validator rule definitions (no parallel schema copy); it SHALL describe acting on the validator findings, not the rules themselves.

The SKILL SHALL ALSO define a third prompt mode `verify:` selected by the prompt prefix. The `verify:` mode SHALL instruct the agent to read the supplied planned pages plus a generated quiz body and judge each question against exactly five content defect types — answer-wrong (marked option not supported as correct by the planned pages), out-of-scope (a claim the planned pages do not state), not-exactly-one-correct (multiple defensibly-correct options or the marked one wrong), degenerate-distractor (a non-discriminating distractor), and off-topic (not about the supplied topic; evaluated only when a topic is supplied) — and to emit, for each flagged question, its question number, the defect type, and a concrete correction suggestion. The `verify:` mode SHALL NOT restate the deterministic validator structural/citation rules, and the SKILL SHALL keep the deterministic `codebus quiz validate` structural check separate from this content judgement.

The `verify:` mode SHALL additionally contain an explicit output-termination boundary instructing the agent to STOP after the last `Q<n> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`), and to emit no further prose, rationale, evaluation summary, or per-question commentary. The SKILL body SHALL state this boundary as a normative MUST/SHALL clause, parallel in shape to the `plan:` mode boundary that forbids any content before the `[CODEBUS_QUIZ_SCOPE]` line. This requirement closes the prompt-surface-review F78 finding (an empirical 2026-05-24 run observed the verify agent emitting `**Q1 evaluation** / **Q2 evaluation** / **Q3 evaluation** / 驗證: xxx 第 N 行` rationale paragraphs before the closing `CONTENT_OK`, a contract violation that contributed to the unparseable-verify-output incident even though the line-by-line splitn parser silently skipped most prose).

#### Scenario: Quiz bundle declares wiki-only read scope

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL state that read scope is `wiki/` only AND SHALL explicitly forbid reading `raw/`

#### Scenario: Quiz bundle defines plan and generate modes

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** it SHALL define the `plan:` mode emitting `[CODEBUS_QUIZ_SCOPE]`/`[CODEBUS_QUIZ_NO_MATCH]` and the `generate:` mode emitting the question body without agent-authored frontmatter

#### Scenario: Generate mode defines a bounded self-validate/self-repair loop

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its `generate:` mode SHALL instruct the agent to invoke `codebus quiz validate` on its draft, correct reported findings, and re-validate up to a fixed internal iteration cap stated in the body AND SHALL instruct the agent to emit its best current body when the cap is reached

#### Scenario: Quiz bundle does not duplicate validator rules

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** its body SHALL reference `codebus quiz validate` as the structural/citation authority AND SHALL NOT contain a restated copy of the validator rule definitions

#### Scenario: Verify mode defines the five-item content defect contract

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** it SHALL define a `verify:` mode that judges each question against the five defect types (answer-wrong, out-of-scope, not-exactly-one-correct, degenerate-distractor, off-topic) AND instructs emitting per flagged question its number, defect type, and correction suggestion AND SHALL keep this content judgement separate from the deterministic `codebus quiz validate` structural check

#### Scenario: Verify mode declares STOP boundary after defect lines

- **WHEN** the `codebus-quiz/SKILL.md` is materialized
- **THEN** the `verify:` mode body SHALL contain a normative clause instructing the agent to STOP after the last `Q<n> | <defect-type> | <suggestion>` line or after `CONTENT_OK` AND SHALL forbid emitting any further prose, rationale, evaluation summary, or per-question commentary

##### Example: STOP boundary shape

- **GIVEN** the SKILL body for the `verify:` mode
- **WHEN** a reader looks for output-termination language
- **THEN** the body SHALL contain a sentence stating substantively that after the last `Q<n> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`) the agent SHALL stop emitting content AND SHALL NOT emit further prose, rationale, or summary

<!-- @trace
source: quiz-content-verify, prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/quiz.rs
  - codebus-cli/src/commands/quiz.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src-tauri/src/ipc/quiz.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
  - codebus-core/tests/verb_library_surface.rs
  - codebus-cli/tests/bins/mock_claude.rs
-->


<!-- @trace
source: prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - docs/2026-05-23-prompt-surface-inventory.md
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/content_verify.rs
-->

---
### Requirement: Codebus-Goal Verify Mode

The `codebus-goal` SKILL.md SHALL define a `verify:` prompt mode (selected by the prompt prefix), distinct from its normal ingest workflow, used by the independent content-verify spawn of the `verb-library` capability `Goal Content Verification and Repair` requirement. The `verify:` mode SHALL instruct the agent to read the supplied changed `wiki/` pages plus the originating goal, and — for grounding the faithfulness check — SHALL explicitly permit reading the `raw/code/` source mirror (read only, for verification; the agent SHALL NOT emit `raw/` contents, only defect judgements). It SHALL instruct judging each changed page against exactly three content defect types — **unfaithful** (a claim not grounded in / contradicting `raw/code/`), **off-goal** (content unrelated to this run goal), and **taxonomy-misplaced** (content in the wrong page type or folder) — and emitting, for each flagged page, one line `<wiki-relative-path> | <defect-type> | <concrete correction suggestion>`, or exactly `CONTENT_OK` when no page has a defect. The `verify:` mode SHALL NOT restate the deterministic lint rules; the structural lint / fix loop remains a separate concern.

The `verify:` mode SHALL additionally contain an explicit output-termination boundary instructing the agent to STOP after the last `<wiki-relative-path> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`), and to emit no further prose, rationale, evaluation summary, or per-page commentary. The SKILL body SHALL state this boundary as a normative MUST/SHALL clause. This requirement closes the prompt-surface-review F38 finding (an empirical 2026-05-24 run observed the goal verify agent emitting `已完成所有變更頁面與 raw/code/src/db.py 原始碼的比對。` prose immediately before the closing `CONTENT_OK`; the current line-by-line parser tolerated it but the contract was violated and the behavior is unpredictable across runs).

#### Scenario: Goal bundle defines the verify mode and three-item contract

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** it SHALL define a `verify:` mode that judges changed pages against the three defect types (unfaithful, off-goal, taxonomy-misplaced) AND requires per-page `path | defect-type | suggestion` output or `CONTENT_OK`

#### Scenario: Verify mode permits raw/code grounding reads

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** its `verify:` mode SHALL explicitly permit reading `raw/code/` for the faithfulness check AND SHALL forbid emitting `raw/` contents (only defect judgements)

#### Scenario: Verify mode does not duplicate lint rules

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** the `verify:` mode SHALL NOT contain a restated copy of the deterministic lint rule definitions

#### Scenario: Verify mode declares STOP boundary after defect lines

- **WHEN** the `codebus-goal/SKILL.md` is materialized
- **THEN** the `verify:` mode body SHALL contain a normative clause instructing the agent to STOP after the last `<wiki-relative-path> | <defect-type> | <suggestion>` line or after `CONTENT_OK` AND SHALL forbid emitting any further prose, rationale, evaluation summary, or per-page commentary

##### Example: STOP boundary shape

- **GIVEN** the SKILL body for the goal `verify:` mode
- **WHEN** a reader looks for output-termination language
- **THEN** the body SHALL contain a sentence stating substantively that after the last `<wiki-relative-path> | <defect-type> | <suggestion>` line (or after `CONTENT_OK`) the agent SHALL stop emitting content AND SHALL NOT emit further prose, rationale, or summary

<!-- @trace
source: goal-content-verify, prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - codebus-core/src/config/mod.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/mod.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/verb/content_verify.rs
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/skill_bundle/mod.rs
tests:
  - codebus-core/tests/verb_library_surface.rs
-->


<!-- @trace
source: prompt-surface-output-discipline-batch
updated: 2026-05-24
code:
  - docs/2026-05-23-prompt-surface-inventory.md
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/content_verify.rs
-->

---
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


<!-- @trace
source: prompt-surface-layer-2-skill-split
updated: 2026-05-24
code:
  - codebus-core/src/skill_bundle/mod.rs
-->

---
### Requirement: NEUTRAL_RULES Language Policy

The `NEUTRAL_RULES` schema document (materialized as `<vault>/CLAUDE.md` on the Claude provider path and as `<vault>/AGENTS.md` on the codex provider path) SHALL contain a `§0 Language Policy` section preceding `§1 Workspace Layout` that defines two normative rules:

1. The natural language of agent output — page bodies, stdout summary lines, and answer text — SHALL follow the prompt context language (the language of the user's goal/query/chat text), and SHALL NOT default to the language of any existing wiki page or raw source content read along the way.
2. Structural tokens and YAML keys (`type:`, `sources:`, `created:`, `updated:`, marker lines such as `[CODEBUS_*]`) SHALL always be literal English regardless of the prompt context language.

This requirement makes the contract real for the SKILL workflow body references (in `codebus-core/src/skill_bundle/mod.rs`, including goal Step 5, query Step 4, fix Step 5, and quiz mode validation paths) that cite "the §0 Language Policy in cwd CLAUDE.md" as the authority for output-language selection. Without `§0` the contract is dangling: agent behavior on multi-language prompts falls back to the underlying model's heuristic, producing inconsistent output language across providers and across model versions.

#### Scenario: Multi-language goal produces same-language summary

- **WHEN** a user runs `codebus goal "把支付模組的時序圖整理出來"` (Traditional Chinese goal text) against a vault whose existing wiki pages are written in English
- **THEN** the agent's stdout summary line and any newly written or updated wiki page body content SHALL be in Traditional Chinese
- **AND** structural frontmatter keys (`type:`, `sources:`, `created:`, `updated:`, `[CODEBUS_*]` markers) SHALL remain literal English

##### Example: Mixed-language vault, Japanese goal

- **GIVEN** a vault containing `wiki/modules/payment-gateway.md` authored in English and `wiki/concepts/checkout-flow.md` authored in Traditional Chinese
- **WHEN** a user runs `codebus goal "決済処理の主要なコンポーネントを把握したい"` (Japanese goal text)
- **THEN** the stdout summary line SHALL be in Japanese
- **AND** any new `## from goal: ... (YYYY-MM-DD)` section appended to existing pages SHALL have its body in Japanese (per Language Override) while the `## from goal:` heading literal and date stay English/numeric
- **AND** `type:`, `sources:`, `goals:`, `created:`, `updated:` keys in frontmatter SHALL remain literal English

#### Scenario: Schema document materialized with Language Policy preceding workspace layout

- **WHEN** `codebus init` materializes `NEUTRAL_RULES` to the vault's `CLAUDE.md` (Claude provider path) or `AGENTS.md` (codex provider path)
- **THEN** the materialized file SHALL contain `## 0. Language Policy` as a section ordered before `## 1. Workspace Layout`
- **AND** the section body SHALL define both the agent-output-language rule and the structural-tokens-stay-English rule

<!-- @trace
source: prompt-surface-layer-1-batch
updated: 2026-05-24
code:
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/schema/neutral.md
tests:
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Codex-Side SKILL Mode Invocation Trigger

When the codex provider is active and a `codebus` CLI verb (`goal`, `query`, `fix`, `chat`, or `quiz`) spawns a codex agent against an initialized vault, the SKILL bundle materialization plus the codex backend prompt composition SHALL together cause the agent to enter the verb-specific SKILL Mode workflow, not a generic task-reply mode. "Enter SKILL Mode" is defined by the per-verb observable proxy conditions in the scenarios below — codebus SHALL NOT rely on codex CLI internal state to assert this requirement; only externally observable behavior on stdout, stream events, and vault filesystem mutations counts.

The trigger mechanism (sigil form, prompt prefix, SKILL.md frontmatter shape) is an implementation detail of `codebus-core/src/agent/codex_backend.rs` plus `codebus-core/src/vault/init/skills/codex_*.rs` and MAY change across codex CLI versions without spec amendment, provided the observable proxy conditions continue to hold. When the codex CLI version in use prevents all candidate trigger mechanisms from satisfying the proxy conditions, the codebus CLI SHALL surface a non-silent error or warning on stderr identifying the failure rather than reporting success while the SKILL workflow is bypassed.

#### Scenario: Quiz plan spawn emits scope marker

- **WHEN** active provider is `codex` AND the user runs `codebus quiz "<topic>" --count <n>` against an initialized vault containing at least one wiki page on the topic
- **THEN** the first stream line of the plan spawn output SHALL be either `[CODEBUS_QUIZ_SCOPE] ...` or `[CODEBUS_QUIZ_NO_MATCH] ...`, and the codebus CLI SHALL NOT exit with the error `quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/[CODEBUS_QUIZ_NO_MATCH] marker on any line`

#### Scenario: Goal spawn writes at least one wiki page

- **WHEN** active provider is `codex` AND the user runs `codebus goal "<task>"` against an initialized vault AND the task description plausibly implies vault content updates
- **THEN** the agent SHALL write at least one new or modified file under `<vault>/.codebus/wiki/**/*.md` observable via the agent stream's tool-call events and via filesystem inspection after the spawn completes

#### Scenario: Query spawn reads vault wiki

- **WHEN** active provider is `codex` AND the user runs `codebus query "<question>"` against an initialized vault containing wiki pages relevant to the question
- **THEN** the agent stream SHALL contain at least one tool-call event reading a file under `<vault>/.codebus/wiki/**/*.md` (via `Read`, `Glob`, `Grep`, or the codex equivalent), and the agent's final answer SHALL reference at least one vault `[[wikilink]]` or wiki page path

#### Scenario: Chat spawn does not emit vault-vs-source meta-comment

- **WHEN** active provider is `codex` AND the user runs `codebus chat` against an initialized vault and feeds a single-shot question on stdin
- **THEN** the agent's response text SHALL NOT contain a meta-comment of the form "I found this is a documentation vault rather than application source" or equivalent phrasing indicating the agent treated the vault as an unexpected workspace shape; the agent SHALL answer the question grounded in vault content without surfacing its own workspace classification

#### Scenario: Fix spawn enters fix SKILL workflow and repairs the lint warning

- **WHEN** active provider is `codex` AND the user runs `codebus fix` against an initialized vault that has at least one `codebus lint` warning
- **THEN** the agent's first reasoning or tool-call activity SHALL be scoped to repairing the identified lint warning (e.g., locating the offending wiki page, reading it, applying an edit), NOT generic "treat this as a planning task for the codebus-fix project" exploration, AND after the agent terminates the previously-failing `codebus lint` warning SHALL no longer be reported by a re-run of `codebus lint`

#### Scenario: Codex SKILL trigger failure is surfaced, not silenced

- **WHEN** active provider is `codex` AND for any of the five verbs the codex agent fails to enter SKILL Mode (proxy conditions above do not hold)
- **THEN** the codebus CLI SHALL exit with a non-zero status or emit a stderr error or warning that identifies the failing verb and points to actionable diagnostic context, and SHALL NOT print a success summary that masks the failure

<!-- @trace
source: codex-skill-trigger-fix
updated: 2026-05-25
code:
  - codebus-core/src/vault/init.rs
  - codebus-core/src/agent/claude_cli.rs
  - docs/2026-05-25-codex-skill-trigger-diagnose.md
  - codebus-core/src/agent/backend.rs
  - codebus-core/src/agent/codex_backend.rs
-->