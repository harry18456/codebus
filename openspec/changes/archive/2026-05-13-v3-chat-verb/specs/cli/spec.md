## MODIFIED Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register exactly seven subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand. The `config` subcommand SHALL itself expose three sub-actions (`set-key`, `get-key`, `delete-key`); the sub-action contract is defined normatively in the `claude-code-config` capability. The `chat` subcommand contract is defined normatively in the `Chat Subcommand Behavior` requirement of this capability and the `Chat CLI Subcommand Behavior` requirement of the `chat-verb` capability.

#### Scenario: Help output lists exactly the seven subcommands

- **WHEN** `codebus --help` is invoked
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the binary SHALL print a single line containing the cargo package version of the `codebus-cli` crate AND SHALL exit with status zero

#### Scenario: Unknown subcommand is rejected by clap

- **WHEN** `codebus mcp` or `codebus randomverb` is invoked
- **THEN** the binary SHALL print a clap error message to stderr identifying the unknown subcommand AND SHALL exit with non-zero status

#### Scenario: Config subcommand help lists its three actions

- **WHEN** `codebus config --help` is invoked
- **THEN** the help output SHALL list `set-key`, `get-key`, `delete-key` as the only sub-actions AND SHALL exit with status zero

#### Scenario: Chat subcommand help describes REPL behavior

- **WHEN** `codebus chat --help` is invoked
- **THEN** the help output SHALL describe the subcommand as launching an interactive multi-turn read-only chat REPL AND SHALL exit with status zero

---

### Requirement: Verb RunLog Capture and Persistence

The `goal`, `query`, `fix`, and `chat` subcommands SHALL each capture `RunLog` entries per the `run-log` capability `RunLog Schema and Per-Invocation Capture` requirement and SHALL persist them to the configured `LogSink` (resolved per the `run-log` capability `Log Configuration Schema` and `Default Log Directory Resolution` requirements).

For `goal`, `query`, and `fix`, exactly one `RunLog` entry SHALL be appended per invocation; the persistence step SHALL run as the verb's penultimate action — after the auto-commit step (where applicable) and before the `Done` banner — so that the entry includes the final `wiki_changed`, `lint_error_count`, and `lint_warn_count` values.

For `chat`, exactly one `RunLog` entry SHALL be appended per chat turn (the REPL invokes `run_chat_turn` once per turn; each call writes one entry). Each chat turn's `RunLog` SHALL have `mode == "chat"` AND `session_id == Some(session_id_from_init_event)`. The `chat` subcommand SHALL NOT write a separate aggregate `RunLog` entry for the REPL session as a whole — only the per-turn entries.

When a `chat` turn culminates in a confirmed promote-to-goal, the spawned `codebus goal` child subprocess SHALL itself write its own `RunLog` entry (with `mode == "goal"` and no `session_id` field) per the same `run-log` capability rules; the parent `codebus chat` process SHALL NOT additionally write that entry.

When the verb fails (agent crash, lint phase failure, auto-commit failure), the persistence step SHALL still run so the `RunLog` records the partial-state outcome; the verb SHALL NOT skip log persistence on its failure paths. When the `LogSink::write_run` call returns an error, the verb SHALL emit a stderr warning prefixed with `warning: run-log` and SHALL NOT propagate the failure into its exit code (per the `run-log` capability `RunLog Write Failure Is Non-Fatal` requirement).

#### Scenario: Each verb invocation appends exactly one RunLog entry

- **WHEN** `codebus goal "X"` runs to completion against a vault configured with `log.sink: jsonl`
- **THEN** the file `<vault>/.codebus/log/runs-<YYYY-MM-DD>.jsonl` SHALL gain exactly one new line containing a single JSON object whose `goal` field equals `"X"` and whose `mode` field equals `"goal"`

#### Scenario: RunLog written even when agent exits non-zero

- **WHEN** the agent spawn exits with non-zero status during a `codebus goal "X"` invocation
- **THEN** the `RunLog` entry SHALL still be appended to the jsonl file AND its `tokens` field SHALL reflect any `Usage` events that were streamed before the failure AND its `mode` SHALL equal `"goal"`

#### Scenario: Default sink resolves to vault-local log directory

- **WHEN** `codebus goal "X"` runs against a vault and no `log:` section is present in `~/.codebus/config.yaml`
- **THEN** the default `Jsonl { dir: None }` config SHALL resolve to `<vault>/.codebus/log/` AND the entry SHALL be appended there

#### Scenario: Explicit none sink suppresses persistence

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: none\n` AND `codebus goal "X"` runs to completion
- **THEN** no file under `<vault>/.codebus/log/` SHALL be created or modified

#### Scenario: Chat turn writes RunLog with mode chat and session_id

- **WHEN** the user enters one prompt during a `codebus chat` session AND the chat turn completes successfully
- **THEN** the run-log jsonl SHALL gain exactly one new line whose `mode` field equals `"chat"` AND whose `session_id` field equals `Some(<session_id_from_init_event>)`

#### Scenario: Promote-to-goal subprocess writes its own RunLog

- **WHEN** a chat turn culminates in confirmed promote AND the spawned `codebus goal` subprocess completes successfully
- **THEN** the run-log jsonl SHALL gain one additional line (beyond the chat turn's row) with `mode == "goal"` AND SHALL NOT contain a `session_id` field on that goal row

---

### Requirement: Spawn Verb Library Delegation

The CLI subcommand handlers for the three thin-wrapper spawn verbs (`codebus-cli/src/commands/goal.rs`, `codebus-cli/src/commands/query.rs`, `codebus-cli/src/commands/fix.rs`) SHALL act as thin wrappers that delegate orchestration to `codebus_core::verb::goal::run_goal`, `codebus_core::verb::query::run_query`, and `codebus_core::verb::fix::run_fix` respectively (see the `verb-library` capability). Each handler SHALL be responsible for: (1) clap argument parsing into the verb-specific options struct, (2) constructing a closure that maps `VerbEvent::Banner(_)` to `print_banner` and `VerbEvent::Stream(_)` to `print_event` against the active `RenderOptions`, (3) calling the library function with the closure and a `None` cancel signal, (4) matching exhaustively on `VerbError` to derive the exit code preserving the existing per-error mapping, and (5) writing the resulting `RunLog` entry via the shared run-log persistence helpers.

The CLI subcommand handler for the `chat` verb (`codebus-cli/src/commands/chat.rs`) SHALL NOT follow the thin-wrapper one-shot pattern. The chat handler SHALL instead implement a REPL loop that calls `run_chat_turn` once per user turn, maintains transcript state across turns, registers a SIGINT trap to flip an `AtomicBool` cancel flag passed as `cancel: Some(flag)` to each `run_chat_turn` invocation, observes `VerbLifecycleEvent::PromoteSuggestion` events emitted via `on_event`, prompts the user for `(y/n)` confirmation on promote suggestions, and (when confirmed) spawns a `codebus goal` child subprocess (NOT a `run_goal` library call). The detailed REPL contract is defined normatively in the `chat-verb` capability `Chat CLI Subcommand Behavior` requirement and related requirements.

#### Scenario: Goal handler is a thin wrapper

- **WHEN** the `codebus-cli/src/commands/goal.rs` handler is invoked for `codebus goal "X"`
- **THEN** the handler SHALL parse clap args AND call `run_goal` exactly once AND match exhaustively on `VerbError` AND SHALL NOT contain a stdin read loop AND SHALL pass `cancel: None`

#### Scenario: Chat handler runs a REPL with cancel signal

- **WHEN** the `codebus-cli/src/commands/chat.rs` handler is invoked for `codebus chat`
- **THEN** the handler SHALL register a SIGINT trap AND enter a stdin read loop AND call `run_chat_turn` once per user turn AND pass `cancel: Some(flag)` on every call AND NOT pass `cancel: None`

#### Scenario: Promote confirmation spawns subprocess not library call

- **WHEN** the chat REPL receives `VerbLifecycleEvent::PromoteSuggestion { reason }` AND the user confirms with `y`
- **THEN** the handler SHALL spawn a `codebus goal "<transcript>"` child subprocess via `std::process::Command` AND SHALL NOT call `codebus_core::verb::goal::run_goal` from within the chat command

## ADDED Requirements

### Requirement: Chat Subcommand Behavior

The `codebus chat` subcommand SHALL accept zero positional arguments and SHALL accept the standard global flags (`--debug`, `--no-emoji`, `--no-color`, etc., resolved per `Environment-Aware Output Styling`). When invoked, the subcommand SHALL launch the interactive multi-turn chat REPL per the `chat-verb` capability `Chat CLI Subcommand Behavior` requirement, using the read-only sandbox toolset `CHAT_TOOLSET` (`Read,Glob,Grep`) passed verbatim to `agent::invoke` per the `Cancellation Signal Polling` requirement of the `verb-library` capability.

The chat subcommand SHALL exit with status zero when the user requests REPL exit (`exit`, `:q`, Ctrl+D, or second Ctrl+C). The chat subcommand SHALL exit with non-zero status only when an unrecoverable `VerbError` other than `Cancelled` propagates from `run_chat_turn` (in which case the per-error exit code mapping defined by the `verb-library` capability `Verb Error Enum` requirement applies).

The chat subcommand SHALL operate under the standard vault precondition: when `<repo>/.codebus/` is absent, the first `run_chat_turn` invocation returns `VerbError::VaultMissing` and the CLI SHALL print a stderr message instructing the user to run `codebus init` first AND exit with status 2 before entering the REPL loop.

#### Scenario: Chat exits zero on REPL exit alias

- **WHEN** the user types `exit\n` at any chat REPL prompt
- **THEN** the `codebus chat` process SHALL exit with status zero

#### Scenario: Chat aborts on missing vault before first turn

- **WHEN** `codebus chat` is invoked against a directory where `<cwd>/.codebus/` does not exist
- **THEN** the process SHALL print a stderr message indicating the vault is missing AND SHALL exit with status 2 AND SHALL NOT enter the REPL loop

#### Scenario: Chat accepts no positional args

- **WHEN** `codebus chat "extra positional"` is invoked
- **THEN** clap SHALL reject the invocation AND SHALL emit a usage error to stderr AND SHALL exit with non-zero status
