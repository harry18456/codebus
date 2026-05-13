# run-log Specification

## Purpose

TBD - created by archiving change 'v3-run-log'. Update Purpose after archive.

## Requirements

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

---
### Requirement: LogSink Trait and Implementations

The system SHALL define an object-safe `LogSink` trait with three methods: `name(&self) -> &str` returning a stable identifier, `write_run(&mut self, entry: &RunLog) -> Result<(), LogError>` persisting one entry, and `flush(&mut self) -> Result<(), LogError>` with a default no-op body for sinks without internal buffers. The system SHALL provide two implementations:

- **NullSink** — stable name `"null"`. `write_run` returns `Ok(())` without I/O. Used as the user-facing opt-out.
- **JsonlSink** — stable name `"jsonl"`. `write_run` SHALL append the serialized entry plus a trailing newline byte to `<dir>/runs-YYYY-MM-DD.jsonl`, where `<dir>` is the directory passed at construction and `YYYY-MM-DD` is the first 10 characters of `entry.started_at` (RFC 3339 prefix). The sink SHALL `create_dir_all(<dir>)` lazily on each `write_run` call. The file SHALL be opened with `OpenOptions::new().append(true).create(true).open(...)` so concurrent writes from multiple processes are line-wise atomic on POSIX (best-effort on Windows).

The system SHALL define a tagged-enum `SinkConfig` with two variants `Null {}` and `Jsonl { dir: Option<PathBuf> }`. The default value of `SinkConfig` SHALL be `Jsonl { dir: None }` (caller resolves the `None` to `<vault>/.codebus/log/`). A `build_sink(cfg)` factory SHALL construct the corresponding `LogSink` instance and SHALL return an `Err(SinkError::Setup(...))` when `Jsonl` is supplied with `dir: None` (the resolution is the caller's responsibility, not the factory's).

#### Scenario: NullSink write_run is a successful no-op

- **WHEN** the caller invokes `NullSink::new().write_run(&entry)`
- **THEN** the call SHALL return `Ok(())` AND no filesystem state SHALL change

#### Scenario: JsonlSink writes one JSON line plus newline

- **WHEN** `JsonlSink::new(dir).write_run(&entry)` is called for an `entry` with `started_at` beginning `"2026-05-10T..."`
- **THEN** the file `<dir>/runs-2026-05-10.jsonl` SHALL exist AND its contents SHALL end with one valid JSON line for `entry` followed by `\n`

#### Scenario: JsonlSink date rotation by started_at

- **WHEN** two entries are written in succession, the first with `started_at` of `"2026-05-10T23:55:00Z"` and the second `"2026-05-11T00:05:00Z"`
- **THEN** the first entry SHALL appear in `runs-2026-05-10.jsonl` AND the second SHALL appear in `runs-2026-05-11.jsonl` (split by `started_at` date prefix, NOT by file-system clock at write time)

#### Scenario: JsonlSink creates directory lazily on first write

- **WHEN** `JsonlSink::new("/tmp/no/such/dir").write_run(&entry)` is called and `/tmp/no/such/dir/` does not yet exist
- **THEN** the directory SHALL be created via `create_dir_all` AND the write SHALL succeed

#### Scenario: build_sink rejects Jsonl with unresolved dir

- **WHEN** `build_sink(SinkConfig::Jsonl { dir: None })` is called
- **THEN** the result SHALL be `Err(SinkError::Setup(_))` whose message references the `dir` field

#### Scenario: build_sink dispatches to Null and Jsonl correctly

- **WHEN** `build_sink(SinkConfig::Null {})` is called
- **THEN** the returned trait object's `name()` SHALL equal `"null"`

- **WHEN** `build_sink(SinkConfig::Jsonl { dir: Some(...) })` is called with a real path
- **THEN** the returned trait object's `name()` SHALL equal `"jsonl"`

---
### Requirement: Log Configuration Schema

The system SHALL load log sink configuration from `~/.codebus/config.yaml` under the top-level key `log`. The schema SHALL define exactly two fields under the active variant: `sink` (string discriminator with values `"jsonl"` or `"null"`, default `"jsonl"`) and `dir` (optional string path, default `None` — caller resolves to `<vault>/.codebus/log/`). When the file is missing, the `log` section is absent, or the `sink` field is absent, the system SHALL apply the `Jsonl { dir: None }` default. Unknown keys inside the `log` section SHALL be silently ignored (forward-compat). When the `sink` value is unknown (e.g., `sink: otel`) the loader SHALL return a parse error and the caller SHALL fall back to the default after emitting a stderr warning prefixed with `warning: log config`.

The user-facing opt-out form SHALL be the literal string `none` (`sink: none` → `SinkConfig::Null {}`). The bare YAML null literal (`sink: null`) SHALL NOT match the opt-out variant — this aligns with the `pii.scanner: none` foot-gun avoidance pattern shipped in v3-config: a typo'd `null` returns a parse error and the caller falls back to the default with a stderr warning, surfacing the typo instead of silently doing the wrong thing.

#### Scenario: Default config when file missing

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the loaded `LogConfig.sink` SHALL equal `SinkConfig::Jsonl { dir: None }`

#### Scenario: Explicit null sink opts out

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: none\n`
- **THEN** the loaded `LogConfig.sink` SHALL equal `SinkConfig::Null {}`

#### Scenario: Custom dir path is honored

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: jsonl\n  dir: /var/log/codebus\n`
- **THEN** the loaded `LogConfig.sink` SHALL equal `SinkConfig::Jsonl { dir: Some(PathBuf::from("/var/log/codebus")) }`

#### Scenario: Bare YAML null in sink position returns parse error

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: null\n` (the YAML null literal, NOT the quoted string `"null"`)
- **THEN** the loader SHALL return `Err(ConfigLoadError::YamlParse(_))` AND the caller SHALL fall back to the default after emitting a stderr warning prefixed with `warning: log config` — equivalent UX to the unknown-discriminator case (user gets the default behavior; the warning surfaces the typo)

#### Scenario: Unknown sink discriminator returns parse error

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: otel\n`
- **THEN** the loader SHALL return `Err(ConfigLoadError::YamlParse(_))` AND the caller SHALL fall back to the default after emitting a stderr warning prefixed with `warning: log config`

#### Scenario: Unknown subkey silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: jsonl\n  retention_days: 30\n`
- **THEN** the loader SHALL succeed AND the `retention_days` field SHALL have no observable effect

---
### Requirement: Default Log Directory Resolution

When `LogConfig.sink == Jsonl { dir: None }`, the verb command (caller) SHALL resolve the directory to `<vault>/.codebus/log/` before constructing the `JsonlSink`. The vault root SHALL be derived from the verb's resolved repo path via `vault_paths(repo).log`. When `dir` is `Some(path)`, the caller SHALL pass `path` to `JsonlSink::new` verbatim (no further resolution), supporting absolute paths, paths with leading `~/` (which the caller SHALL expand to the home directory before passing through), and relative paths (resolved relative to the verb's resolved repo path).

#### Scenario: None resolves to vault-local log directory

- **WHEN** the verb resolves `LogConfig.sink == Jsonl { dir: None }` against repo `/repo`
- **THEN** the constructed `JsonlSink` SHALL target the directory `/repo/.codebus/log/`

#### Scenario: Tilde path expanded against home directory

- **WHEN** the verb resolves `dir: Some(PathBuf::from("~/codebus-history"))` and the home directory is `/home/harry`
- **THEN** the constructed `JsonlSink` SHALL target `/home/harry/codebus-history/`

#### Scenario: Absolute path used verbatim

- **WHEN** the verb resolves `dir: Some(PathBuf::from("/var/log/codebus"))`
- **THEN** the constructed `JsonlSink` SHALL target `/var/log/codebus/` exactly

---
### Requirement: RunLog Write Failure Is Non-Fatal

When `LogSink::write_run` returns an error, the verb SHALL emit a stderr warning prefixed with `warning: run-log` describing the error and SHALL continue to its normal exit code path (the agent's exit code, the lint/fix exit code, or the auto-commit exit code). The verb SHALL NOT propagate the log write failure into its own exit code. This behavior preserves the contract that logging is best-effort: a disk-full / permission-denied / locked-file failure on the log path MUST NOT fail an otherwise successful goal / query / fix run.

#### Scenario: JsonlSink IO error becomes warning, exit code unchanged

- **WHEN** the verb runs to a successful agent termination AND the configured `JsonlSink` cannot write (e.g., the target directory's parent is read-only)
- **THEN** stderr SHALL contain a line beginning with `warning: run-log` AND the verb's exit code SHALL be 0 (not 1)

#### Scenario: Missing log directory does not block verb success

- **WHEN** the verb runs against a vault whose `.codebus/log/` directory was deleted between init and the verb invocation
- **THEN** the `JsonlSink` SHALL recreate the directory via `create_dir_all` AND the verb SHALL succeed normally
