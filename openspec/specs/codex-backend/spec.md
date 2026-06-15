# codex-backend Specification

## Purpose

TBD - created by archiving change 'codex-backend'. Update Purpose after archive.

## Requirements

### Requirement: Codex Backend Argv Composition

`CodexBackend` SHALL implement `AgentBackend::build_command` by translating the provider-neutral `SpawnSpec` into a `codex exec` invocation. The composed command SHALL always include the per-spawn isolation flags verified by the 2026-05-22 spike: `--json`, `--ignore-user-config`, `--disable apps`, `--ignore-rules`, `--skip-git-repo-check`, `--ephemeral`, a `-c project_root_markers=['<vault-marker>']` override naming a vault-unique marker file so the codex project root is pinned to the `.codebus/` vault directory, AND a `-c web_search=disabled` config override that turns off codex's hosted web search tool so the agent cannot fetch external URLs at runtime. The composed command SHALL additionally disable codex built-in agent capabilities that codebus does not drive, via a `--disable <id>` pair for each of `plugins`, `hooks`, `browser_use`, `browser_use_external`, `computer_use`, and `in_app_browser`. These disables are feature-surface defense-in-depth: they keep the agent on tested capability ground and keep stderr free of "feature unavailable" noise, but they do NOT move the sandbox / filesystem read-write boundary (which is governed by `-s` plus the OS sandbox) and their marginal security value is ~0. `--disable plugins` does NOT affect codebus's own project-level `.codex/skills/` registration, which is independent of the `plugins` feature. The codex binary path SHALL be resolvable via a `CODEBUS_CODEX_BIN` environment override defaulting to `codex`.

`SpawnSpec.permission` SHALL map to the codex sandbox flag `-s`: `Permission::ReadOnly` SHALL map to `read-only` and `Permission::Workspace` SHALL map to `workspace-write`. The resolved per-verb model SHALL be passed as `-m <model>` and the resolved per-verb effort SHALL be passed as `-c model_reasoning_effort=<effort>` (codex's `.codex/config.toml` is trust-gated and SHALL NOT be relied on for these values). When `SpawnSpec.resume_session_id` is `Some(id)`, the command SHALL use the `codex exec resume <id>` subcommand form. The fully-composed prompt string SHALL be passed as the exec prompt argument, and the child process stdin SHALL be closed or fed empty input so `codex exec` does not block waiting on stdin.

`SpawnSpec.command_allowance` has no codex equivalent (codex gates command execution by sandbox, not by a per-command allowlist). When `command_allowance` is `Some(...)`, `CodexBackend` SHALL proceed without a per-command gate and SHALL emit a single warning rather than failing the spawn (no hard gate).

The `-c web_search=disabled` override is required because codex's `--disable` flag accepts only built-in sub-feature IDs (`apps`, `image_generation`, etc.) and does NOT accept `web_search`; the hosted web search tool can only be turned off through a config-key override. Without this override the agent retains the ability to fetch arbitrary URLs at runtime, which violates the codebus offline / sandbox-bounded contract. Image generation is intentionally NOT disabled by this requirement.

#### Scenario: Read-only permission maps to read-only sandbox

- **WHEN** `build_command` is called with a `SpawnSpec` whose `permission` is `Permission::ReadOnly`
- **THEN** the composed argv SHALL contain `-s read-only` and SHALL NOT contain `workspace-write` or `danger-full-access`

#### Scenario: Workspace permission maps to workspace-write sandbox

- **WHEN** `build_command` is called with a `SpawnSpec` whose `permission` is `Permission::Workspace`
- **THEN** the composed argv SHALL contain `-s workspace-write`

#### Scenario: Isolation flags always present

- **WHEN** `build_command` is called with any `SpawnSpec`
- **THEN** the composed argv SHALL contain all of `--ignore-user-config`, `--disable apps`, `--ignore-rules`, a `project_root_markers` override pinning the project root to the vault directory, a `-c web_search=disabled` pair that turns off codex's hosted web search tool, AND a `--disable <id>` pair for each of `plugins`, `hooks`, `browser_use`, `browser_use_external`, `computer_use`, and `in_app_browser` (feature-surface defense-in-depth, not a sandbox/filesystem boundary)

#### Scenario: Model and effort passed as CLI flags

- **WHEN** `build_command` resolves a verb to model `gpt-5.4` and effort `high`
- **THEN** the composed argv SHALL contain `-m gpt-5.4` and a `-c model_reasoning_effort=high` override

#### Scenario: Resume id uses the resume subcommand

- **WHEN** `build_command` is called with `resume_session_id = Some("019e-...")`
- **THEN** the composed command SHALL use the `codex exec resume 019e-...` subcommand form

#### Scenario: Command allowance degrades with a warning

- **WHEN** `build_command` is called with `command_allowance = Some(...)`
- **THEN** the spawn SHALL proceed without a per-command gate AND a single warning SHALL be emitted AND the build SHALL NOT fail


<!-- @trace
source: backend-cleanup-codex-websearch-and-runid-millis
updated: 2026-05-28
code:
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/goal.rs
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src-tauri/src/ipc/chats.rs
  - codebus-core/src/agent/codex_backend.rs
  - docs/2026-05-28-run-id-collision-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-28-runid-source-of-truth-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/src-tauri/src/ipc/goals.rs
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
  - docs/2026-05-28-cancelling-stuck-todo.md
  - codebus-core/src/verb/query.rs
-->

---
### Requirement: Codex Stream Parsing

`CodexBackend` SHALL implement `AgentBackend::parse_stream_line` as a format-only mapping from one line of codex `--json` JSONL output to zero or more normalized `StreamEvent` values. A `codex exec --json` line of type `item.completed` whose `item.type` is `command_execution` SHALL map to a `StreamEvent::ToolUse { name: "Shell", input }` carrying the `command` field followed by a `StreamEvent::ToolResult { output, is_error }` where `output` is the `aggregated_output` field and `is_error` is `true` when `exit_code` is non-zero. A line of type `item.completed` whose `item.type` is `agent_message` SHALL map to a `StreamEvent::Thought` carrying the `text` field. A line of type `turn.completed` with a `usage` object SHALL map to a `StreamEvent::Usage` whose token counts are taken from the `usage` object's `input_tokens`, `cached_input_tokens`, `output_tokens`, and `reasoning_output_tokens` fields (mapped to the corresponding `TokenUsage` fields, including the reasoning-token field). A line of type `turn.completed` without a `usage` object SHALL return zero `StreamEvent` values and SHALL NOT emit a usage warning. Lines of type `thread.started`, `turn.started`, and `item.started` SHALL NOT produce a `StreamEvent`. `parse_stream_line` SHALL NOT interpret codebus-semantic `[CODEBUS_*]` markers - those remain a verb-layer concern.

When a `turn.completed` line contains a `usage` object but none of the expected usage fields (`input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`) decode as an unsigned integer, the parser SHALL emit exactly one stderr warning prefixed with `warning: codex usage` and SHALL still return the normalized `StreamEvent::Usage` produced by the existing mapping rules. The warning SHALL NOT include the verbatim `usage` JSON object. When at least one expected usage field decodes as an unsigned integer, including a decoded value of zero, the parser SHALL NOT emit this missing-fields warning.

`CodexBackend` SHALL implement `AgentBackend::extract_session_id` to return `Some(id)` for a line of type `thread.started` carrying a `thread_id`, and `None` for every other line.

#### Scenario: Command execution maps to a ToolUse and ToolResult pair

- **WHEN** `parse_stream_line` receives an `item.completed` line with `item.type = command_execution`, `command = "echo hi"`, `aggregated_output = "hi\n"`, and `exit_code = 0`
- **THEN** it SHALL return a `StreamEvent::ToolUse { name: "Shell", ... }` carrying the command followed by a `StreamEvent::ToolResult { output: "hi\n", is_error: false }`

#### Scenario: Non-zero exit code marks the tool result as an error

- **WHEN** `parse_stream_line` receives an `item.completed` `command_execution` line with `exit_code = 1`
- **THEN** the emitted `StreamEvent::ToolResult` SHALL have `is_error = true`

#### Scenario: Agent message maps to a thought

- **WHEN** `parse_stream_line` receives an `item.completed` line with `item.type = agent_message` and `text = "DONE"`
- **THEN** it SHALL return a single `StreamEvent::Thought { text: "DONE" }`

#### Scenario: Turn completion maps usage tokens

- **WHEN** `parse_stream_line` receives a `turn.completed` line whose `usage` has `input_tokens = 30515`, `cached_input_tokens = 22272`, `output_tokens = 43`, and `reasoning_output_tokens = 17`
- **THEN** it SHALL return a `StreamEvent::Usage` whose `TokenUsage` carries those four counts in the corresponding fields and SHALL NOT emit a missing-fields warning

#### Scenario: Turn completion without usage emits no event and no warning

- **WHEN** `parse_stream_line` receives a `turn.completed` line without a `usage` object
- **THEN** it SHALL return zero `StreamEvent` values and SHALL NOT emit a missing-fields warning

#### Scenario: Turn completion with unrecognized usage fields emits a warning

- **WHEN** `parse_stream_line` receives a `turn.completed` line whose `usage` object contains `inputTokenCount = 30515` and `outputTokenCount = 43` but contains none of `input_tokens`, `cached_input_tokens`, `output_tokens`, or `reasoning_output_tokens` as unsigned integers
- **THEN** it SHALL emit exactly one stderr warning prefixed with `warning: codex usage` AND return a `StreamEvent::Usage` whose normalized token counts are all zero or `None` while `TokenUsage.extras` preserves the verbatim `usage` object

#### Scenario: Turn completion with one recognized usage field emits no missing-fields warning

- **WHEN** `parse_stream_line` receives a `turn.completed` line whose `usage` object contains `input_tokens = 12` and `outputTokenCount = 7`
- **THEN** it SHALL return a `StreamEvent::Usage` whose `input_tokens` equals 12 and `output_tokens` equals 0 AND SHALL NOT emit a missing-fields warning

#### Scenario: Thread start yields the session id and no event

- **WHEN** `parse_stream_line` and `extract_session_id` each receive a `thread.started` line with `thread_id = "019e4d0e-..."`
- **THEN** `extract_session_id` SHALL return `Some("019e4d0e-...")` AND `parse_stream_line` SHALL return zero `StreamEvent` values for that line


<!-- @trace
source: codex-backend
updated: 2026-06-15
code:
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/store/chat.ts
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/mod.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-core/src/verb/error.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-app/src/store/goals.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/dispatch.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/config/endpoint.rs
tests:
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
-->

---
### Requirement: Provider Dispatch Selection

The system SHALL provide a runtime dispatch that maps the configured `active_provider` to a `Box<dyn AgentBackend>`: `claude` (or an absent `active_provider`, which defaults to claude) SHALL select `ClaudeBackend` and `codex` SHALL select `CodexBackend`. Each verb (`goal`, `query`, `fix`, `chat`, `quiz`) SHALL construct its backend through this dispatch rather than constructing `ClaudeBackend` directly, so changing `active_provider` re-routes every verb's spawn without further per-verb changes.

#### Scenario: Codex provider routes to CodexBackend

- **WHEN** the loaded config has `agent.active_provider: codex`
- **THEN** the dispatch SHALL return a `CodexBackend` and every verb SHALL drive its spawns through it

#### Scenario: Claude provider or absent selector routes to ClaudeBackend

- **WHEN** the loaded config has `agent.active_provider: claude` OR no `active_provider` key
- **THEN** the dispatch SHALL return a `ClaudeBackend`

<!-- @trace
source: codex-backend
updated: 2026-05-23
code:
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/store/chat.ts
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/mod.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-core/src/verb/error.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-app/src/store/goals.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/dispatch.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/config/endpoint.rs
tests:
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
-->

---
### Requirement: Codex Multi-Line Prompt Stdin Routing

The codex backend SHALL route any prompt that contains a newline character (`\n`) through the child process's stdin pipe instead of the prompt argv element, because Rust's standard library rejects newline-containing argv elements with `InvalidInput: batch file arguments are invalid` since 1.77 when the spawned executable resolves to a Windows `.cmd` or `.bat` shim — and codex's npm install on Windows is exactly such a `.cmd` shim. The routing decision SHALL be a pure function of the formatted prompt (single function, `format_codex_prompt`, shared by `build_command` and `stdin_payload` so they cannot disagree).

When the formatted prompt contains a newline:

1. `CodexBackend::build_command` SHALL pass `-` (a single hyphen) as the prompt argv element. `codex exec` interprets `-` as "read the prompt from stdin" per its CLI contract.
2. `CodexBackend::stdin_payload(spec)` SHALL return `Some(formatted_prompt)` so the invocation loop pipes stdin and writes the payload.
3. No argv element SHALL contain `\n`, `\r`, or `\0`, regardless of which `-c` config overrides or model flags are present.

When the formatted prompt is single-line (no `\n`):

1. `CodexBackend::build_command` SHALL pass the formatted prompt as the final argv element (preserving the existing visible-argv contract used by tests and observability tools).
2. `CodexBackend::stdin_payload(spec)` SHALL return `None` so the invocation loop keeps stdin closed (the historical codex contract — codex exec blocks waiting on stdin when given an open empty pipe with no data).

The verify-stage and repair-stage spawns from the `verb::goal` and `verb::quiz` content-verify pipeline routinely produce multi-line prompts (`goal=<task>\n\nCHANGED PAGES:\n<paths>` and `goal=<task>\n\nCONTENT DEFECTS:\n<defects>\n\nFLAGGED PAGES:\n<pages>`); these SHALL route through stdin and SHALL NOT fail with the historical `spawn agent: batch file arguments are invalid` error.

#### Scenario: Multi-line verify prompt is passed via stdin

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `sub_mode = Some("verify")` and whose `input` contains newlines (e.g. `goal=X\n\nCHANGED PAGES:\nwiki/a.md`)
- **THEN** the composed argv's final element SHALL be `-`, no argv element SHALL contain `\n`, and `CodexBackend::stdin_payload(&spec)` SHALL return `Some("$codebus-goal verify: goal=X\n\nCHANGED PAGES:\nwiki/a.md")` so the invocation loop writes that string to the child's stdin

#### Scenario: Single-line plan prompt stays in argv

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `input` contains no newlines (e.g. `JWT issuance and verification`)
- **THEN** the composed argv's final element SHALL be the fully formatted prompt (e.g. `$codebus-quiz plan: JWT issuance and verification`), AND `CodexBackend::stdin_payload(&spec)` SHALL return `None` (stdin stays closed)

#### Scenario: codebus verify-stage spawn does not fail with batch-file argv error

- **WHEN** `codebus goal` or `codebus quiz` runs against an initialized vault on Windows with `active_provider=codex` AND the content-verify stage assembles its multi-line prompt
- **THEN** the spawn SHALL NOT print `spawn agent: batch file arguments are invalid` (or the underlying `InvalidInput` from Rust's stdlib) AND the verify stage SHALL execute and emit either `CONTENT_OK` or `<id> | <defect-type> | <suggestion>` lines per the `verb::content_verify` line grammar


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
### Requirement: Codex Sandbox Write Enablement Override

The codex backend's `build_command` SHALL inject a platform-specific sandbox elevation override into the per-spawn argv whenever the resolved `SpawnSpec.permission` is `Permission::Workspace`, so that the codex agent's filesystem sandbox actually permits writes to the vault working directory. The override SHALL be passed as an additional `-c <key>=<value>` argument (unquoted; codex parses each `-c` value as TOML and falls back to a literal string when the value is not valid TOML, so the bare identifier becomes a string literal without any embedded quotes that would interfere with the Windows `.cmd` shim's re-quoting) alongside the existing isolation recipe flags (`--ignore-user-config`, `--disable apps`, `--ignore-rules`, `-s workspace-write`, `-c project_root_markers=...`) without removing or weakening any of them.

This requirement exists because `--ignore-user-config` strips the user's per-platform sandbox enablement default from the loaded config, which leaves codex's sandbox at a conservative read-only baseline even when the CLI flag `-s workspace-write` is also present. The override re-establishes the write-capable baseline per-spawn without re-introducing any user-config-derived MCP servers, plugins, personality presets, or trust list.

On Windows, the override SHALL be `-c windows.sandbox=unelevated`. The value `unelevated` (not `elevated`) is required so that codex's Windows sandbox runs as the current user; `elevated` requires the parent process to already be admin and aborts subprocess spawning otherwise (`windows sandbox: spawn setup refresh` error). Codex 0.133.0 accepts only `elevated` or `unelevated` for `windows.sandbox`; other values are rejected with `unknown variant`. On macOS and Linux, the corresponding override may be a no-op or may require a different key; behavior on those platforms is intentionally deferred and SHALL be considered a follow-up change once those platforms are exercised. The codex backend SHALL pass the Windows override unconditionally when constructing the argv on any host platform — codex's TOML config tolerates unknown-platform tables, and the override is harmless on non-Windows hosts.

When `SpawnSpec.permission` is `Permission::ReadOnly`, the override SHALL still be safe to include (codex's sandbox stays read-only because `-s read-only` takes effect; the elevation override only applies to the workspace-write path).

#### Scenario: Workspace permission argv includes sandbox elevation override

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `permission` is `Permission::Workspace`
- **THEN** the composed argv SHALL contain a `-c windows.sandbox=unelevated` pair (the `-c` flag followed by the `windows.sandbox=unelevated` value) alongside the existing `-s workspace-write` mapping, and SHALL NOT contain `--dangerously-bypass-approvals-and-sandbox` or any equivalent sandbox-bypass flag

#### Scenario: Read-only permission argv still includes the override harmlessly

- **WHEN** `CodexBackend::build_command` is invoked with a `SpawnSpec` whose `permission` is `Permission::ReadOnly`
- **THEN** the composed argv SHALL contain `-s read-only`, MAY contain the `-c windows.sandbox=unelevated` override (codex's sandbox stays read-only because the sandbox-mode flag governs), and SHALL NOT contain `--dangerously-bypass-approvals-and-sandbox`

#### Scenario: Workspace-write spawn against initialized vault actually writes

- **WHEN** active provider is `codex` AND the user runs a `Permission::Workspace` verb (`goal` or `fix`) against an initialized vault on Windows AND the agent attempts an `apply_patch` or equivalent file write under the vault directory
- **THEN** the write SHALL succeed and the resulting file change SHALL be observable via filesystem inspection after the spawn completes, and the agent SHALL NOT self-classify as "read-only filesystem sandbox" or "approvals are disabled" in its final message

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