## ADDED Requirements

### Requirement: Spawn Verb Library Delegation

The CLI subcommand handlers for the three spawn verbs (`codebus-cli/src/commands/goal.rs`, `codebus-cli/src/commands/query.rs`, `codebus-cli/src/commands/fix.rs`) SHALL act as thin wrappers that delegate orchestration to `codebus_core::verb::goal::run_goal`, `codebus_core::verb::query::run_query`, and `codebus_core::verb::fix::run_fix` respectively (see the `verb-library` capability). Each handler SHALL be responsible for: (1) clap argument parsing into the verb-specific options struct, (2) constructing a closure that maps `VerbEvent::Banner(_)` to `print_banner` and `VerbEvent::Stream(_)` to `print_event` against the active `RenderOptions`, (3) calling the library function with the closure and a `None` cancel signal, (4) matching exhaustively on `VerbError` to derive the exit code preserving the existing per-error mapping, and (5) writing the resulting `RunLog` entry via the shared run-log persistence helpers.

The CLI handlers SHALL NOT contain inline orchestration logic such as drift detection, raw mirror re-sync, agent spawn, fix loop invocation, or auto-commit. The handlers SHALL NOT call `codebus_core::agent::invoke` directly — invocation SHALL go through the verb library function.

CLI observable behavior — stdout banner sequence, `print_event` rendered output bytes, stderr error messages, exit codes for every error variant, auto-commit message strings, and RunLog entry contents — SHALL be byte-equivalent to the pre-change implementation. The existing integration test suites `codebus-cli/tests/cli_routing.rs`, `codebus-cli/tests/goal_flow.rs`, `codebus-cli/tests/query_flow.rs`, and `codebus-cli/tests/fix_flow.rs` SHALL pass without modification to their golden assertions.

#### Scenario: Goal command body delegates to run_goal

- **WHEN** the goal subcommand handler in `codebus-cli/src/commands/goal.rs` is invoked
- **THEN** the handler SHALL build a `GoalOptions` from the parsed clap args AND SHALL invoke `codebus_core::verb::goal::run_goal(repo, options, on_event, None)` exactly once AND SHALL NOT contain inline calls to `agent::invoke`, `auto_commit`, source-signal drift detection, or `run_fix_loop`

#### Scenario: Query command body delegates to run_query

- **WHEN** the query subcommand handler in `codebus-cli/src/commands/query.rs` is invoked
- **THEN** the handler SHALL invoke `codebus_core::verb::query::run_query(repo, options, on_event, None)` exactly once AND SHALL NOT contain inline calls to `agent::invoke`

#### Scenario: Fix command body delegates to run_fix

- **WHEN** the fix subcommand handler in `codebus-cli/src/commands/fix.rs` is invoked
- **THEN** the handler SHALL invoke `codebus_core::verb::fix::run_fix(repo, options, on_event, None)` exactly once AND SHALL NOT contain inline calls to `agent::invoke`, `run_fix_loop`, or `auto_commit`

#### Scenario: CLI exit codes match per-error mapping

- **WHEN** the goal / query / fix library function returns a `VerbError` variant
- **THEN** the CLI handler SHALL produce the exit code per the existing policy: `VaultMissing` from query / fix → 2, `ConfigParse` → 2, `Spawn` → 1, `Internal` → 1; and the CLI handler SHALL be statically guaranteed to handle every variant via exhaustive `match`

#### Scenario: CLI stdout / stderr / exit code byte-equivalent

- **WHEN** any of `codebus goal "<text>"`, `codebus query "<text>"`, or `codebus fix` is invoked under identical environment, working directory, and mock claude output
- **THEN** the captured stdout bytes, stderr bytes, and exit code SHALL be identical to the pre-change implementation AND the existing 27+ integration tests in `codebus-cli/tests/` SHALL pass without modification to their golden assertions
