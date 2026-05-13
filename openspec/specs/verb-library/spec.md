# verb-library Specification

## Purpose

TBD - created by archiving change 'v3-goal-library'. Update Purpose after archive.

## Requirements

### Requirement: Verb Library Module Surface

The system SHALL expose a public module `codebus_core::verb` containing three sub-modules `goal`, `query`, and `fix`. Each sub-module SHALL export exactly one public orchestration function (`run_goal`, `run_query`, `run_fix`) plus the verb-specific options and report structs. The `codebus_core::verb` parent module SHALL also export the cross-verb types `VerbEvent`, `VerbLifecycleEvent`, and `VerbError`. No other public surface SHALL be exposed under `codebus_core::verb` by this change. The `codebus_core::vault::init::run_init` function defined by foundation SHALL remain in its existing location and SHALL NOT be moved into `codebus_core::verb`.

#### Scenario: Verb library module path exists

- **WHEN** a downstream crate (codebus-cli or codebus-app) writes `use codebus_core::verb::{goal, query, fix};`
- **THEN** the compilation SHALL succeed AND the three sub-modules SHALL resolve to public modules each exporting one `run_*` function

#### Scenario: Init verb is not moved

- **WHEN** a downstream crate writes `use codebus_core::verb::init;`
- **THEN** the compilation SHALL fail (no such module) AND init orchestration SHALL remain accessible only via `codebus_core::vault::init::run_init`

---
### Requirement: Goal Verb Library Function

The system SHALL provide `codebus_core::verb::goal::run_goal` as a public function with the signature:

```
pub fn run_goal(
    repo: &Path,
    options: GoalOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<GoalReport, VerbError>
```

The function SHALL execute the full goal-verb orchestration in this order: vault precondition (auto-init when `<repo>/.codebus/` is missing), claude-code and lint-fix and pii config loading, source-signal drift detection, conditional raw mirror re-sync with PII scanner dispatch, `agent::invoke` spawn with `GOAL_TOOLSET` and the resolved verb config, optional fix loop invocation (skipped when `options.no_fix` is true), wiki-changed detection against the nested git repo HEAD, conditional `auto_commit` of the wiki on success, and RunLog field accumulation. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`. The function SHALL NOT call `auto_commit` when the result is `Err(VerbError::Cancelled)`.

The function SHALL fan out the `on_event` callback so that each `VerbEvent` emitted is **also** persisted to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability). The fan-out SHALL preserve the caller's view of the event stream (the caller closure SHALL be invoked exactly as before) AND additionally write one `EventEnvelope { ts, event }` to the events sink per emission. When `EventsSink::write_event` returns `Err`, the function SHALL emit a stderr warning prefixed with `warning: events-log` AND continue normal execution (per `events-log` `events.jsonl Write Failure Is Non-Fatal`).

The function SHALL write the final `RunLog` entry (per the `run-log` capability) with `outcome` set to `"succeeded"` on the normal success path, `"failed"` when the agent terminated non-zero and the verb propagates that failure, OR `"cancelled"` on the cancel path (cancel-path write happens before the `Err(VerbError::Cancelled)` return per `Cancellation Signal Polling`).

`GoalOptions` SHALL contain the fields `text: String`, `force_resync: bool`, `no_fix: bool`, and `no_obsidian_register: bool`. `GoalReport` SHALL contain the fields `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `lint_error_count: usize`, `lint_warn_count: usize`, `started_at: String` (RFC 3339 UTC), `finished_at: String` (RFC 3339 UTC), `agent_exit_code: Option<i32>`, AND `fix_post_lint_issues_remain: bool`. The `agent_exit_code` field carries the spawned goal agent's exit code so the CLI thin wrapper can apply the existing exit-code-precedence rule (agent failure preempts fix-phase outcome). The `fix_post_lint_issues_remain` field is `true` only when the post-spawn fix-and-lint phase terminated with `TerminationReason::PostLintIssuesRemain` AND `!report.clean`; the CLI thin wrapper uses it to emit the `✗ fix: N error(s), M warning(s) remain after agent terminated` stderr line at byte-equivalent timing.

#### Scenario: run_goal returns GoalReport on successful run

- **WHEN** `run_goal` is invoked with a valid vault AND the agent exits zero AND the fix loop reports zero errors
- **THEN** the function SHALL return `Ok(GoalReport)` AND `GoalReport.wiki_changed` SHALL be true when the nested git repo's `wiki/` tree differs from HEAD AND `auto_commit` SHALL have been invoked once with a `wiki: <goal-text>` message AND the appended `RunLog.outcome` SHALL equal `"succeeded"`

#### Scenario: run_goal auto-inits missing vault

- **WHEN** `run_goal` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL invoke `vault::init::run_init` to create the vault AND proceed with the goal flow. The auto-init invocation SHALL pass a no-op `InitEvent` closure (`|_| {}`) — the library does NOT translate `InitEvent` variants into `VerbEvent` emissions in this change. CLI's direct `codebus init` invocation continues to emit init banners via `commands::init::run`; auto-init triggered inside `run_goal` runs silently. A future enhancement MAY add an InitEvent-to-VerbEvent adapter.

#### Scenario: run_goal short-circuits when fix is disabled

- **WHEN** `run_goal` is invoked with `GoalOptions { no_fix: true, .. }`
- **THEN** the function SHALL skip the fix loop step AND the returned `GoalReport.lint_error_count` and `GoalReport.lint_warn_count` SHALL reflect the pre-fix lint counts (or zero when no lint was run)

#### Scenario: run_goal cancel path writes cancelled RunLog and persists events.jsonl

- **WHEN** `run_goal` is invoked with a cancel flag AND the flag flips mid-stream
- **THEN** the function SHALL invoke `EventsSink::write_event` at least once before the cancel observation (the events.jsonl file SHALL contain the envelopes accumulated up to the cancel point) AND SHALL invoke `LogSink::write_run` exactly once with `RunLog.outcome == "cancelled"` AND THEN SHALL return `Err(VerbError::Cancelled)` AND `git::auto_commit` SHALL NOT have been invoked

---
### Requirement: Query Verb Library Function

The system SHALL provide `codebus_core::verb::query::run_query` as a public function with the signature:

```
pub fn run_query(
    repo: &Path,
    options: QueryOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<QueryReport, VerbError>
```

The function SHALL execute the full query-verb orchestration in this order: strict vault precondition (return `Err(VerbError::VaultMissing { path })` when `<repo>/.codebus/` is missing — query SHALL NOT auto-init), claude-code config loading, `agent::invoke` spawn with `QUERY_TOOLSET` (read-only: `Read`, `Glob`, `Grep`) and the resolved verb config, and RunLog field accumulation. The function SHALL NOT call `auto_commit` under any circumstance (query is read-only). The function SHALL NOT run the fix loop. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`.

The function SHALL fan out the `on_event` callback to additionally persist each `VerbEvent` to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability), with the same write-failure-non-fatal semantics as `run_goal`.

The function SHALL write the final `RunLog` entry with `outcome` set to `"succeeded"` on the normal success path (regardless of agent exit code — `query_propagates_agent_exit_code` test asserts CLI propagates the child exit but the RunLog row is still marked succeeded), OR `"cancelled"` on the cancel path (cancel-path write happens before the `Err(VerbError::Cancelled)` return per `Cancellation Signal Polling`).

`QueryOptions` SHALL contain the field `text: String`. `QueryReport` SHALL contain `accumulated_tokens: TokenUsage`, `started_at: String`, `finished_at: String`, AND `agent_exit_code: Option<i32>`. `QueryReport` SHALL NOT contain `wiki_changed` or any lint count field. The `agent_exit_code` field carries the spawned claude child's exit code so the CLI thin wrapper can propagate it as its own exit code (preserving `query_propagates_agent_exit_code` and other golden tests); `None` represents signal termination on platforms where the child died without an integer exit code.

#### Scenario: run_query refuses missing vault

- **WHEN** `run_query` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL return `Err(VerbError::VaultMissing { path })` where `path` is the `<repo>/.codebus/` path AND SHALL NOT spawn `agent::invoke` AND SHALL NOT auto-init the vault

#### Scenario: run_query never auto-commits

- **WHEN** `run_query` runs to successful agent completion AND the agent has written files into the vault working tree
- **THEN** the function SHALL return `Ok(QueryReport)` AND SHALL NOT have invoked `auto_commit` AND the working tree SHALL retain any uncommitted writes (caller decides how to handle them)

#### Scenario: run_query records outcome succeeded with non-zero agent exit

- **WHEN** `run_query` is invoked AND the agent exits with non-zero exit code (e.g., agent crash mid-stream)
- **THEN** the function SHALL return `Ok(QueryReport)` with `agent_exit_code == Some(<non-zero>)` AND the appended `RunLog.outcome` SHALL equal `"succeeded"` (the verb itself completed even though the child crashed; CLI propagates the child exit via `agent_exit_code` field)

---
### Requirement: Fix Verb Library Function

The system SHALL provide `codebus_core::verb::fix::run_fix` as a public function with the signature:

```
pub fn run_fix(
    repo: &Path,
    options: FixOptions,
    on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<FixReport, VerbError>
```

The function SHALL execute the full fix-verb orchestration in this order: strict vault precondition (return `Err(VerbError::VaultMissing { path })` when `<repo>/.codebus/` is missing — fix SHALL NOT auto-init), `no_fix` short-circuit (return immediately when `options.no_fix` is true), lint pre-check (return immediately with zero-issue `FixReport` when no lint errors AND no warnings exist), `agent::invoke` spawn with `FIX_TOOLSET` and the resolved verb config, fix loop run, final lint pass, conditional `auto_commit` on success, and RunLog field accumulation. The function SHALL NOT call `auto_commit` when the result is `Err(VerbError::Cancelled)`. The function SHALL NOT touch stdout or stderr directly; all renderable progress SHALL flow through `on_event`.

The function SHALL fan out the `on_event` callback to additionally persist each `VerbEvent` to the run's events.jsonl file via an internally-constructed `EventsSink` (per the `events-log` capability), with the same write-failure-non-fatal semantics as `run_goal`.

The function SHALL write the final `RunLog` entry with `outcome` set to `"succeeded"` when termination is `PostLintClean` or `InitialClean`, `"failed"` when termination is `PostLintIssuesRemain`, OR `"cancelled"` on the cancel path. The `Skipped` status (no_fix flag or `lint.fix.enabled` false) SHALL NOT write a RunLog entry (no agent spawn occurred — there is nothing to record).

`FixOptions` SHALL contain the field `no_fix: bool`. `FixReport` SHALL contain `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `final_lint_error_count: usize`, `final_lint_warn_count: usize`, `fix_iterations: u8`, `started_at: Option<String>`, `finished_at: Option<String>`, AND `status: FixStatus`. The `started_at` / `finished_at` fields SHALL be `None` on the `Skipped` and `InitialClean` short-circuit paths (where no agent spawn occurred) and `Some(rfc3339)` on the post-agent paths.

`FixStatus` SHALL be a public enum with exactly four variants: `Skipped { reason: SkipReason }`, `InitialClean`, `PostLintClean`, `PostLintIssuesRemain`. `SkipReason` SHALL be a public enum with exactly two variants: `NoFixFlag` (the caller passed `FixOptions { no_fix: true }`) and `DisabledByConfig` (the loaded `lint.fix.enabled` was `false`). The CLI thin wrapper SHALL `match` exhaustively on `FixStatus` to emit the existing per-status stderr messages (`fix: disabled by --no-fix or lint.fix.enabled = false` for `Skipped`, `✗ fix: N error(s), M warning(s) remain after agent terminated` for `PostLintIssuesRemain`, no stderr for `InitialClean` and `PostLintClean`) and derive its own exit code (`Skipped` / `InitialClean` / `PostLintClean` → 0; `PostLintIssuesRemain` → 1).

#### Scenario: run_fix refuses missing vault

- **WHEN** `run_fix` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL return `Err(VerbError::VaultMissing { path })` AND SHALL NOT spawn `agent::invoke`

#### Scenario: run_fix short-circuits on no_fix

- **WHEN** `run_fix` is invoked with `FixOptions { no_fix: true }`
- **THEN** the function SHALL return `Ok(FixReport)` with `fix_iterations == 0` AND `status == FixStatus::Skipped { reason: SkipReason::NoFixFlag }` AND `started_at == None` AND `finished_at == None` AND SHALL NOT spawn `agent::invoke` AND SHALL NOT run lint AND SHALL NOT write a RunLog entry

#### Scenario: run_fix short-circuits on clean lint pre-check

- **WHEN** `run_fix` is invoked AND the pre-check lint reports zero error count AND zero warn count
- **THEN** the function SHALL return `Ok(FixReport)` with `fix_iterations == 0`, `final_lint_error_count == 0`, `final_lint_warn_count == 0` AND `status == FixStatus::InitialClean` AND `started_at == None` AND `finished_at == None` AND SHALL NOT spawn `agent::invoke` AND SHALL NOT write a RunLog entry

#### Scenario: run_fix populates status PostLintClean on successful repair and writes RunLog succeeded

- **WHEN** `run_fix` runs to completion AND the post-spawn lint reports zero errors AND zero warnings
- **THEN** the function SHALL return `Ok(FixReport)` with `status == FixStatus::PostLintClean` AND `fix_iterations == 1` AND `started_at == Some(_)` AND `finished_at == Some(_)` AND the appended `RunLog.outcome` SHALL equal `"succeeded"`

#### Scenario: run_fix populates status PostLintIssuesRemain and writes RunLog failed

- **WHEN** `run_fix` runs to completion AND the post-spawn lint reports at least one error or warning
- **THEN** the function SHALL return `Ok(FixReport)` with `status == FixStatus::PostLintIssuesRemain` AND `fix_iterations == 1` AND the appended `RunLog.outcome` SHALL equal `"failed"`

#### Scenario: run_fix cancel path writes cancelled RunLog

- **WHEN** `run_fix` is invoked with a cancel flag AND the flag flips mid-stream
- **THEN** the function SHALL invoke `LogSink::write_run` exactly once with `RunLog.outcome == "cancelled"` BEFORE returning `Err(VerbError::Cancelled)` AND SHALL NOT invoke `git::auto_commit`

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

`VerbLifecycleEvent` SHALL be a public enum with at minimum these variants: `SpawnStart { verb: Verb }`, `SpawnEnd { verb: Verb, exit_code: Option<i32> }`, `FixIterationStart { iteration: u8 }`, and `LintFinal { error_count: usize, warn_count: usize }`. Additional lifecycle variants MAY be added by future changes following minor-version semantics; downstream pattern matches SHALL be required to use a non-exhaustive marker or wildcard arm.

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

- `VaultMissing { path }` — returned by `run_query` and `run_fix` when `<repo>/.codebus/` is absent; `run_goal` SHALL auto-init instead (per Goal Verb Library Function) and SHALL NOT return this variant
- `ConfigParse { which, source }` — returned by any `run_*` when a config section yaml fails to parse. The `which` field SHALL be one of `"claude_code"`, `"lint.fix"`, `"log"`, or `"pii"` (a `&'static str` chosen by the verb function based on which loader rejected the yaml) so the CLI thin wrapper can emit section-specific stderr (`error: {which} config parse failed at {path}: {source}`) preserving byte-equivalent output
- `KeyringMissing { source }` — returned by any `run_*` when `build_env_overrides` cannot resolve the Azure profile's API key from the OS keyring + env fallback chain. Surfaced to the CLI as exit code 3, preserving the pre-refactor `error: {verb}: {source}` stderr line
- `Spawn { source }` — returned by any `run_*` when `agent::invoke` returns an `io::Result::Err` (e.g., claude binary not on PATH, fork failure)
- `Cancelled` — returned by any `run_*` when the `cancel` signal flag was observed flipped to true during the run
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

---
### Requirement: Cancellation Signal Polling

The `agent::invoke` function SHALL accept a `cancel: Option<Arc<AtomicBool>>` parameter. When `cancel` is `Some(flag)`, the function SHALL read the flag with `Ordering::Relaxed` after processing each line read from the child's stdout. When the flag is observed as `true`, the function SHALL invoke `child.kill()` on the spawned child process, SHALL drain any remaining bytes from the child's stdout pipe on a best-effort basis (no further `on_event` invocations), SHALL `child.wait()` to reap the child, and SHALL return `Ok(InvokeReport { exit: <kill-state>, .. })` with `started_at` and `finished_at` populated as for a normal return. The function SHALL NOT panic if `child.kill()` fails (the child may have already exited between the poll and the kill call) — best-effort termination is the contract.

Each `run_*` function in `codebus_core::verb` SHALL, after `agent::invoke` returns, read the same `cancel` flag (if `Some`) and translate an observed-true value into `Err(VerbError::Cancelled)`. Before returning that `Err`, the verb function SHALL write one `RunLog` entry with `outcome: "cancelled"` to the configured log sink (per the `run-log` capability), reflecting the accumulated partial token usage and `wiki_changed` status as observed at the cancel point. The verb function SHALL skip `git::auto_commit` on this path per the existing `Auto-Commit Skipped On Cancel Or Error` requirement.

When `cancel` is `None`, the function SHALL behave exactly as before this change (no polling overhead beyond a single `None` discriminant check per event loop iteration).

#### Scenario: Cancel flag flipped mid-stream halts loop

- **WHEN** `agent::invoke` is invoked with `cancel: Some(flag)` AND the caller flips `flag` to `true` after the second stream line has been processed
- **THEN** the function SHALL invoke `child.kill()` no later than after processing the third line (one-line poll cadence) AND SHALL return `Ok(InvokeReport)` whose `exit.success()` SHALL be `false`

#### Scenario: Cancel propagates to VerbError::Cancelled and writes RunLog first

- **WHEN** `run_goal` is invoked with a cancel flag that is flipped during the agent spawn AND `agent::invoke` returns with a non-success exit due to the kill
- **THEN** `run_goal` SHALL invoke `LogSink::write_run` exactly once with a `RunLog` whose `outcome == "cancelled"` BEFORE returning `Err(VerbError::Cancelled)` AND SHALL NOT invoke `git::auto_commit`

#### Scenario: Cancel-path RunLog reflects partial state

- **WHEN** `run_goal` is cancelled after the agent emitted 3 `Thought` events AND 2 `ToolUse` events AND no `Usage` event yet
- **THEN** the written `RunLog.tokens.input_tokens` SHALL equal 0 (no Usage emitted) AND `RunLog.lint_error_count` SHALL equal 0 (no lint phase ran) AND `RunLog.wiki_changed` SHALL reflect the actual `git diff HEAD~1 wiki/` result at cancel time AND `RunLog.outcome` SHALL equal `"cancelled"`

---
### Requirement: Auto-Commit Skipped On Cancel Or Error

When `run_goal` or `run_fix` returns `Err(VerbError::*)` for any error variant, the function SHALL NOT invoke `git::auto_commit`. When `run_goal` or `run_fix` returns `Ok(report)` after a successful agent run, auto_commit SHALL be invoked exactly once with the existing message format (preserving CLI byte-equivalent commit messages: `wiki: <goal-text>` for goal, `wiki: lint fix loop` for fix). `run_query` SHALL NEVER invoke `auto_commit` regardless of return value (query is read-only per its Query Verb Library Function requirement).

The auto-commit skip on error SHALL apply to all `VerbError` variants, including but not limited to `Cancelled` and `Spawn`. The on-disk vault working tree SHALL retain any partial wiki writes performed by the agent before the error — the caller (CLI or GUI) decides whether to discard or preserve them.

#### Scenario: Spawn error skips auto-commit

- **WHEN** `run_goal` is invoked AND `agent::invoke` returns `io::Result::Err` (e.g., binary missing) AND the wiki working tree had no pre-existing uncommitted changes
- **THEN** the function SHALL return `Err(VerbError::Spawn { source })` AND `git::auto_commit` SHALL NOT have been invoked AND the wiki tree SHALL remain at its pre-call HEAD state

#### Scenario: Cancel preserves partial writes

- **WHEN** `run_goal` is invoked with a cancel flag AND the agent writes one wiki page to the working tree before the cancel is observed AND the cancel halts the run
- **THEN** the function SHALL return `Err(VerbError::Cancelled)` AND `git::auto_commit` SHALL NOT have been invoked AND the written wiki page SHALL remain on disk in the working tree (uncommitted) AND the caller MAY inspect or discard the change as appropriate
