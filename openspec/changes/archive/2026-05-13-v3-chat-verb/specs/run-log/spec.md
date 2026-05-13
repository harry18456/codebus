## MODIFIED Requirements

### Requirement: RunLog Schema and Per-Invocation Capture

The system SHALL capture one `RunLog` entry per verb invocation (`goal` / `query` / `fix` / `chat`), including invocations that terminate via the cancel signal. For the `chat` verb, "one invocation" corresponds to one call to `run_chat_turn` (i.e. one turn of the chat REPL), so a multi-turn chat session produces multiple `RunLog` entries — one per turn, each sharing the same `session_id` value.

The `RunLog` struct SHALL contain exactly these fields with these semantics:

- `goal` (String) — the user-facing input that triggered the invocation: the goal text for `goal`, the query text for `query`, the literal empty string `""` for `fix` (which has no positional argument), the per-turn user prompt text for `chat`
- `mode` (String) — one of `"goal"` / `"query"` / `"fix"` / `"chat"`, identifying the verb
- `model` (Option<String>) — the resolved `claude_code.<verb>.model` config value, or `None` when the user disabled it via config
- `effort` (Option<String>) — the resolved `claude_code.<verb>.effort` config value, or `None`
- `started_at` (String, RFC 3339 UTC, e.g., `2026-05-10T03:25:11Z`) — captured immediately before spawn
- `finished_at` (String, RFC 3339 UTC) — captured immediately after `child.wait()` returns
- `tokens` (TokenUsage) — accumulated from every `Usage` event emitted by the agent during the invocation; zero when no `result` event was received (e.g., agent crash mid-stream)
- `wiki_changed` (bool) — whether `<vault>/wiki/` byte-content differs from `HEAD~1` per the nested git repo, computed via `git -C <vault> diff --quiet HEAD~1 -- wiki/` (true when diff exit code is 1). For `chat` invocations this field SHALL always be `false` because chat is read-only (no Write/Edit tools available at the binary layer)
- `lint_error_count` (usize) — error count from the post-spawn lint (for `goal` / `fix`); 0 for `query` and `chat` and when no post-spawn lint runs
- `lint_warn_count` (usize) — warning count, same semantics
- `outcome` (String) — one of `"succeeded"` / `"failed"` / `"cancelled"`. `"succeeded"` SHALL be the value when the agent terminated with exit code zero AND any post-spawn lint phase completed AND any auto-commit step succeeded. `"failed"` SHALL be the value when the agent terminated with non-zero exit AND the verb propagated that failure. `"cancelled"` SHALL be the value when the run terminated because the caller-supplied cancel signal was observed flipped to `true` during the run — the verb function SHALL write this `RunLog` entry BEFORE returning `Err(VerbError::Cancelled)` and SHALL still skip auto-commit per the existing cancel contract
- `session_id` (Option<String>) — the Claude CLI session identifier for the spawned `claude` child process, extracted from the spawn's first `init` stream event. For `chat` invocations this field SHALL always be `Some(<session_id>)` (every chat turn spawns through `agent::invoke` and the init event always emits a `session_id`). For `goal`, `query`, and `fix` invocations this field SHALL always be `None` because these verbs do not currently expose session resume to the user (the field is reserved for future expansion if any of these verbs grows multi-turn behavior)

The system SHALL serialize `RunLog` as a single JSON object on one line. `Option` fields with `None` SHALL be omitted from the serialized form (`#[serde(skip_serializing_if = "Option::is_none")]`) — this applies uniformly to `model`, `effort`, and `session_id`. The `tokens.extras` field SHALL be omitted when null. The `outcome` field SHALL use `#[serde(default = "default_outcome")]` where `default_outcome()` returns `"succeeded"` so existing pre-v3-run-log-events jsonl rows that lack the field deserialize cleanly to `outcome: "succeeded"`. The `session_id` field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so legacy jsonl rows that predate this change deserialize cleanly to `session_id: None`.

The system SHALL write the entry by calling `LogSink::write_run` on the configured sink immediately before the `Done` banner on the success path, OR immediately before returning `Err(VerbError::Cancelled)` on the cancel path, OR immediately before propagating a non-zero exit code on the failure path. For `chat`, the per-turn `RunLog` SHALL be written within `run_chat_turn` itself once per turn (no aggregate session-level entry).

#### Scenario: Goal RunLog records goal text and goal mode

- **WHEN** `codebus goal "describe auth"` runs to completion
- **THEN** the appended `RunLog` SHALL have `goal == "describe auth"` AND `mode == "goal"` AND the serialized JSON SHALL NOT contain a `session_id` field

#### Scenario: Query RunLog records empty wiki_changed and zero lint counts

- **WHEN** `codebus query "what does X do"` runs to completion (query is read-only)
- **THEN** the appended `RunLog` SHALL have `wiki_changed == false` AND `lint_error_count == 0` AND `lint_warn_count == 0` AND the serialized JSON SHALL NOT contain a `session_id` field

#### Scenario: Fix RunLog uses empty goal string

- **WHEN** `codebus fix` runs to completion
- **THEN** the appended `RunLog` SHALL have `goal == ""` AND `mode == "fix"` AND the serialized JSON SHALL NOT contain a `session_id` field

#### Scenario: Chat RunLog records session_id and chat mode per turn

- **WHEN** a single chat turn completes successfully with user prompt `"what does X do?"` AND the spawned agent's first init event carries `session_id: "abc-123"`
- **THEN** the appended `RunLog` SHALL have `goal == "what does X do?"` AND `mode == "chat"` AND `session_id == Some("abc-123")` AND `wiki_changed == false` AND `lint_error_count == 0` AND `lint_warn_count == 0`

#### Scenario: Chat REPL with three turns appends three RunLog entries with the same session_id

- **WHEN** a `codebus chat` REPL session runs three turns successfully against the same vault
- **THEN** the run-log jsonl SHALL gain exactly three new lines AND all three SHALL have `mode == "chat"` AND all three `session_id` values SHALL be `Some(<same session_id>)`

#### Scenario: Tokens populated when agent emits result event

- **WHEN** the agent emits a `result` event with `usage.input_tokens=100, output_tokens=50`
- **THEN** the appended `RunLog.tokens.input_tokens` SHALL equal 100 AND `RunLog.tokens.output_tokens` SHALL equal 50

#### Scenario: Tokens zero when agent crashes before result event

- **WHEN** the agent exits non-zero before emitting any `result` event
- **THEN** the system SHALL still write a `RunLog` entry AND `RunLog.tokens.input_tokens` SHALL equal 0 AND `RunLog.mode` reflects the verb attempted

#### Scenario: started_at and finished_at bracket the agent run

- **WHEN** the verb runs for any non-zero duration
- **THEN** the appended `RunLog.finished_at` SHALL be greater than or equal to `RunLog.started_at` when both are parsed as RFC 3339

#### Scenario: Successful goal run records outcome succeeded

- **WHEN** `codebus goal "X"` runs the agent to exit zero AND the post-spawn fix-and-lint phase completes AND auto-commit succeeds
- **THEN** the appended `RunLog.outcome` SHALL equal the string `"succeeded"`

#### Scenario: Cancelled goal run records outcome cancelled and writes RunLog before returning Err

- **WHEN** `verb::goal::run_goal` is invoked with a cancel flag AND the flag is flipped to `true` mid-stream AND the verb function observes the cancel
- **THEN** the verb function SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "cancelled"` AND `tokens` reflect the accumulated partial token usage AND `wiki_changed` reflects whether any wiki files were modified before the cancel, AND THEN the verb function SHALL return `Err(VerbError::Cancelled)` AND SHALL NOT invoke `git::auto_commit`

#### Scenario: Cancelled chat turn records outcome cancelled with session_id

- **WHEN** `run_chat_turn` is invoked with a cancel flag AND the flag is flipped to `true` mid-stream AND the init event already carried `session_id: "abc-123"` AND the cancel is observed
- **THEN** the verb function SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "cancelled"` AND `mode == "chat"` AND `session_id == Some("abc-123")` BEFORE returning `Err(VerbError::Cancelled)`

#### Scenario: Failed verb run records outcome failed

- **WHEN** `codebus goal "X"` runs the agent to a non-zero exit code AND the verb propagates the failure to its return value
- **THEN** the appended `RunLog.outcome` SHALL equal the string `"failed"`

#### Scenario: Legacy jsonl row without outcome field deserializes cleanly

- **WHEN** a legacy `runs-YYYY-MM-DD.jsonl` row written before v3-run-log-events shipped (no `outcome` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.outcome` SHALL equal the string `"succeeded"` (the serde default value) AND the resulting `RunLog.session_id` SHALL equal `None`

#### Scenario: Legacy jsonl row without session_id field deserializes cleanly

- **WHEN** any jsonl row written before v3-chat-verb shipped (no `session_id` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.session_id` SHALL equal `None` AND no error SHALL be raised

##### Example: serialized RunLog for a successful chat turn

- **GIVEN** a chat turn with user prompt `what is X`, model `opus`, effort `high`, session id `abc-123`, that emits one result event with usage `{input_tokens: 100, output_tokens: 50}` and produces no wiki changes
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain at minimum the fields `"goal":"what is X"`, `"mode":"chat"`, `"model":"opus"`, `"effort":"high"`, `"tokens":{"input_tokens":100,"output_tokens":50,...}`, `"wiki_changed":false`, `"lint_error_count":0`, `"lint_warn_count":0`, `"outcome":"succeeded"`, `"session_id":"abc-123"` AND SHALL parse cleanly via `serde_json::from_str`

##### Example: serialized RunLog for a successful goal (unchanged from pre-chat-verb behavior)

- **GIVEN** a `goal` invocation with text `describe auth`, model `opus`, effort `high`, that emits one result event with usage `{input_tokens: 100, output_tokens: 50}` and produces 0 lint errors / 1 warning, modifying the wiki
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain at minimum the fields `"goal":"describe auth"`, `"mode":"goal"`, `"model":"opus"`, `"effort":"high"`, `"tokens":{"input_tokens":100,"output_tokens":50,...}`, `"wiki_changed":true`, `"lint_error_count":0`, `"lint_warn_count":1`, `"outcome":"succeeded"` AND SHALL parse cleanly via `serde_json::from_str` AND SHALL NOT contain a `"session_id"` field

##### Example: serialized RunLog for a cancelled goal (unchanged from pre-chat-verb behavior)

- **GIVEN** a `goal` invocation with text `describe X`, that was cancelled after 3 stream events with accumulated `tokens.input_tokens: 25, output_tokens: 10` and no wiki modifications yet
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain `"outcome":"cancelled"` AND `"wiki_changed":false` AND the row SHALL appear in the same `runs-YYYY-MM-DD.jsonl` file as a successful run from the same day AND SHALL NOT contain a `"session_id"` field
