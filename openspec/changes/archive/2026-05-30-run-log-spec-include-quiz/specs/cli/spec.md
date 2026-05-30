## MODIFIED Requirements

### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit (in addition to the default-mode banner sequence) the per-step `✓ <internal-detail>` progress lines describing intermediate orchestration outcomes AND the `[debug]` lines describing internal decisions, fs operations, computed values, and target paths. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]` AND SHALL NOT emit per-step `✓ <internal-detail>` progress lines (only the higher-level banner sequence emerges in default mode).

When `--debug` is set, the binary SHALL additionally render the agent stream in verbose form: it SHALL set `RenderOptions.verbose` to true so the agent-stream renderer (per the `agent-stream-rendering` capability `Stream Event Terminal Rendering` requirement) surfaces complete tool input and complete tool result without summarization, truncation, or suppression. When `--debug` is NOT set, `RenderOptions.verbose` SHALL be false and the agent stream SHALL render in the compact form (byte-identical to the pre-change behavior). This verbose stream rendering applies to the agent-spawning verbs (`goal`, `query`, `fix`, `chat`, `quiz`); it does not alter how non-agent subcommands render.

#### Scenario: Default mode suppresses [debug] lines

- **WHEN** `codebus init` runs without `--debug`
- **THEN** neither stdout nor stderr SHALL contain any line beginning with `[debug]`

#### Scenario: Debug mode emits both detail and trace lines

- **WHEN** `codebus init --debug` runs against any repository
- **THEN** stdout SHALL contain at least one per-step `✓ <internal-detail>` progress line AND at least one `[debug]` trace line

#### Scenario: Debug flag enables verbose agent stream rendering

- **WHEN** a `codebus` agent-spawning verb runs with `--debug`
- **THEN** the `RenderOptions` passed to the agent-stream renderer SHALL have `verbose` set to true

#### Scenario: Default mode keeps compact agent stream rendering

- **WHEN** a `codebus` agent-spawning verb runs without `--debug`
- **THEN** the `RenderOptions` passed to the agent-stream renderer SHALL have `verbose` set to false

#### Scenario: Quiz inherits verbose agent stream rendering under debug

- **WHEN** `codebus quiz "<topic>"` runs with `--debug` (the quiz subcommand is an agent-spawning verb that consumes the same `--debug`-derived `RenderOptions` snapshot as `goal` / `query` / `fix` / `chat`)
- **THEN** the `RenderOptions` passed to the quiz generate spawn's agent-stream renderer SHALL have `verbose` set to true, so the quiz agent stream surfaces complete tool input and complete tool result without truncation or summarization

### Requirement: Verb RunLog Capture and Persistence

The `goal`, `query`, `fix`, `chat`, and `quiz` subcommands SHALL each capture `RunLog` entries per the `run-log` capability `RunLog Schema and Per-Invocation Capture` requirement and SHALL persist them to the configured `LogSink` (resolved per the `run-log` capability `Log Configuration Schema` and `Default Log Directory Resolution` requirements).

For `goal`, `query`, and `fix`, exactly one `RunLog` entry SHALL be appended per invocation; the persistence step SHALL run as the verb's penultimate action — after the auto-commit step (where applicable) and before the `Done` banner — so that the entry includes the final `wiki_changed`, `lint_error_count`, and `lint_warn_count` values.

For `chat`, exactly one `RunLog` entry SHALL be appended per chat turn (the REPL invokes `run_chat_turn` once per turn; each call writes one entry). Each chat turn's `RunLog` SHALL have `mode == "chat"` AND `session_id == Some(session_id_from_init_event)`. The `chat` subcommand SHALL NOT write a separate aggregate `RunLog` entry for the REPL session as a whole — only the per-turn entries.

When a `chat` turn culminates in a confirmed promote-to-goal, the spawned `codebus goal` child subprocess SHALL itself write its own `RunLog` entry (with `mode == "goal"` and no `session_id` field) per the same `run-log` capability rules; the parent `codebus chat` process SHALL NOT additionally write that entry.

For `quiz`, exactly one `RunLog` entry SHALL be appended per `codebus quiz` invocation. It SHALL be written by the generate spawn (`run_quiz_generate`); the plan sub-step (`run_quiz_plan`) SHALL NOT write a `RunLog`. The quiz `RunLog` SHALL have `mode == "quiz"`, `goal` equal to the comma-joined selected page paths, AND `session_id` carrying the generate spawn's id (recorded for logging only, NOT used for resume), per the `run-log` capability and the `quiz` capability.

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

#### Scenario: Quiz subcommand appends one RunLog with mode quiz

- **WHEN** `codebus quiz "auth"` runs the generate spawn to completion against a vault with selected pages `["wiki/modules/auth.md"]`
- **THEN** the file `<vault>/.codebus/log/runs-<YYYY-MM-DD>.jsonl` SHALL gain exactly one new line whose `mode` field equals `"quiz"` AND whose `goal` field equals `"wiki/modules/auth.md"` AND no separate `RunLog` line SHALL be written for the plan sub-step
