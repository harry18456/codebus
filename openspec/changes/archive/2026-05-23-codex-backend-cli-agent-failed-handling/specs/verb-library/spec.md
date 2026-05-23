## MODIFIED Requirements

### Requirement: Verb Error Enum

The system SHALL define `codebus_core::verb::VerbError` as a public enum with exactly these variants:

```
pub enum VerbError {
    VaultMissing { path: PathBuf },
    ConfigParse { which: &'static str, source: ConfigLoadError },
    KeyringMissing { source: KeyringError },
    Spawn { source: io::Error },
    Cancelled,
    AgentFailed { exit_code: Option<i32> },
    Internal { message: String },
}
```

The enum SHALL implement `std::error::Error` (via `thiserror`) AND `std::fmt::Display`. The variant semantics SHALL be:

- `VaultMissing { path }` — returned by `run_query`, `run_fix`, AND `run_chat_turn` when `<repo>/.codebus/` is absent; `run_goal` SHALL auto-init instead (per Goal Verb Library Function) AND SHALL NOT return this variant
- `ConfigParse { which, source }` — returned by any `run_*` when a config section yaml fails to parse. The `which` field SHALL be one of `"claude_code"`, `"lint.fix"`, `"log"`, or `"pii"` (a `&'static str` chosen by the verb function based on which loader rejected the yaml) so the CLI thin wrapper can emit section-specific stderr (`error: {which} config parse failed at {path}: {source}`) preserving byte-equivalent output
- `KeyringMissing { source }` — returned by any `run_*` when `build_env_overrides` cannot resolve the Azure profile's API key from the OS keyring + env fallback chain. Surfaced to the CLI as exit code 3, preserving the pre-refactor `error: {verb}: {source}` stderr line
- `Spawn { source }` — returned by any `run_*` when `agent::invoke` returns an `io::Result::Err` (e.g., claude binary not on PATH, fork failure)
- `Cancelled` — returned by any `run_*` when the `cancel` signal flag was observed flipped to true during the run. The `chat` verb CLI subcommand (`commands/chat.rs`) SHALL pass `cancel: Some(flag)` AND observe `VerbError::Cancelled` on the user's first Ctrl+C; the `goal` / `query` / `fix` CLI thin wrappers SHALL continue to pass `cancel: None` AND never observe this variant. Downstream `match` arms on `VerbError` in CLI commands SHALL handle `Cancelled` only in the chat command's branch AND SHALL leave the other commands' branches as unreachable for that variant (current behavior preserved)
- `AgentFailed { exit_code }` — returned by `run_chat_turn` when the spawned agent child terminated with a non-zero exit code. The `exit_code` field carries the child's reported exit code (`None` represents signal termination on platforms where the child died without an integer code). Distinct from `Spawn` (which is a launch failure): `AgentFailed` indicates the agent launched successfully AND ran but exited with an error, so callers SHALL surface "the turn failed" instead of silently treating a non-zero exit as success — this prevents the regression that the codex-backend change fixed (RunLog row mislabeled `"succeeded"` when codex returned an error code). The one-shot verbs (`run_goal`, `run_query`, `run_fix`, `run_quiz_plan`, `run_quiz_generate`) SHALL NOT emit `AgentFailed`; they propagate the child's exit code through their report struct's `agent_exit_code` field instead so the CLI thin wrapper SHALL call `ExitCode::from(child_exit)` directly on the `Ok(report)` path (the one-shot verbs always succeed at the verb-library level — only the agent's process exit matters to the caller).
- `Internal { message }` — returned by any `run_*` for any other unrecoverable failure with a human-readable message

`VerbError` SHALL expose a `cli_exit_code(&self) -> u8` method that maps each variant to the per-verb exit code policy preserved by the refactor: `VaultMissing` → 2, `ConfigParse` → 2, `KeyringMissing` → 3, `Spawn` → 1, `Cancelled` → 0 (CLI never observes this — guard for completeness), `AgentFailed` → 1, `Internal` → 1. The `AgentFailed` mapping SHALL collapse the child exit code into the single non-success code `1` rather than propagating the child's exit code value — chat REPL semantics keep `1 = something failed` simple for shell consumers, in contrast to the one-shot verbs that propagate the child's exit code through `Ok(report).agent_exit_code` (this divergence is intentional AND not a defect; it reflects different consumption models, chat REPL vs scriptable one-shot). CLI thin wrappers in `codebus-cli/src/commands/{chat,goal,query,fix,quiz}.rs` SHALL `match` exhaustively on `VerbError` to derive the exit code AND to emit the verb-specific stderr message. The exhaustive match guarantees compile-time coverage when a future variant is added.

The CLI thin wrappers SHALL handle `AgentFailed` per the following split:

- The `chat` command's `translate_error` SHALL match `AgentFailed { exit_code }` as an active arm AND emit a user-facing stderr line containing the child exit code (`error: chat: agent exited with code <N>`, or `error: chat: agent exited without a recorded exit code` when `exit_code` is `None`) AND exit with the `cli_exit_code()` value.
- The `goal` / `query` / `fix` / `quiz` thin wrappers SHALL match `AgentFailed { exit_code }` as a defensive arm (the verb library functions SHALL NOT emit `AgentFailed` on those paths, but exhaustive match requires the arm); the arm SHALL emit a generic stderr fallback (`error: <verb>: agent exited with code <N>` or the `None` form) AND exit with the `cli_exit_code()` value. The defensive arms SHALL NOT use `unreachable!()` — using a generic fallback avoids panicking the binary if a future regression emits `AgentFailed` from a one-shot verb.

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

#### Scenario: Chat returns AgentFailed when agent exits non-zero

- **WHEN** `run_chat_turn` is invoked AND the spawned agent child terminates with a non-zero exit code (e.g., codex `exec resume` rejecting a cross-provider switch)
- **THEN** the function SHALL return `Err(VerbError::AgentFailed { exit_code })` where `exit_code` carries the child's reported integer code (or `None` for signal termination) AND `VerbError::cli_exit_code()` SHALL equal `1` AND the chat CLI command branch SHALL emit a stderr line containing the child exit code AND the appended `RunLog.outcome` for that turn SHALL equal `"failed"` (NOT `"succeeded"`)

#### Scenario: AgentFailed Display message includes exit code when present

- **WHEN** `VerbError::AgentFailed { exit_code: Some(42) }.to_string()` is called
- **THEN** the resulting string SHALL contain the literal substring `42` AND SHALL contain the literal substring `non-zero status`

##### Example: Display output by exit_code shape

| exit_code value | Resulting Display contains | Notes |
| --- | --- | --- |
| `Some(0)` | `(0)` | uncommon but representable; verb only emits AgentFailed for non-zero, but the type permits any i32 |
| `Some(1)` | `(1)` | typical |
| `Some(42)` | `(42)` | multi-digit code |
| `None` | no `(...)` parenthesised group after `non-zero status` | signal termination; condition-formatted with `unwrap_or_default()` |

#### Scenario: Goal CLI thin wrapper handles AgentFailed as defensive fallback

- **WHEN** a hypothetical regression causes `run_goal` to emit `Err(VerbError::AgentFailed { exit_code: Some(7) })` (the verb library SHALL NOT emit this on the goal path under the contract, but exhaustive match requires the arm)
- **THEN** the `goal` CLI thin wrapper's `translate_error` SHALL match the arm AND emit a stderr line containing `agent exited with code 7` AND exit with status `1` AND SHALL NOT panic (`unreachable!()` is forbidden on this arm)
