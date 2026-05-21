## ADDED Requirements

### Requirement: Agent Backend Trait Contract

The codebus core SHALL define an `AgentBackend` trait that is the sole contract between the provider-agnostic invocation loop and a concrete agent CLI. The trait SHALL declare exactly three methods: `build_command` (maps a `SpawnSpec` to a `std::process::Command`), `parse_stream_line` (maps one raw stdout line to zero or more `StreamEvent`), and `extract_session_id` (maps one raw stdout line to an optional session id). The trait SHALL NOT expose tool, sandbox, MCP, model, or argv concepts to its caller — those SHALL be encapsulated entirely inside the implementing type.

#### Scenario: Trait exposes exactly the three contract methods

- **WHEN** a type implements `AgentBackend`
- **THEN** it SHALL provide `build_command(&SpawnSpec) -> Command`, `parse_stream_line(&str) -> Vec<StreamEvent>`, and `extract_session_id(&str) -> Option<String>` AND the trait SHALL NOT require any method that takes tool / sandbox / model parameters

#### Scenario: Backend output is the normalized event contract

- **WHEN** `parse_stream_line` is called with a provider stdout line
- **THEN** it SHALL return `Vec<StreamEvent>` (the normalized cross-provider event type) AND SHALL NOT return any provider-specific event shape

### Requirement: SpawnSpec Provider-Neutral Intent

The `SpawnSpec` type SHALL carry provider-neutral spawn intent and SHALL NOT embed provider-specific encodings (no slash-command strings, no CLI flag glob syntax). `SpawnSpec` SHALL contain: `verb` (the existing `Verb` enum), `input` (user text), `permission` (an enum with variants `ReadOnly` and `Workspace`), `command_allowance` (an optional `CommandPrefix` holding a neutral command token sequence), and `resume_session_id` (optional). The `permission`, `command_allowance`, and `resume_session_id` fields SHALL be per-spawn values, NOT derived from `verb`, because a single verb can issue multiple spawns with differing permission. The codebus core SHALL NOT introduce a separate `SpawnRole` enum; model/effort resolution SHALL reuse the existing `Verb` enum and its resolution function.

#### Scenario: A single verb issues multiple spawns with differing permission

- **WHEN** the quiz flow runs
- **THEN** it SHALL issue a plan spawn with `verb: Quiz, permission: ReadOnly`, a generate spawn with `verb: Quiz, permission: ReadOnly, command_allowance: Some(["codebus","quiz","validate"])`, and a content-verify spawn with `verb: Verify, permission: ReadOnly`

#### Scenario: command_allowance is a neutral token sequence

- **WHEN** a `SpawnSpec` restricts the agent to a single command family
- **THEN** `command_allowance` SHALL hold a `CommandPrefix` of plain tokens (e.g. `["codebus","quiz","validate"]`) AND SHALL NOT hold a Claude `--allowedTools` glob string such as `Bash(codebus quiz validate *)`

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

### Requirement: Invocation Loop Drives Backend Trait

The `agent::invoke` function SHALL accept an `&dyn AgentBackend` parameter and SHALL delegate command construction, stdout line parsing, and session-id extraction to that backend. The spawn / stdio piping / cancellation polling / stderr passthrough / token accumulation loop SHALL remain provider-agnostic and SHALL NOT contain any provider-specific branching or hard-coded `claude` argv.

#### Scenario: invoke delegates to the supplied backend

- **WHEN** `invoke` is called with a `&dyn AgentBackend`
- **THEN** the child process SHALL be spawned from the `Command` returned by `backend.build_command(...)` AND each stdout line SHALL be parsed via `backend.parse_stream_line(...)` AND the session id SHALL be captured via `backend.extract_session_id(...)`

#### Scenario: Loop body contains no provider-specific code

- **WHEN** the `invoke` loop processes stdout, polls cancellation, and accumulates `Usage` events
- **THEN** none of that loop logic SHALL reference the `claude` binary name, Claude argv flags, or Claude stream-json field names directly
