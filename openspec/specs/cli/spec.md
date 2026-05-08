# cli Specification

## Purpose

TBD - created by archiving change 'v3-workspace'. Update Purpose after archive.

## Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register exactly five subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand.

#### Scenario: Help output lists exactly the five subcommands

- **WHEN** `codebus --help` is invoked
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the binary SHALL print a single line containing the cargo package version of the `codebus-cli` crate AND SHALL exit with status zero

#### Scenario: Unknown subcommand is rejected by clap

- **WHEN** `codebus mcp` or `codebus randomverb` is invoked
- **THEN** the binary SHALL print a clap error message to stderr identifying the unknown subcommand AND SHALL exit with non-zero status


<!-- @trace
source: v3-workspace
updated: 2026-05-08
code:
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/main.rs
  - codebus-app/src/lib.rs
  - codebus-cli/src/commands/goal.rs
  - Cargo.toml
  - codebus-app/Cargo.toml
  - codebus-app/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/mod.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/lib.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: No-Arg Defaults to Init Dispatch

Invoking the `codebus` binary with zero arguments SHALL be treated equivalently to invoking `codebus init` with the subcommand's default arguments. The dispatch path SHALL be unified: the binary MUST NOT have separate code paths for "no-arg" and "explicit init" beyond the subcommand resolution layer.

#### Scenario: Bare invocation routes to init handler

- **WHEN** `codebus` is invoked with no arguments
- **THEN** the binary SHALL invoke the `init` subcommand handler exactly as if the user had typed `codebus init`

#### Scenario: Explicit init invocation produces identical behavior to bare

- **WHEN** `codebus` and `codebus init` are both invoked under identical environment and working directory
- **THEN** their stderr / stdout output AND exit status SHALL be identical


<!-- @trace
source: v3-workspace
updated: 2026-05-08
code:
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/main.rs
  - codebus-app/src/lib.rs
  - codebus-cli/src/commands/goal.rs
  - Cargo.toml
  - codebus-app/Cargo.toml
  - codebus-app/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/mod.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/lib.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->

---
### Requirement: Stub Verb Exit Behavior

Each of the five subcommands (`init`, `goal`, `query`, `lint`, `fix`) in this change SHALL be a stub that prints a single line containing the literal substring `not yet implemented` to stderr (the line MAY include the verb name as additional context) AND exit with non-zero status. The binary MUST NOT panic, MUST NOT block waiting for input, AND MUST NOT silently no-op.

#### Scenario: Each verb stub exits with not-yet-implemented message

- **WHEN** any of `codebus init`, `codebus goal`, `codebus query`, `codebus lint`, `codebus fix` is invoked
- **THEN** the binary SHALL write a line to stderr containing the substring `not yet implemented` AND SHALL exit with non-zero status AND SHALL NOT panic AND SHALL NOT block

#### Scenario: Stub verbs do not accept positional arguments yet

- **WHEN** `codebus goal "some text"` is invoked
- **THEN** clap MAY accept the positional argument silently OR reject it; in either case the binary SHALL exit with non-zero status and SHALL NOT panic. Real positional argument handling for `goal` / `query` is added in subsequent changes

<!-- @trace
source: v3-workspace
updated: 2026-05-08
code:
  - codebus-cli/src/commands/query.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/main.rs
  - codebus-app/src/lib.rs
  - codebus-cli/src/commands/goal.rs
  - Cargo.toml
  - codebus-app/Cargo.toml
  - codebus-app/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/mod.rs
  - codebus-cli/src/commands/lint.rs
  - codebus-core/src/lib.rs
  - codebus-cli/Cargo.toml
  - codebus-cli/src/commands/fix.rs
tests:
  - codebus-cli/tests/cli_routing.rs
-->