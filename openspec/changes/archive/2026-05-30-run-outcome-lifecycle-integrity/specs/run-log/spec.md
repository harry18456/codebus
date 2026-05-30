## MODIFIED Requirements

### Requirement: RunLog Schema and Per-Invocation Capture

The system SHALL capture one `RunLog` entry per verb invocation (`goal` / `query` / `fix` / `chat` / `quiz`), including invocations that terminate via the cancel signal. For the `chat` verb, "one invocation" corresponds to one call to `run_chat_turn` (i.e. one turn of the chat REPL), so a multi-turn chat session produces multiple `RunLog` entries — one per turn, each sharing the same `session_id` value. For the `quiz` verb, the `RunLog` is written by the generate spawn (`run_quiz_generate`); the plan sub-step (`run_quiz_plan`) SHALL NOT write a `RunLog`, so a `codebus quiz` invocation produces exactly one `RunLog` entry — the one recorded by the generate spawn — per the `quiz` capability.

The `RunLog` struct SHALL contain exactly these fields with these semantics:

- `goal` (String) — the user-facing input that triggered the invocation: the goal text for `goal`, the query text for `query`, the literal empty string `""` for `fix` (which has no positional argument), the per-turn user prompt text for `chat`, and the comma-joined list of selected page paths (`options.pages.join(",")`) for `quiz`
- `mode` (String) — one of `"goal"` / `"query"` / `"fix"` / `"chat"` / `"quiz"`, identifying the verb
- `model` (Option<String>) — the resolved `claude_code.<verb>.model` config value, or `None` when the user disabled it via config
- `effort` (Option<String>) — the resolved `claude_code.<verb>.effort` config value, or `None`
- `started_at` (String, RFC 3339 UTC, e.g., `2026-05-10T03:25:11Z`) — captured immediately before spawn
- `finished_at` (String, RFC 3339 UTC) — captured immediately after `child.wait()` returns
- `tokens` (TokenUsage) — accumulated from every `Usage` event emitted by the agent during the invocation; zero when no `result` event was received (e.g., agent crash mid-stream)
- `wiki_changed` (bool) — whether `<vault>/wiki/` byte-content differs from `HEAD~1` per the nested git repo, computed via `git -C <vault> diff --quiet HEAD~1 -- wiki/` (true when diff exit code is 1). For `chat` invocations this field SHALL always be `false` because chat is read-only (no Write/Edit tools available at the binary layer). For `quiz` invocations this field SHALL likewise always be `false` because the generated quiz attempt is persisted outside `wiki/` (under `<vault>/.codebus/quiz/`) and the quiz flow does not modify wiki pages
- `lint_error_count` (usize) — error count from the post-spawn lint (for `goal` / `fix`); for `quiz` the count of `error`-severity findings reported by the deterministic quiz validator acting as the final verifier (per the `quiz` capability); 0 for `query` and `chat` and when no post-spawn lint runs
- `lint_warn_count` (usize) — warning count, same semantics; `0` for `quiz`, whose validator reports only `error`-severity findings
- `sandbox_denial_count` (usize) — the number of agent tool results during the invocation that both terminated non-zero (`is_error == true`) AND carried a locale-independent sandbox / permission-denial marker in their output, as accumulated by `agent::invoke` (see the `verb-library` capability `Sandbox Denial Signal Observability` requirement). This field is a best-effort observability signal for the case where a provider (notably codex `exec`) exits zero at the top level even though an inner shell command was blocked by the OS sandbox. It SHALL be `0` for the overwhelmingly common case (no denial markers observed). It is orthogonal to `outcome`: a non-zero `sandbox_denial_count` SHALL NOT by itself change `outcome`. Serde SHALL serialize this field with `#[serde(default)]` AND SHALL omit it from the serialized JSON when its value is `0`, so existing rows from non-codex or clean runs remain byte-identical and legacy jsonl rows that predate this change deserialize cleanly to `sandbox_denial_count: 0`
- `outcome` (String) — one of `"succeeded"` / `"failed"` / `"cancelled"`. `"succeeded"` SHALL be the value when the agent terminated with exit code zero AND any post-spawn lint phase completed AND any auto-commit step succeeded. `"failed"` SHALL be the value when the agent terminated with non-zero exit AND the verb propagated that failure, OR when the run was terminated by the per-run wall-clock timeout (see the `verb-library` capability `Run Wall-Clock Timeout Safety Net` requirement). `"cancelled"` SHALL be the value when the run terminated because the caller-supplied cancel signal was observed flipped to `true` during the run — the verb function SHALL write this `RunLog` entry BEFORE returning `Err(VerbError::Cancelled)` and SHALL still skip auto-commit per the existing cancel contract
- `session_id` (Option<String>) — the Claude CLI session identifier for the spawned `claude` child process, extracted from the spawn's first `init` stream event. For `chat` invocations this field SHALL always be `Some(<session_id>)` (every chat turn spawns through `agent::invoke` and the init event always emits a `session_id`). For `quiz` invocations this field SHALL carry the generate spawn's `session_id` — `Some(<session_id>)` when the generate spawn's `init` event was observed — but unlike `chat` the value is recorded for logging only and is NOT used to resume any session. For `goal`, `query`, and `fix` invocations this field SHALL always be `None` because these verbs do not currently expose session resume to the user (the field is reserved for future expansion if any of these verbs grows multi-turn behavior)
- `interrupt_reason` (Option<InterruptReason>) — a classifying tag for why the run did not reach the success path, intended for the GUI Interrupted detail view (see `app-workspace` spec). The system SHALL define `InterruptReason` as an enum with five variants: `AppClose` (process exited before the verb returned, typically detected at next launch from an orphan events jsonl file), `UserCancel` (the caller-supplied cancel signal flipped to `true` during the run), `NetworkDrop` (an external connection error caused the verb to abort), `Timeout` (the per-run wall-clock limit elapsed and `agent::invoke` terminated the agent process tree — see the `verb-library` capability `Run Wall-Clock Timeout Safety Net` requirement), and `Other(String)` (free-form fallback for future classifications not yet promoted to a named variant). Serde SHALL serialize the enum with `#[serde(rename_all = "kebab-case")]`, producing JSON literals `"app-close"`, `"user-cancel"`, `"network-drop"`, `"timeout"`, and the `Other` variant as the untagged object form `{"other": "<string>"}`. The field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so legacy jsonl rows written before this change deserialize cleanly to `interrupt_reason: None` and rows that never carry a reason (e.g., normal succeeded runs) SHALL NOT emit the key. This change does NOT alter the closed set of `outcome` string values; `interrupt_reason` is an orthogonal classifier, populated only when the verb layer or GUI synthesizer has a reason to set it.

The system SHALL serialize `RunLog` as a single JSON object on one line. `Option` fields with `None` SHALL be omitted from the serialized form (`#[serde(skip_serializing_if = "Option::is_none")]`) — this applies uniformly to `model`, `effort`, `session_id`, and `interrupt_reason`. The `tokens.extras` field SHALL be omitted when null. The `sandbox_denial_count` field SHALL use `#[serde(default)]` AND SHALL be omitted from the serialized form when its value is `0` (via a `skip_serializing_if` zero-check helper), so existing rows from non-codex or clean runs remain byte-identical. The `outcome` field SHALL use `#[serde(default = "default_outcome")]` where `default_outcome()` returns `"succeeded"` so existing pre-v3-run-log-events jsonl rows that lack the field deserialize cleanly to `outcome: "succeeded"`. The `session_id` field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so legacy jsonl rows that predate that change deserialize cleanly to `session_id: None`. Because `quiz` populates `session_id` with the generate spawn's id, a `quiz` RunLog's serialized JSON normally DOES contain a `session_id` field (in contrast to `goal` / `query` / `fix`, whose `None` value is omitted). The `interrupt_reason` field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so legacy jsonl rows that predate this change deserialize cleanly to `interrupt_reason: None`.

The system SHALL write the entry by calling `LogSink::write_run` on the configured sink immediately before the `Done` banner on the success path, OR immediately before returning `Err(VerbError::Cancelled)` on the cancel path, OR immediately before propagating a non-zero exit code on the failure path. For `chat`, the per-turn `RunLog` SHALL be written within `run_chat_turn` itself once per turn (no aggregate session-level entry). For `quiz`, the `RunLog` SHALL be written within `run_quiz_generate` after the generate spawn and the deterministic validator complete; `run_quiz_plan` SHALL NOT write a `RunLog` (planning is a sub-step), per the `quiz` capability.

#### Scenario: Goal RunLog records goal text and goal mode

- **WHEN** `codebus goal "describe auth"` runs to completion
- **THEN** the appended `RunLog` SHALL have `goal == "describe auth"` AND `mode == "goal"` AND the serialized JSON SHALL NOT contain a `session_id` field AND the serialized JSON SHALL NOT contain an `interrupt_reason` field

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

#### Scenario: Quiz RunLog records pages-joined goal and quiz mode

- **WHEN** `run_quiz_generate` runs to completion over selected pages `["wiki/modules/auth.md", "wiki/processes/login.md"]` AND the generate spawn's first init event carries `session_id: "quiz-sid-1"`
- **THEN** the appended `RunLog` SHALL have `mode == "quiz"` AND `goal == "wiki/modules/auth.md,wiki/processes/login.md"` AND `session_id == Some("quiz-sid-1")` AND `wiki_changed == false`

#### Scenario: Quiz plan sub-step writes no RunLog

- **WHEN** `run_quiz_plan` runs to completion (the planning sub-step that resolves the page scope, with no generate spawn)
- **THEN** no new `RunLog` entry SHALL be appended for the plan sub-step AND the only `quiz`-mode `RunLog` for the invocation SHALL be the one written by `run_quiz_generate`

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
- **THEN** the appended `RunLog.outcome` SHALL equal the string `"succeeded"` AND the serialized JSON SHALL NOT contain an `interrupt_reason` field

#### Scenario: Cancelled goal run records outcome cancelled and writes RunLog before returning Err

- **WHEN** `verb::goal::run_goal` is invoked with a cancel flag AND the flag is flipped to `true` mid-stream AND the verb function observes the cancel
- **THEN** the verb function SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "cancelled"` AND `tokens` reflect the accumulated partial token usage AND `wiki_changed` reflects whether any wiki files were modified before the cancel, AND THEN the verb function SHALL return `Err(VerbError::Cancelled)` AND SHALL NOT invoke `git::auto_commit`

#### Scenario: Cancelled chat turn records outcome cancelled with session_id

- **WHEN** `run_chat_turn` is invoked with a cancel flag AND the flag is flipped to `true` mid-stream AND the init event already carried `session_id: "abc-123"` AND the cancel is observed
- **THEN** the verb function SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "cancelled"` AND `mode == "chat"` AND `session_id == Some("abc-123")` BEFORE returning `Err(VerbError::Cancelled)`

#### Scenario: Failed verb run records outcome failed

- **WHEN** `codebus goal "X"` runs the agent to a non-zero exit code AND the verb propagates the failure to its return value
- **THEN** the appended `RunLog.outcome` SHALL equal the string `"failed"`

#### Scenario: Timed-out goal run records outcome failed with timeout interrupt_reason

- **WHEN** `codebus goal "X"` is invoked with a per-run wall-clock timeout AND the agent run exceeds that limit AND `agent::invoke` terminates the agent process tree AND the cancel signal was never flipped
- **THEN** the appended `RunLog.outcome` SHALL equal the string `"failed"` AND `RunLog.interrupt_reason` SHALL equal `Some(InterruptReason::Timeout)` AND the serialized JSON SHALL contain the substring `"interrupt_reason":"timeout"`

#### Scenario: Legacy jsonl row without outcome field deserializes cleanly

- **WHEN** a legacy `runs-YYYY-MM-DD.jsonl` row written before v3-run-log-events shipped (no `outcome` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.outcome` SHALL equal the string `"succeeded"` (the serde default value) AND the resulting `RunLog.session_id` SHALL equal `None` AND the resulting `RunLog.interrupt_reason` SHALL equal `None` AND the resulting `RunLog.sandbox_denial_count` SHALL equal `0`

#### Scenario: Legacy jsonl row without session_id field deserializes cleanly

- **WHEN** any jsonl row written before v3-chat-verb shipped (no `session_id` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.session_id` SHALL equal `None` AND no error SHALL be raised

#### Scenario: Legacy jsonl row without interrupt_reason field deserializes cleanly

- **WHEN** any jsonl row written before this change shipped (no `interrupt_reason` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.interrupt_reason` SHALL equal `None` AND no error SHALL be raised

#### Scenario: Legacy jsonl row without sandbox_denial_count field deserializes cleanly

- **WHEN** any jsonl row written before this change shipped (no `sandbox_denial_count` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.sandbox_denial_count` SHALL equal `0` AND no error SHALL be raised

#### Scenario: RunLog with interrupt_reason UserCancel serializes to kebab-case string literal

- **WHEN** a `RunLog` with `interrupt_reason: Some(InterruptReason::UserCancel)` is serialized to a single JSON line via `serde_json::to_string`
- **THEN** the JSON SHALL contain the substring `"interrupt_reason":"user-cancel"` AND SHALL round-trip via `serde_json::from_str::<RunLog>` to a structurally equal value

#### Scenario: RunLog with interrupt_reason Timeout serializes to kebab-case string literal

- **WHEN** a `RunLog` with `interrupt_reason: Some(InterruptReason::Timeout)` is serialized to a single JSON line via `serde_json::to_string`
- **THEN** the JSON SHALL contain the substring `"interrupt_reason":"timeout"` AND SHALL round-trip via `serde_json::from_str::<RunLog>` to a structurally equal value

#### Scenario: RunLog with interrupt_reason Other serializes to object form

- **WHEN** a `RunLog` with `interrupt_reason: Some(InterruptReason::Other("agent-crash".into()))` is serialized to a single JSON line
- **THEN** the JSON SHALL contain the substring `"interrupt_reason":{"other":"agent-crash"}` AND SHALL round-trip via `serde_json::from_str::<RunLog>` to a structurally equal value

#### Scenario: RunLog with zero sandbox_denial_count omits the field

- **WHEN** a `RunLog` with `sandbox_denial_count: 0` is serialized to a single JSON line via `serde_json::to_string`
- **THEN** the serialized JSON SHALL NOT contain a `sandbox_denial_count` field

#### Scenario: RunLog with non-zero sandbox_denial_count serializes the field

- **WHEN** a `RunLog` with `sandbox_denial_count: 2` and `outcome: "succeeded"` is serialized to a single JSON line via `serde_json::to_string`
- **THEN** the serialized JSON SHALL contain the substring `"sandbox_denial_count":2` AND the `outcome` value SHALL remain `"succeeded"` (the denial count does not alter outcome) AND the row SHALL round-trip via `serde_json::from_str::<RunLog>` to a structurally equal value

##### Example: serialized RunLog for a successful chat turn

- **GIVEN** a chat turn with user prompt `what is X`, model `opus`, effort `high`, session id `abc-123`, that emits one result event with usage `{input_tokens: 100, output_tokens: 50}` and produces no wiki changes
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain at minimum the fields `"goal":"what is X"`, `"mode":"chat"`, `"model":"opus"`, `"effort":"high"`, `"tokens":{"input_tokens":100,"output_tokens":50,...}`, `"wiki_changed":false`, `"lint_error_count":0`, `"lint_warn_count":0`, `"outcome":"succeeded"`, `"session_id":"abc-123"` AND SHALL NOT contain an `"interrupt_reason"` field AND SHALL NOT contain a `"sandbox_denial_count"` field AND SHALL parse cleanly via `serde_json::from_str`

##### Example: serialized RunLog for a successful goal (unchanged from pre-chat-verb behavior)

- **GIVEN** a `goal` invocation with text `describe auth`, model `opus`, effort `high`, that emits one result event with usage `{input_tokens: 100, output_tokens: 50}` and produces 0 lint errors / 1 warning, modifying the wiki
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain at minimum the fields `"goal":"describe auth"`, `"mode":"goal"`, `"model":"opus"`, `"effort":"high"`, `"tokens":{"input_tokens":100,"output_tokens":50,...}`, `"wiki_changed":true`, `"lint_error_count":0`, `"lint_warn_count":1`, `"outcome":"succeeded"` AND SHALL NOT contain a `"session_id"` field AND SHALL NOT contain an `"interrupt_reason"` field AND SHALL NOT contain a `"sandbox_denial_count"` field AND SHALL parse cleanly via `serde_json::from_str`

##### Example: serialized RunLog for a successful quiz generate

- **GIVEN** a `quiz` generate over selected pages `["wiki/modules/auth.md"]`, model `haiku`, effort `low`, generate-spawn session id `quiz-sid-1`, that emits one result event with usage `{input_tokens: 200, output_tokens: 80}`, produces no wiki changes, and passes the deterministic validator with 0 error findings
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain at minimum the fields `"goal":"wiki/modules/auth.md"`, `"mode":"quiz"`, `"model":"haiku"`, `"effort":"low"`, `"tokens":{"input_tokens":200,"output_tokens":80,...}`, `"wiki_changed":false`, `"lint_error_count":0`, `"lint_warn_count":0`, `"outcome":"succeeded"`, `"session_id":"quiz-sid-1"` AND (because `session_id` is `Some`) SHALL contain a `"session_id"` field AND SHALL NOT contain an `"interrupt_reason"` field AND SHALL parse cleanly via `serde_json::from_str`

##### Example: serialized RunLog for a cancelled goal with user-cancel reason

- **GIVEN** a `goal` invocation with text `describe X`, that was cancelled after 3 stream events with accumulated `tokens.input_tokens: 25, output_tokens: 10`, no wiki modifications, and `interrupt_reason: Some(InterruptReason::UserCancel)`
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain `"outcome":"cancelled"` AND `"wiki_changed":false` AND `"interrupt_reason":"user-cancel"` AND the row SHALL appear in the same `runs-YYYY-MM-DD.jsonl` file as a successful run from the same day AND SHALL NOT contain a `"session_id"` field

##### Example: interrupt_reason variants serialize to the expected JSON shapes

| InterruptReason variant            | Serialized form inside RunLog JSON                |
| ---------------------------------- | ------------------------------------------------- |
| `Some(AppClose)`                   | `"interrupt_reason":"app-close"`                  |
| `Some(UserCancel)`                 | `"interrupt_reason":"user-cancel"`                |
| `Some(NetworkDrop)`                | `"interrupt_reason":"network-drop"`               |
| `Some(Timeout)`                    | `"interrupt_reason":"timeout"`                    |
| `Some(Other("agent-crash".into()))` | `"interrupt_reason":{"other":"agent-crash"}`     |
| `None`                             | (field omitted from the serialized JSON entirely) |
