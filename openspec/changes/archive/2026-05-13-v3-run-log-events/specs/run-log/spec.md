## MODIFIED Requirements

### Requirement: RunLog Schema and Per-Invocation Capture

The system SHALL capture one `RunLog` entry per verb invocation (`goal` / `query` / `fix`), including invocations that terminate via the cancel signal. The `RunLog` struct SHALL contain exactly these fields with these semantics:

- `goal` (String) — the user-facing input that triggered the invocation: the goal text for `goal`, the query text for `query`, the literal empty string `""` for `fix` (which has no positional argument)
- `mode` (String) — one of `"goal"` / `"query"` / `"fix"`, identifying the verb
- `model` (Option<String>) — the resolved `claude_code.<verb>.model` config value, or `None` when the user disabled it via config
- `effort` (Option<String>) — the resolved `claude_code.<verb>.effort` config value, or `None`
- `started_at` (String, RFC 3339 UTC, e.g., `2026-05-10T03:25:11Z`) — captured immediately before spawn
- `finished_at` (String, RFC 3339 UTC) — captured immediately after `child.wait()` returns
- `tokens` (TokenUsage) — accumulated from every `Usage` event emitted by the agent during the invocation; zero when no `result` event was received (e.g., agent crash mid-stream)
- `wiki_changed` (bool) — whether `<vault>/wiki/` byte-content differs from `HEAD~1` per the nested git repo, computed via `git -C <vault> diff --quiet HEAD~1 -- wiki/` (true when diff exit code is 1)
- `lint_error_count` (usize) — error count from the post-spawn lint (for `goal` / `fix`); 0 for `query` and when no post-spawn lint runs
- `lint_warn_count` (usize) — warning count, same semantics
- `outcome` (String) — one of `"succeeded"` / `"failed"` / `"cancelled"`. `"succeeded"` SHALL be the value when the agent terminated with exit code zero AND any post-spawn lint phase completed AND any auto-commit step succeeded. `"failed"` SHALL be the value when the agent terminated with non-zero exit AND the verb propagated that failure. `"cancelled"` SHALL be the value when the run terminated because the caller-supplied cancel signal was observed flipped to `true` during the run — the verb function SHALL write this `RunLog` entry BEFORE returning `Err(VerbError::Cancelled)` and SHALL still skip auto-commit per the existing cancel contract

The system SHALL serialize `RunLog` as a single JSON object on one line. `Option` fields with `None` SHALL be omitted from the serialized form (`#[serde(skip_serializing_if = "Option::is_none")]`). The `tokens.extras` field SHALL be omitted when null. The `outcome` field SHALL use `#[serde(default = "default_outcome")]` where `default_outcome()` returns `"succeeded"` so existing pre-v3-run-log-events jsonl rows that lack the field deserialize cleanly to `outcome: "succeeded"`.

The system SHALL write the entry by calling `LogSink::write_run` on the configured sink immediately before the `Done` banner on the success path, OR immediately before returning `Err(VerbError::Cancelled)` on the cancel path, OR immediately before propagating a non-zero exit code on the failure path.

#### Scenario: Goal RunLog records goal text and goal mode

- **WHEN** `codebus goal "describe auth"` runs to completion
- **THEN** the appended `RunLog` SHALL have `goal == "describe auth"` AND `mode == "goal"`

#### Scenario: Query RunLog records empty wiki_changed and zero lint counts

- **WHEN** `codebus query "what does X do"` runs to completion (query is read-only)
- **THEN** the appended `RunLog` SHALL have `wiki_changed == false` AND `lint_error_count == 0` AND `lint_warn_count == 0`

#### Scenario: Fix RunLog uses empty goal string

- **WHEN** `codebus fix` runs to completion
- **THEN** the appended `RunLog` SHALL have `goal == ""` AND `mode == "fix"`

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

#### Scenario: Failed verb run records outcome failed

- **WHEN** `codebus goal "X"` runs the agent to a non-zero exit code AND the verb propagates the failure to its return value
- **THEN** the appended `RunLog.outcome` SHALL equal the string `"failed"`

#### Scenario: Legacy jsonl row without outcome field deserializes cleanly

- **WHEN** a legacy `runs-YYYY-MM-DD.jsonl` row written before v3-run-log-events shipped (no `outcome` key in the JSON) is parsed via `serde_json::from_str::<RunLog>`
- **THEN** the parse SHALL succeed AND the resulting `RunLog.outcome` SHALL equal the string `"succeeded"` (the serde default value)

##### Example: serialized RunLog for a successful goal

- **GIVEN** a `goal` invocation with text `describe auth`, model `opus`, effort `high`, that emits one result event with usage `{input_tokens: 100, output_tokens: 50}` and produces 0 lint errors / 1 warning, modifying the wiki
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain at minimum the fields `"goal":"describe auth"`, `"mode":"goal"`, `"model":"opus"`, `"effort":"high"`, `"tokens":{"input_tokens":100,"output_tokens":50,...}`, `"wiki_changed":true`, `"lint_error_count":0`, `"lint_warn_count":1`, `"outcome":"succeeded"` AND SHALL parse cleanly via `serde_json::from_str`

##### Example: serialized RunLog for a cancelled goal

- **GIVEN** a `goal` invocation with text `describe X`, that was cancelled after 3 stream events with accumulated `tokens.input_tokens: 25, output_tokens: 10` and no wiki modifications yet
- **WHEN** the entry is serialized to a single JSON line
- **THEN** the JSON SHALL contain `"outcome":"cancelled"` AND `"wiki_changed":false` AND the row SHALL appear in the same `runs-YYYY-MM-DD.jsonl` file as a successful run from the same day
