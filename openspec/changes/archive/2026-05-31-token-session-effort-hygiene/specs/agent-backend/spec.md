## ADDED Requirements

### Requirement: Provider-Declared Token Usage Semantics

Each `AgentBackend` implementation SHALL declare how its emitted `Usage` token events combine across one invocation, via an opt-in trait method `token_usage_semantics(&self) -> TokenUsageSemantics` with a default return of `TokenUsageSemantics::Delta`. `TokenUsageSemantics` SHALL be a closed enum with exactly two variants: `Delta` (each `Usage` event reports the tokens attributable to that event alone; the per-invocation total is the field-wise sum of all events) and `Cumulative` (each `Usage` event reports a running total for the invocation so far; the per-invocation total is the latest event, NOT a sum). The Claude backend SHALL use the default `Delta` (the Claude CLI emits one `result` usage event per `-p` run). The codex backend SHALL override to `Cumulative` (the codex `turn.completed.usage` field carries a cumulative total, not a per-turn delta).

`agent::invoke` SHALL read the backend's declared semantics once and combine each `Usage` event into the accumulated `TokenUsage` accordingly: under `Delta` it SHALL field-wise sum (the existing `accumulate_token_usage` behavior); under `Cumulative` it SHALL replace the accumulated value with the latest event (last-wins). This dispatch SHALL remain provider-agnostic per the `Invocation Loop Drives Backend Trait` requirement — the loop SHALL branch on the `TokenUsageSemantics` value only and SHALL NOT reference any provider binary name, provider argv flag, or provider stream-json field name. The resulting accumulated `TokenUsage` is the value recorded as `RunLog.tokens` per the `run-log` capability; for a `Cumulative` backend this value is the run's final cumulative total and SHALL NOT be double-counted across multiple `Usage` events.

This requirement SHALL NOT alter the serialized shape of `StreamEvent` (events.jsonl) or `TokenUsage` (runs.jsonl): `TokenUsageSemantics` is a transient combination directive used only inside `invoke` and SHALL NOT be serialized into either jsonl format.

#### Scenario: Delta backend sums usage events

- **WHEN** `invoke` runs against a backend whose `token_usage_semantics()` returns `Delta` AND the stream yields two `Usage` events with `input_tokens` 100 then 25
- **THEN** the accumulated `RunLog.tokens.input_tokens` SHALL equal 125 (field-wise sum)

#### Scenario: Cumulative backend takes the latest usage snapshot

- **WHEN** `invoke` runs against a backend whose `token_usage_semantics()` returns `Cumulative` AND the stream yields two `Usage` events with `input_tokens` 100 then 250
- **THEN** the accumulated `RunLog.tokens.input_tokens` SHALL equal 250 (latest cumulative snapshot) AND SHALL NOT equal 350 (the sum)

#### Scenario: Codex backend declares cumulative, Claude backend declares delta

- **WHEN** `token_usage_semantics()` is queried on the codex backend and on the claude backend
- **THEN** the codex backend SHALL return `Cumulative` AND the claude backend SHALL return `Delta`

#### Scenario: Semantics dispatch references no provider identity

- **WHEN** the `invoke` loop combines `Usage` events using the declared semantics
- **THEN** the dispatch SHALL branch only on the `TokenUsageSemantics` enum value AND SHALL NOT reference the `claude` or `codex` binary name, provider-specific argv flags, or provider-specific stream-json field names

## MODIFIED Requirements

### Requirement: Claude Backend Argv Equivalence

`ClaudeBackend` SHALL implement `AgentBackend`. For any `SpawnSpec`, `ClaudeBackend::build_command` SHALL produce a `claude` argv equal to the pre-refactor `build_claude_cmd` for the corresponding inputs, EXCEPT for the additive `--no-session-persistence` flag introduced by the session-persistence gating below. This SHALL include: the `-p /codebus-<verb> "<input>"` slash invocation, the `--tools` / `--allowedTools` / `--permission-mode acceptEdits` flags, the MCP isolation flags (`--strict-mcp-config` plus an empty `--mcp-config`), the `--model` / `--effort` flags resolved from config, and `--resume <id>` placement before the toolset flags when `resume_session_id` is `Some`. `ClaudeBackend::parse_stream_line` and `extract_session_id` SHALL produce results identical to the pre-refactor `parse_claude_stream_line` and `sniff_init_session_id`.

`ClaudeBackend::build_command` SHALL gate Claude CLI session persistence on the spawn's verb, mirroring the codex backend's `--ephemeral` gate: for every verb OTHER THAN `Verb::Chat` (i.e. `Goal` / `Query` / `Fix` / `Quiz`, including cross-flow `Verify` spawns) the argv SHALL include the `--no-session-persistence` flag, so these single-shot verbs (which never resume) leave no Claude session rollout on disk. For `Verb::Chat` the argv SHALL NOT include `--no-session-persistence`, because chat is multi-turn and depends on session persistence for `--resume <id>` to continue the conversation. The `--no-session-persistence` flag SHALL be valid only because codebus always spawns Claude in `-p` (print) mode.

#### Scenario: Read-only permission excludes write tools

- **WHEN** `build_command` is called with `permission: ReadOnly` and no `command_allowance`
- **THEN** the `--tools` value SHALL contain the read-only tool set (Read / Glob / Grep) AND SHALL NOT contain `Write`, `Edit`, or `Bash`

#### Scenario: command_allowance maps to fine-grained Bash specifier

- **WHEN** `build_command` is called with `command_allowance: Some(["codebus","quiz","validate"])`
- **THEN** the `--allowedTools` value SHALL contain `Bash(codebus quiz validate *)` AND the `--tools` value SHALL contain bare `Bash`

#### Scenario: Argv equals pre-refactor builder except the session-persistence flag

- **WHEN** a `SpawnSpec` is constructed for a goal spawn (`verb: Goal, permission: Workspace`, model/effort resolved)
- **THEN** the argv produced by `ClaudeBackend::build_command` SHALL equal, token-for-token, the argv the pre-refactor `build_claude_cmd` produced for the equivalent `InvokeAgentOptions` with the single addition of the `--no-session-persistence` flag

#### Scenario: Resume id placed before toolset flags

- **WHEN** `build_command` is called with `resume_session_id: Some("abc-123")`
- **THEN** `--resume abc-123` SHALL appear in the argv before the `--tools` flag

#### Scenario: Single-shot verbs include no-session-persistence

- **WHEN** `build_command` is called for a `SpawnSpec` with `verb` in {`Goal`, `Query`, `Fix`, `Quiz`}
- **THEN** the produced argv SHALL include the `--no-session-persistence` flag

#### Scenario: Chat verb omits no-session-persistence so resume works

- **WHEN** `build_command` is called for a `SpawnSpec` with `verb: Chat` and `resume_session_id: Some("abc-123")`
- **THEN** the produced argv SHALL NOT include `--no-session-persistence` AND SHALL include `--resume abc-123`
