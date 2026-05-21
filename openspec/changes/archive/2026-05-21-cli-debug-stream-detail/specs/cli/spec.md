## MODIFIED Requirements

### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit (in addition to the default-mode banner sequence) the per-step `✓ <internal-detail>` progress lines describing intermediate orchestration outcomes AND the `[debug]` lines describing internal decisions, fs operations, computed values, and target paths. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]` AND SHALL NOT emit per-step `✓ <internal-detail>` progress lines (only the higher-level banner sequence emerges in default mode).

When `--debug` is set, the binary SHALL additionally render the agent stream in verbose form: it SHALL set `RenderOptions.verbose` to true so the agent-stream renderer (per the `agent-stream-rendering` capability `Stream Event Terminal Rendering` requirement) surfaces complete tool input and complete tool result without summarization, truncation, or suppression. When `--debug` is NOT set, `RenderOptions.verbose` SHALL be false and the agent stream SHALL render in the compact form (byte-identical to the pre-change behavior). This verbose stream rendering applies to the agent-spawning verbs (`goal`, `query`, `fix`, `chat`); it does not alter how non-agent subcommands render.

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
