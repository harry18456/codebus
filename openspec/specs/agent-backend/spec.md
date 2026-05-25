# agent-backend Specification

## Purpose

TBD - created by archiving change 'agent-backend-seam'. Update Purpose after archive.

## Requirements

### Requirement: Agent Backend Trait Contract

The codebus core SHALL define an `AgentBackend` trait that is the sole contract between the provider-agnostic invocation loop and a concrete agent CLI. The trait SHALL declare three required methods (`build_command`, `parse_stream_line`, `extract_session_id`) and MAY declare additional optional methods whose default implementations preserve the existing three-method behavior. The currently-permitted optional method is `stdin_payload(&SpawnSpec) -> Option<String>`, with a default `None` body so backends that do not need it can continue to implement only the three required methods. The trait SHALL NOT expose tool, sandbox, MCP, model, or argv concepts to its caller — those SHALL be encapsulated entirely inside the implementing type. Any optional method SHALL be motivated by a concrete cross-backend variation (not speculative future extension) and SHALL have a safe default that preserves the prior contract.

#### Scenario: Trait exposes the required contract methods

- **WHEN** a type implements `AgentBackend`
- **THEN** it SHALL provide `build_command(&SpawnSpec) -> Command`, `parse_stream_line(&str) -> Vec<StreamEvent>`, and `extract_session_id(&str) -> Option<String>` AND the trait SHALL NOT require any method that takes tool / sandbox / model parameters

#### Scenario: Backend output is the normalized event contract

- **WHEN** `parse_stream_line` is called with a provider stdout line
- **THEN** it SHALL return `Vec<StreamEvent>` (the normalized cross-provider event type) AND SHALL NOT return any provider-specific event shape

#### Scenario: Optional stdin payload method has a safe default

- **WHEN** a backend implements only the three required methods
- **THEN** the trait's default `stdin_payload` implementation SHALL return `None`, AND the invocation loop SHALL close the child's stdin as before (no behavior change for backends that do not opt in)

#### Scenario: Backend opt-in routes a multi-line prompt to stdin

- **WHEN** a backend's `stdin_payload(spec)` returns `Some(payload)`
- **THEN** the invocation loop SHALL open the child's stdin as a pipe, write `payload` to it, and close the pipe before reading stdout — and the backend's own `build_command` SHALL have used `-` (or omitted) as the prompt argv element so the CLI reads from stdin


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

---
### Requirement: SpawnSpec Provider-Neutral Intent

The `SpawnSpec` type SHALL carry provider-neutral spawn intent and SHALL NOT embed provider-specific encodings (no slash-command strings, no CLI flag glob syntax, no provider-specific trigger prefix). `SpawnSpec` SHALL contain:

- `verb` (one of the five SKILL bundle verbs: `Goal`, `Query`, `Fix`, `Chat`, `Quiz`) — the bundle name used by both providers to address the SKILL workflow. The `verb` field is the **bundle identity**, NOT the model-resolution key.
- `resolve_as` (`Option<Verb>`) — optional model-resolution override. When `None`, the backend SHALL resolve model/effort via `resolve(verb)` (i.e. the bundle's own config sub-block). When `Some(other_verb)`, the backend SHALL resolve via `resolve(other_verb)` instead. The override exists for **cross-flow content-verify spawns**: goal verify and quiz verify spawns set `verb: Goal` / `verb: Quiz` (the SKILL bundle they invoke) but `resolve_as: Some(Verb::Verify)` (so model/effort come from the dedicated `verify` config sub-block per the verify-stage-independent-model pattern).
- `sub_mode` (`Option<String>`) — when present, names a verb sub-mode such as `verify`, `repair`, `plan`, `generate`; when absent, the spawn is a free-text invocation.
- `input` (`String`) — user text or structured body.
- `permission` (an enum with variants `ReadOnly` and `Workspace`).
- `command_allowance` (an optional `CommandPrefix` holding a neutral command token sequence).
- `resume_session_id` (optional).

The `permission`, `command_allowance`, `sub_mode`, `resolve_as`, and `resume_session_id` fields SHALL be per-spawn values, NOT derived from `verb`, because a single verb can issue multiple spawns with differing permission, sub-mode, and model-resolution context. The codebus core SHALL NOT introduce a separate `SpawnRole` enum; model/effort resolution SHALL reuse the existing `Verb` enum and its resolution function (via `resolve_as.unwrap_or(verb)` for the lookup key).

**Backend assembly responsibility**: each concrete `AgentBackend` implementation SHALL synthesize the provider-specific invocation string from `verb` + `sub_mode` + `input`. The verb layer SHALL NOT pre-compose any slash-command or dollar-prefix string into `SpawnSpec`; passing such a pre-composed string would violate the provider-neutral intent of `SpawnSpec`.

**Provider-specific assembly forms**:
- The Claude backend SHALL assemble `/codebus-{verb} {sub_mode}: {input}` when `sub_mode` is `Some`, OR `/codebus-{verb} "{input}"` (with double-quote wrapping) when `sub_mode` is `None`. The `-p` CLI flag SHALL carry the assembled string.
- The codex backend SHALL assemble `$codebus-{verb} {sub_mode}: {input}` when `sub_mode` is `Some`, OR `$codebus-{verb} {input}` (no quote wrapping) when `sub_mode` is `None`. The first positional argument SHALL carry the assembled string. The `$`-prefix invokes the codex CLI's native skill explicit-invocation mechanism (verified 2026-05-23 against codex-cli 0.133.0: `$`-prefix saves approximately 24.8% input tokens versus the claude `/`-prefix because codex routes `/`-prefix through description-match implicit invocation, which adds a separate Read of the SKILL body).

#### Scenario: A single verb issues multiple spawns with differing permission

- **WHEN** the quiz flow runs
- **THEN** it SHALL issue a plan spawn with `verb: Quiz, sub_mode: Some("plan"), resolve_as: None, permission: ReadOnly`, a generate spawn with `verb: Quiz, sub_mode: Some("generate"), resolve_as: None, permission: ReadOnly, command_allowance: Some(["codebus","quiz","validate"])`, and a content-verify spawn with `verb: Quiz, sub_mode: Some("verify"), resolve_as: Some(Verb::Verify), permission: ReadOnly` (the verify spawn invokes the quiz SKILL bundle but resolves model/effort from the dedicated `verify` config sub-block)

#### Scenario: command_allowance is a neutral token sequence

- **WHEN** a `SpawnSpec` restricts the agent to a single command family
- **THEN** `command_allowance` SHALL hold a `CommandPrefix` of plain tokens (e.g. `["codebus","quiz","validate"]`) AND SHALL NOT hold a Claude `--allowedTools` glob string such as `Bash(codebus quiz validate *)`

#### Scenario: Claude backend assembles slash-prefix invocation from SpawnSpec fields

- **WHEN** the Claude backend receives a `SpawnSpec { verb: Goal, sub_mode: None, input: "draft payments overview" }`
- **THEN** the assembled `-p` argument SHALL equal the literal string `/codebus-Goal "draft payments overview"` (quote-wrapped free-text form)
- **WHEN** the Claude backend receives a `SpawnSpec { verb: Goal, sub_mode: Some("verify"), input: "goal=X\n\nCHANGED PAGES:\n..." }`
- **THEN** the assembled `-p` argument SHALL equal the literal string `/codebus-Goal verify: goal=X\n\nCHANGED PAGES:\n...` (sub-mode prefix form, no quote wrapping)

##### Example: Claude assembly for chat verb free-text

- **GIVEN** `SpawnSpec { verb: Chat, sub_mode: None, input: "explain the auth flow" }`
- **WHEN** the Claude backend builds the `claude` CLI command
- **THEN** the `-p` argument SHALL be the literal string `/codebus-Chat "explain the auth flow"`

##### Example: Claude assembly for quiz verb plan sub-mode

- **GIVEN** `SpawnSpec { verb: Quiz, sub_mode: Some("plan"), input: "auth middleware" }`
- **WHEN** the Claude backend builds the `claude` CLI command
- **THEN** the `-p` argument SHALL be the literal string `/codebus-Quiz plan: auth middleware`

#### Scenario: codex backend assembles dollar-prefix invocation from SpawnSpec fields

- **WHEN** the codex backend receives a `SpawnSpec { verb: Goal, sub_mode: None, input: "draft payments overview" }`
- **THEN** the assembled first positional argument SHALL equal the literal string `$codebus-Goal draft payments overview` (no quote wrapping)
- **WHEN** the codex backend receives a `SpawnSpec { verb: Goal, sub_mode: Some("verify"), input: "goal=X\n\nCHANGED PAGES:\n..." }`
- **THEN** the assembled first positional argument SHALL equal the literal string `$codebus-Goal verify: goal=X\n\nCHANGED PAGES:\n...` (sub-mode prefix form)

##### Example: codex assembly for chat verb free-text

- **GIVEN** `SpawnSpec { verb: Chat, sub_mode: None, input: "explain the auth flow" }`
- **WHEN** the codex backend builds the `codex` CLI command
- **THEN** the first positional argument SHALL be the literal string `$codebus-Chat explain the auth flow`

##### Example: codex assembly for quiz verb plan sub-mode

- **GIVEN** `SpawnSpec { verb: Quiz, sub_mode: Some("plan"), input: "auth middleware" }`
- **WHEN** the codex backend builds the `codex` CLI command
- **THEN** the first positional argument SHALL be the literal string `$codebus-Quiz plan: auth middleware`

#### Scenario: SpawnSpec does not embed provider-specific trigger prefix

- **WHEN** a verb layer constructs a `SpawnSpec`
- **THEN** the `input` field SHALL NOT begin with `/codebus-` or `$codebus-` (those prefixes are backend-assembly territory)
- **AND** the `input` field SHALL NOT contain `\"` (double-quote) escaping around free text (claude backend adds quote wrapping on free-text spawns; codex backend never adds quotes — verb layer is unaware of either)


<!-- @trace
source: prompt-surface-layer-3-spawnspec-restructure
updated: 2026-05-24
code:
  - codebus-core/src/agent/spawn_spec.rs
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/wiki/fix/prompt.rs
  - codebus-core/src/verb/query.rs
  - codebus-core/src/verb/goal.rs
tests:
  - codebus-cli/tests/scoped_env_injection.rs
-->

---
### Requirement: Claude Backend Argv Equivalence

`ClaudeBackend` SHALL implement `AgentBackend`. For any `SpawnSpec`, `ClaudeBackend::build_command` SHALL produce a `claude` argv byte-equivalent to the pre-refactor `build_claude_cmd` for the corresponding inputs. This SHALL include: the `-p /codebus-<verb> "<input>"` slash invocation, the `--tools` / `--allowedTools` / `--permission-mode acceptEdits` flags, the MCP isolation flags (`--strict-mcp-config` plus an empty `--mcp-config`), the `--model` / `--effort` flags resolved from config, and `--resume <id>` placement before the toolset flags when `resume_session_id` is `Some`. `ClaudeBackend::parse_stream_line` and `extract_session_id` SHALL produce results identical to the pre-refactor `parse_claude_stream_line` and `sniff_init_session_id`.

#### Scenario: Read-only permission excludes write tools

- **WHEN** `build_command` is called with `permission: ReadOnly` and no `command_allowance`
- **THEN** the `--tools` value SHALL contain the read-only tool set (Read / Glob / Grep) AND SHALL NOT contain `Write`, `Edit`, or `Bash`

#### Scenario: command_allowance maps to fine-grained Bash specifier

- **WHEN** `build_command` is called with `command_allowance: Some(["codebus","quiz","validate"])`
- **THEN** the `--allowedTools` value SHALL contain `Bash(codebus quiz validate *)` AND the `--tools` value SHALL contain bare `Bash`

#### Scenario: Argv is byte-equivalent to pre-refactor builder

- **WHEN** a `SpawnSpec` is constructed for a goal spawn (`verb: Goal, permission: Workspace`, model/effort resolved)
- **THEN** the argv produced by `ClaudeBackend::build_command` SHALL equal, token-for-token, the argv the pre-refactor `build_claude_cmd` produced for the equivalent `InvokeAgentOptions`

#### Scenario: Resume id placed before toolset flags

- **WHEN** `build_command` is called with `resume_session_id: Some("abc-123")`
- **THEN** `--resume abc-123` SHALL appear in the argv before the `--tools` flag


<!-- @trace
source: agent-backend-seam
updated: 2026-05-21
code:
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/mod.rs
  - docs/2026-05-21-multi-provider-design-discussion.md
  - codebus-core/src/agent/spawn_spec.rs
  - codebus-core/src/verb/query.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-core/src/agent/backend.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-app/src/store/settings.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/verb/chat.rs
  - docs/v3-roadmap.md
tests:
  - codebus-core/tests/endpoint_config_load.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/azure_key_pre_spawn.rs
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
  - codebus-cli/tests/config_subcommand.rs
  - codebus-cli/tests/scoped_env_injection.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-cli/tests/goal_content_verify_cli.rs
-->

---
### Requirement: Invocation Loop Drives Backend Trait

The `agent::invoke` function SHALL accept an `&dyn AgentBackend` parameter and SHALL delegate command construction, stdout line parsing, and session-id extraction to that backend. The spawn / stdio piping / cancellation polling / stderr passthrough / token accumulation loop SHALL remain provider-agnostic and SHALL NOT contain any provider-specific branching or hard-coded `claude` argv.

#### Scenario: invoke delegates to the supplied backend

- **WHEN** `invoke` is called with a `&dyn AgentBackend`
- **THEN** the child process SHALL be spawned from the `Command` returned by `backend.build_command(...)` AND each stdout line SHALL be parsed via `backend.parse_stream_line(...)` AND the session id SHALL be captured via `backend.extract_session_id(...)`

#### Scenario: Loop body contains no provider-specific code

- **WHEN** the `invoke` loop processes stdout, polls cancellation, and accumulates `Usage` events
- **THEN** none of that loop logic SHALL reference the `claude` binary name, Claude argv flags, or Claude stream-json field names directly

<!-- @trace
source: agent-backend-seam
updated: 2026-05-21
code:
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/mod.rs
  - docs/2026-05-21-multi-provider-design-discussion.md
  - codebus-core/src/agent/spawn_spec.rs
  - codebus-core/src/verb/query.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-core/src/agent/backend.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-app/src/store/settings.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/verb/chat.rs
  - docs/v3-roadmap.md
tests:
  - codebus-core/tests/endpoint_config_load.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/azure_key_pre_spawn.rs
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
  - codebus-cli/tests/config_subcommand.rs
  - codebus-cli/tests/scoped_env_injection.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-cli/tests/goal_content_verify_cli.rs
-->