## ADDED Requirements

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

`GoalOptions` SHALL contain the fields `text: String`, `force_resync: bool`, `no_fix: bool`, and `no_obsidian_register: bool`. `GoalReport` SHALL contain the fields `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `lint_error_count: usize`, `lint_warn_count: usize`, `started_at: String` (RFC 3339 UTC), and `finished_at: String` (RFC 3339 UTC).

#### Scenario: run_goal returns GoalReport on successful run

- **WHEN** `run_goal` is invoked with a valid vault AND the agent exits zero AND the fix loop reports zero errors
- **THEN** the function SHALL return `Ok(GoalReport)` AND `GoalReport.wiki_changed` SHALL be true when the nested git repo's `wiki/` tree differs from HEAD AND `auto_commit` SHALL have been invoked once with a `wiki: <goal-text>` message

#### Scenario: run_goal auto-inits missing vault

- **WHEN** `run_goal` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL invoke `vault::init::run_init` to create the vault AND proceed with the goal flow AND emit the lifecycle events corresponding to the auto-init step via `on_event`

#### Scenario: run_goal short-circuits when fix is disabled

- **WHEN** `run_goal` is invoked with `GoalOptions { no_fix: true, .. }`
- **THEN** the function SHALL skip the fix loop step AND the returned `GoalReport.lint_error_count` and `GoalReport.lint_warn_count` SHALL reflect the pre-fix lint counts (or zero when no lint was run)

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

`QueryOptions` SHALL contain the field `text: String`. `QueryReport` SHALL contain `accumulated_tokens: TokenUsage`, `started_at: String`, and `finished_at: String`. `QueryReport` SHALL NOT contain `wiki_changed` or any lint count field.

#### Scenario: run_query refuses missing vault

- **WHEN** `run_query` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL return `Err(VerbError::VaultMissing { path })` where `path` is the `<repo>/.codebus/` path AND SHALL NOT spawn `agent::invoke` AND SHALL NOT auto-init the vault

#### Scenario: run_query never auto-commits

- **WHEN** `run_query` runs to successful agent completion AND the agent has written files into the vault working tree
- **THEN** the function SHALL return `Ok(QueryReport)` AND SHALL NOT have invoked `auto_commit` AND the working tree SHALL retain any uncommitted writes (caller decides how to handle them)

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

`FixOptions` SHALL contain the field `no_fix: bool`. `FixReport` SHALL contain `accumulated_tokens: TokenUsage`, `wiki_changed: bool`, `final_lint_error_count: usize`, `final_lint_warn_count: usize`, `fix_iterations: u8`, `started_at: String`, and `finished_at: String`.

#### Scenario: run_fix refuses missing vault

- **WHEN** `run_fix` is invoked AND `<repo>/.codebus/` does not exist
- **THEN** the function SHALL return `Err(VerbError::VaultMissing { path })` AND SHALL NOT spawn `agent::invoke`

#### Scenario: run_fix short-circuits on no_fix

- **WHEN** `run_fix` is invoked with `FixOptions { no_fix: true }`
- **THEN** the function SHALL return `Ok(FixReport)` with `fix_iterations == 0` AND SHALL NOT spawn `agent::invoke` AND SHALL NOT run lint

#### Scenario: run_fix short-circuits on clean lint pre-check

- **WHEN** `run_fix` is invoked AND the pre-check lint reports zero error count AND zero warn count
- **THEN** the function SHALL return `Ok(FixReport)` with `fix_iterations == 0`, `final_lint_error_count == 0`, `final_lint_warn_count == 0` AND SHALL NOT spawn `agent::invoke`

---
### Requirement: Verb Event Enum

The system SHALL define `codebus_core::verb::VerbEvent` as a public enum with exactly three variants:

```
pub enum VerbEvent {
    Banner(BannerKind),
    Stream(StreamEvent),
    Lifecycle(VerbLifecycleEvent),
}
```

`BannerKind` SHALL be the existing `codebus_core::render::Banner` enum (re-exported as `BannerKind` to keep the verb module surface free of `render::` coupling at type level). `StreamEvent` SHALL be the existing `codebus_core::stream::StreamEvent` from the `agent-stream-rendering` capability.

`VerbLifecycleEvent` SHALL be a public enum with at minimum these variants: `SpawnStart { verb: Verb }`, `SpawnEnd { verb: Verb, exit_code: Option<i32> }`, `FixIterationStart { iteration: u8 }`, and `LintFinal { error_count: usize, warn_count: usize }`. Additional lifecycle variants MAY be added by future changes following minor-version semantics; downstream pattern matches SHALL be required to use a non-exhaustive marker or wildcard arm.

Each `run_*` function SHALL invoke `on_event` exactly once for each banner step it would have printed in its CLI form (wrapping each `Banner::*` as `VerbEvent::Banner(...)`), exactly once for each `StreamEvent` produced by the underlying `agent::invoke` (wrapping each as `VerbEvent::Stream(...)`), and at appropriate lifecycle boundaries (wrapping each as `VerbEvent::Lifecycle(...)`).

#### Scenario: Banner events flow through on_event

- **WHEN** `run_goal` reaches the step where the CLI form would print `Banner::Start { repo_path }`
- **THEN** `on_event` SHALL be invoked with `VerbEvent::Banner(BannerKind::Start { repo_path })` AND no direct stdout write SHALL occur from the library function

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
    ConfigParse { source: ConfigLoadError },
    Spawn { source: io::Error },
    Cancelled,
    Internal { message: String },
}
```

The enum SHALL implement `std::error::Error` and `std::fmt::Display`. The variant semantics SHALL be:

- `VaultMissing { path }` — returned by `run_query` and `run_fix` when `<repo>/.codebus/` is absent; `run_goal` SHALL auto-init instead (per Goal Verb Library Function) and SHALL NOT return this variant
- `ConfigParse { source }` — returned by any `run_*` when claude-code, lint-fix, log, or pii config yaml fails to parse
- `Spawn { source }` — returned by any `run_*` when `agent::invoke` returns an `io::Result::Err` (e.g., claude binary not on PATH, fork failure)
- `Cancelled` — returned by any `run_*` when the `cancel` signal flag was observed flipped to true during the run
- `Internal { message }` — returned by any `run_*` for any other unrecoverable failure with a human-readable message

CLI thin wrappers in `codebus-cli/src/commands/{goal,query,fix}.rs` SHALL `match` exhaustively on `VerbError` to derive the CLI exit code (preserving existing exit code policy: `VaultMissing` → 2 for query and fix paths, `ConfigParse` → 2, `Spawn` → 1, `Internal` → 1, `Cancelled` → SHALL NOT occur in CLI paths because CLI does not pass a cancel signal).

#### Scenario: ConfigParse propagates underlying error

- **WHEN** `run_goal` is invoked AND `~/.codebus/config.yaml` contains a `claude_code` section that fails yaml parsing
- **THEN** the function SHALL return `Err(VerbError::ConfigParse { source })` where `source.to_string()` SHALL contain the failing field name

#### Scenario: Spawn surfaces underlying io error

- **WHEN** `run_query` is invoked AND the `claude` binary cannot be located on PATH AND `CODEBUS_CLAUDE_BIN` is unset
- **THEN** the function SHALL return `Err(VerbError::Spawn { source })` where `source.kind()` SHALL equal `io::ErrorKind::NotFound` or equivalent

---
### Requirement: Cancellation Signal Polling

The `agent::invoke` function SHALL accept a `cancel: Option<Arc<AtomicBool>>` parameter. When `cancel` is `Some(flag)`, the function SHALL read the flag with `Ordering::Relaxed` after processing each line read from the child's stdout. When the flag is observed as `true`, the function SHALL invoke `child.kill()` on the spawned child process, SHALL drain any remaining bytes from the child's stdout pipe on a best-effort basis (no further `on_event` invocations), SHALL `child.wait()` to reap the child, and SHALL return `Ok(InvokeReport { exit: <kill-state>, .. })` with `started_at` and `finished_at` populated as for a normal return. The function SHALL NOT panic if `child.kill()` fails (the child may have already exited between the poll and the kill call) — best-effort termination is the contract.

Each `run_*` function in `codebus_core::verb` SHALL, after `agent::invoke` returns, read the same `cancel` flag (if `Some`) and translate an observed-true value into `Err(VerbError::Cancelled)`, bypassing any subsequent auto-commit step.

When `cancel` is `None`, the function SHALL behave exactly as before this change (no polling overhead beyond a single `None` discriminant check per event loop iteration).

#### Scenario: Cancel flag flipped mid-stream halts loop

- **WHEN** `agent::invoke` is invoked with `cancel: Some(flag)` AND the caller flips `flag` to `true` after the second stream line has been processed
- **THEN** the function SHALL invoke `child.kill()` no later than after processing the third line (one-line poll cadence) AND SHALL return `Ok(InvokeReport)` whose `exit.success()` SHALL be `false`

#### Scenario: Cancel propagates to VerbError::Cancelled

- **WHEN** `run_goal` is invoked with a cancel flag that is flipped during the agent spawn AND `agent::invoke` returns with a non-success exit due to the kill
- **THEN** `run_goal` SHALL return `Err(VerbError::Cancelled)` AND SHALL NOT invoke `auto_commit`

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
