## ADDED Requirements

### Requirement: Codex Backend Argv Composition

`CodexBackend` SHALL implement `AgentBackend::build_command` by translating the provider-neutral `SpawnSpec` into a `codex exec` invocation. The composed command SHALL always include the per-spawn isolation flags verified by the 2026-05-22 spike: `--json`, `--ignore-user-config`, `--disable apps`, `--ignore-rules`, `--skip-git-repo-check`, `--ephemeral`, and a `-c project_root_markers=['<vault-marker>']` override naming a vault-unique marker file so the codex project root is pinned to the `.codebus/` vault directory. The codex binary path SHALL be resolvable via a `CODEBUS_CODEX_BIN` environment override defaulting to `codex`.

`SpawnSpec.permission` SHALL map to the codex sandbox flag `-s`: `Permission::ReadOnly` SHALL map to `read-only` and `Permission::Workspace` SHALL map to `workspace-write`. The resolved per-verb model SHALL be passed as `-m <model>` and the resolved per-verb effort SHALL be passed as `-c model_reasoning_effort=<effort>` (codex's `.codex/config.toml` is trust-gated and SHALL NOT be relied on for these values). When `SpawnSpec.resume_session_id` is `Some(id)`, the command SHALL use the `codex exec resume <id>` subcommand form. The fully-composed prompt string SHALL be passed as the exec prompt argument, and the child process stdin SHALL be closed or fed empty input so `codex exec` does not block waiting on stdin.

`SpawnSpec.command_allowance` has no codex equivalent (codex gates command execution by sandbox, not by a per-command allowlist). When `command_allowance` is `Some(...)`, `CodexBackend` SHALL proceed without a per-command gate and SHALL emit a single warning rather than failing the spawn (no hard gate).

#### Scenario: Read-only permission maps to read-only sandbox

- **WHEN** `build_command` is called with a `SpawnSpec` whose `permission` is `Permission::ReadOnly`
- **THEN** the composed argv SHALL contain `-s read-only` and SHALL NOT contain `workspace-write` or `danger-full-access`

#### Scenario: Workspace permission maps to workspace-write sandbox

- **WHEN** `build_command` is called with a `SpawnSpec` whose `permission` is `Permission::Workspace`
- **THEN** the composed argv SHALL contain `-s workspace-write`

#### Scenario: Isolation flags always present

- **WHEN** `build_command` is called with any `SpawnSpec`
- **THEN** the composed argv SHALL contain all of `--ignore-user-config`, `--disable apps`, `--ignore-rules`, and a `project_root_markers` override pinning the project root to the vault directory

#### Scenario: Model and effort passed as CLI flags

- **WHEN** `build_command` resolves a verb to model `gpt-5.4` and effort `high`
- **THEN** the composed argv SHALL contain `-m gpt-5.4` and a `-c model_reasoning_effort=high` override

#### Scenario: Resume id uses the resume subcommand

- **WHEN** `build_command` is called with `resume_session_id = Some("019e-...")`
- **THEN** the composed command SHALL use the `codex exec resume 019e-...` subcommand form

#### Scenario: Command allowance degrades with a warning

- **WHEN** `build_command` is called with `command_allowance = Some(...)`
- **THEN** the spawn SHALL proceed without a per-command gate AND a single warning SHALL be emitted AND the build SHALL NOT fail

### Requirement: Codex Stream Parsing

`CodexBackend` SHALL implement `AgentBackend::parse_stream_line` as a format-only mapping from one line of codex `--json` JSONL output to zero or more normalized `StreamEvent` values. A `codex exec --json` line of type `item.completed` whose `item.type` is `command_execution` SHALL map to a `StreamEvent::ToolUse { name: "Shell", input }` carrying the `command` field followed by a `StreamEvent::ToolResult { output, is_error }` where `output` is the `aggregated_output` field and `is_error` is `true` when `exit_code` is non-zero. A line of type `item.completed` whose `item.type` is `agent_message` SHALL map to a `StreamEvent::Thought` carrying the `text` field. A line of type `turn.completed` SHALL map to a `StreamEvent::Usage` whose token counts are taken from the `usage` object's `input_tokens`, `cached_input_tokens`, `output_tokens`, and `reasoning_output_tokens` fields (mapped to the corresponding `TokenUsage` fields, including the reasoning-token field). Lines of type `thread.started`, `turn.started`, and `item.started` SHALL NOT produce a `StreamEvent`. `parse_stream_line` SHALL NOT interpret codebus-semantic `[CODEBUS_*]` markers — those remain a verb-layer concern.

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
- **THEN** it SHALL return a `StreamEvent::Usage` whose `TokenUsage` carries those four counts in the corresponding fields

#### Scenario: Thread start yields the session id and no event

- **WHEN** `parse_stream_line` and `extract_session_id` each receive a `thread.started` line with `thread_id = "019e4d0e-..."`
- **THEN** `extract_session_id` SHALL return `Some("019e4d0e-...")` AND `parse_stream_line` SHALL return zero `StreamEvent` values for that line

### Requirement: Provider Dispatch Selection

The system SHALL provide a runtime dispatch that maps the configured `active_provider` to a `Box<dyn AgentBackend>`: `claude` (or an absent `active_provider`, which defaults to claude) SHALL select `ClaudeBackend` and `codex` SHALL select `CodexBackend`. Each verb (`goal`, `query`, `fix`, `chat`, `quiz`) SHALL construct its backend through this dispatch rather than constructing `ClaudeBackend` directly, so changing `active_provider` re-routes every verb's spawn without further per-verb changes.

#### Scenario: Codex provider routes to CodexBackend

- **WHEN** the loaded config has `agent.active_provider: codex`
- **THEN** the dispatch SHALL return a `CodexBackend` and every verb SHALL drive its spawns through it

#### Scenario: Claude provider or absent selector routes to ClaudeBackend

- **WHEN** the loaded config has `agent.active_provider: claude` OR no `active_provider` key
- **THEN** the dispatch SHALL return a `ClaudeBackend`
