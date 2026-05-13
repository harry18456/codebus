## MODIFIED Requirements

### Requirement: Verb Library Module Surface

The system SHALL expose a public module `codebus_core::verb` containing four sub-modules `goal`, `query`, `fix`, and `chat`. Each sub-module SHALL export exactly one public orchestration function (`run_goal`, `run_query`, `run_fix`, `run_chat_turn`) plus the verb-specific options and report structs. The `codebus_core::verb` parent module SHALL also export the cross-verb types `VerbEvent`, `VerbLifecycleEvent`, and `VerbError`. No other public surface SHALL be exposed under `codebus_core::verb` by this change. The `codebus_core::vault::init::run_init` function defined by foundation SHALL remain in its existing location and SHALL NOT be moved into `codebus_core::verb`.

#### Scenario: Verb library module path exists

- **WHEN** a downstream crate (codebus-cli or codebus-app) writes `use codebus_core::verb::{goal, query, fix, chat};`
- **THEN** the compilation SHALL succeed AND the four sub-modules SHALL resolve to public modules each exporting one public orchestration function

#### Scenario: Init verb is not moved

- **WHEN** a downstream crate writes `use codebus_core::verb::init;`
- **THEN** the compilation SHALL fail (no such module) AND init orchestration SHALL remain accessible only via `codebus_core::vault::init::run_init`

#### Scenario: Chat sub-module exports run_chat_turn

- **WHEN** a downstream crate writes `use codebus_core::verb::chat::{run_chat_turn, ChatTurnOptions, ChatTurnReport, CHAT_TOOLSET};`
- **THEN** the compilation SHALL succeed AND `run_chat_turn` SHALL resolve to a function with the signature defined by the `chat-verb` capability

---

### Requirement: Verb Event Enum

The system SHALL define `codebus_core::verb::VerbEvent` as a public enum with exactly three variants:

```
pub enum VerbEvent {
    Banner(VerbBanner),
    Stream(StreamEvent),
    Lifecycle(VerbLifecycleEvent),
}
```

`VerbBanner` SHALL be a new owning enum in `codebus_core::verb::event` that mirrors `codebus_core::render::Banner<'a>` but with owned fields (`PathBuf` instead of `&Path`, `String` instead of `&str`). The variant set SHALL match `Banner` exactly: `Start { repo_path: PathBuf }`, `Goal { goal_text: String }`, `SyncStart`, `SyncDone { files, mib, elapsed_ms }`, `PiiSummary { scanner: String, scanned, hits, action: String }`, `LintStart`, `LintDone { errors, warns, elapsed_ms }`, `CommitDone { sha7: String }`, `Done { wiki_path: PathBuf }`, `Hint { wiki_path: PathBuf }`. `VerbBanner` SHALL implement an `as_banner(&self) -> Banner<'_>` method that borrows the owning fields back into a `Banner<'_>` so CLI thin wrappers can reuse the existing `print_banner` renderer without duplicating formatting logic. The owning representation is required because `VerbEvent` flows through `impl FnMut(VerbEvent)` closures that must be `'static + Send` for cross-thread use (GUI Tauri event emit); the borrowed `Banner<'a>` cannot satisfy that constraint.

`StreamEvent` SHALL be the existing `codebus_core::stream::StreamEvent` from the `agent-stream-rendering` capability.

`VerbLifecycleEvent` SHALL be a public enum with at minimum these variants: `SpawnStart { verb: Verb }`, `SpawnEnd { verb: Verb, exit_code: Option<i32> }`, `FixIterationStart { iteration: u8 }`, `LintFinal { error_count: usize, warn_count: usize }`, AND `PromoteSuggestion { reason: String }`. The `PromoteSuggestion` variant SHALL be emitted exclusively by `verb::chat::run_chat_turn` when its stream parser detects the chat promote-suggestion line marker per the `chat-verb` capability — `run_goal`, `run_query`, and `run_fix` SHALL NOT emit this variant. Additional lifecycle variants MAY be added by future changes following minor-version semantics; downstream pattern matches SHALL be required to use a non-exhaustive marker or wildcard arm.

Each `run_*` function SHALL invoke `on_event` exactly once for each banner step it would have printed in its CLI form (wrapping each `Banner::*` as `VerbEvent::Banner(VerbBanner::*)`), exactly once for each `StreamEvent` produced by the underlying `agent::invoke` (wrapping each as `VerbEvent::Stream(...)`), and at appropriate lifecycle boundaries (wrapping each as `VerbEvent::Lifecycle(...)`).

#### Scenario: Banner events flow through on_event

- **WHEN** `run_goal` reaches the step where the CLI form would print `Banner::Start { repo_path }`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Banner(VerbBanner::Start { repo_path: PathBuf::from(repo) })` AND no direct stdout write SHALL occur from the library function

#### Scenario: VerbBanner as_banner round-trips into Banner renderer

- **WHEN** the CLI thin wrapper receives `VerbEvent::Banner(vb)` AND calls `print_banner(vb.as_banner(), &render_opts)`
- **THEN** the rendered stdout byte sequence SHALL equal what `print_banner(Banner::*, &render_opts)` would have produced when the equivalent borrowed `Banner<'a>` is built from the same field values (byte-equivalent CLI output)

#### Scenario: Stream events flow through on_event

- **WHEN** `agent::invoke` (called by `run_goal`) parses a stream-json line yielding `StreamEvent::ToolUse { name: "Read", input: ... }`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Stream(StreamEvent::ToolUse { name: "Read", input: ... })` AND no `print_event` call SHALL occur inside the library function

#### Scenario: Lifecycle events bracket spawn

- **WHEN** `run_query` is about to call `agent::invoke`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { verb: Verb::Query })` immediately before the spawn AND `VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd { verb: Verb::Query, exit_code: <exit-code> })` immediately after the child wait returns

#### Scenario: PromoteSuggestion emitted only by chat verb

- **WHEN** `run_chat_turn` is invoked AND the agent's assistant message begins with the chat promote-suggestion line marker
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Lifecycle(VerbLifecycleEvent::PromoteSuggestion { reason })` exactly once for that message; AND when `run_goal`, `run_query`, or `run_fix` is invoked, `on_event` SHALL NOT be invoked with the `PromoteSuggestion` variant under any path

---

### Requirement: Verb Error Enum

The system SHALL define `codebus_core::verb::VerbError` as a public enum with exactly these variants:

```
pub enum VerbError {
    VaultMissing { path: PathBuf },
    ConfigParse { which: &'static str, source: ConfigLoadError },
    KeyringMissing { source: KeyringError },
    Spawn { source: io::Error },
    Cancelled,
    Internal { message: String },
}
```

The enum SHALL implement `std::error::Error` (via `thiserror`) and `std::fmt::Display`. The variant semantics SHALL be:

- `VaultMissing { path }` — returned by `run_query`, `run_fix`, and `run_chat_turn` when `<repo>/.codebus/` is absent; `run_goal` SHALL auto-init instead (per Goal Verb Library Function) and SHALL NOT return this variant
- `ConfigParse { which, source }` — returned by any `run_*` when a config section yaml fails to parse. The `which` field SHALL be one of `"claude_code"`, `"lint.fix"`, `"log"`, or `"pii"` (a `&'static str` chosen by the verb function based on which loader rejected the yaml) so the CLI thin wrapper can emit section-specific stderr (`error: {which} config parse failed at {path}: {source}`) preserving byte-equivalent output
- `KeyringMissing { source }` — returned by any `run_*` when `build_env_overrides` cannot resolve the Azure profile's API key from the OS keyring + env fallback chain. Surfaced to the CLI as exit code 3, preserving the pre-refactor `error: {verb}: {source}` stderr line
- `Spawn { source }` — returned by any `run_*` when `agent::invoke` returns an `io::Result::Err` (e.g., claude binary not on PATH, fork failure)
- `Cancelled` — returned by any `run_*` when the `cancel` signal flag was observed flipped to true during the run. The `chat` verb CLI subcommand (`commands/chat.rs`) SHALL pass `cancel: Some(flag)` and observe `VerbError::Cancelled` on the user's first Ctrl+C; the `goal` / `query` / `fix` CLI thin wrappers SHALL continue to pass `cancel: None` and never observe this variant. Downstream `match` arms on `VerbError` in CLI commands SHALL handle `Cancelled` only in the chat command's branch and SHALL leave the other commands' branches as unreachable for that variant (current behavior preserved)
- `Internal { message }` — returned by any `run_*` for any other unrecoverable failure with a human-readable message

`VerbError` SHALL expose a `cli_exit_code(&self) -> u8` method that maps each variant to the per-verb exit code policy preserved by the refactor: `VaultMissing` → 2, `ConfigParse` → 2, `KeyringMissing` → 3, `Spawn` → 1, `Cancelled` → 0 (CLI never observes this — guard for completeness), `Internal` → 1. CLI thin wrappers in `codebus-cli/src/commands/{goal,query,fix}.rs` SHALL `match` exhaustively on `VerbError` to derive the exit code AND to emit the verb-specific stderr message. The exhaustive match guarantees compile-time coverage when a future variant is added.

#### Scenario: ConfigParse propagates underlying error with section label

- **WHEN** `run_goal` is invoked AND `~/.codebus/config.yaml` contains a `claude_code` section that fails yaml parsing
- **THEN** the function SHALL return `Err(VerbError::ConfigParse { which: "claude_code", source })` where `source.to_string()` SHALL contain the failing field name AND `which` SHALL equal the literal string `"claude_code"`

#### Scenario: KeyringMissing surfaces when Azure profile key is unreachable

- **WHEN** `run_goal` is invoked AND `~/.codebus/config.yaml` selects `claude_code.active: azure` AND `build_env_overrides` returns `Err(KeyringError::*)`
- **THEN** the function SHALL return `Err(VerbError::KeyringMissing { source })` AND `agent::invoke` SHALL NOT have been spawned AND `VerbError::cli_exit_code()` SHALL equal `3`

#### Scenario: Spawn surfaces underlying io error

- **WHEN** `run_query` is invoked AND the `claude` binary cannot be located on PATH AND `CODEBUS_CLAUDE_BIN` is unset
- **THEN** the function SHALL return `Err(VerbError::Spawn { source })` where `source.kind()` SHALL equal `io::ErrorKind::NotFound` or equivalent

#### Scenario: Chat CLI observes Cancelled on user Ctrl+C

- **WHEN** the `codebus chat` CLI is mid-turn AND the user presses Ctrl+C AND the SIGINT trap flips the cancel flag to true
- **THEN** `run_chat_turn` SHALL return `Err(VerbError::Cancelled)` AND the chat CLI command branch SHALL match this variant AND print an interrupted-status line AND redisplay the REPL prompt

## ADDED Requirements

### Requirement: Agent Invoke Resume Session Support

The `codebus_core::agent::claude_cli::InvokeAgentOptions` struct SHALL include a field `pub resume_session_id: Option<String>`. When this field is `Some(id)`, the `invoke()` function SHALL append `--resume <id>` to the `claude` command arguments before the `--tools` / `--allowedTools` / `--permission-mode` flags. When the field is `None`, no `--resume` argument SHALL be added (preserving the pre-chat-verb spawn argv shape).

The `--resume` flag SHALL be passed exactly once and SHALL precede the toolset flags. The session id value SHALL be passed verbatim (no quoting or escaping beyond what `Command::arg` provides) — Claude CLI session ids are UUID-like strings that require no shell quoting.

The existing CLI thin wrappers for `goal`, `query`, and `fix` SHALL initialize `InvokeAgentOptions::resume_session_id` to `None`, preserving byte-equivalent spawn behavior for those verbs.

#### Scenario: Resume id Some triggers --resume argument

- **WHEN** `invoke()` is called with `InvokeAgentOptions { resume_session_id: Some("abc-123"), .. }`
- **THEN** the spawned `claude` child process argv SHALL include `--resume` AND `abc-123` as consecutive arguments AND SHALL precede the `--tools` flag in argument order

#### Scenario: Resume id None omits --resume argument

- **WHEN** `invoke()` is called with `InvokeAgentOptions { resume_session_id: None, .. }`
- **THEN** the spawned `claude` child process argv SHALL NOT contain the `--resume` flag

#### Scenario: Existing verb thin wrappers pass None

- **WHEN** `run_goal`, `run_query`, or `run_fix` constructs an `InvokeAgentOptions` value
- **THEN** the `resume_session_id` field SHALL equal `None` AND the spawned argv SHALL be byte-equivalent to the pre-chat-verb implementation for the same invocation
