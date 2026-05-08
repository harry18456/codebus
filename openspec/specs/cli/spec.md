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

Each of the four subcommands `goal`, `query`, `lint`, `fix` SHALL be a stub that prints a single line containing the literal substring `not yet implemented` to stderr (the line MAY include the verb name as additional context) AND exit with non-zero status. The `init` subcommand SHALL NOT match this requirement (its behavior is defined by `Init Subcommand Behavior`). The binary MUST NOT panic, MUST NOT block waiting for input, AND MUST NOT silently no-op for any of the four stub verbs. The four stub verbs MAY accept the `--debug` global flag silently — they SHALL NOT emit `[debug]` lines because their entire body is the not-yet-implemented stub.

#### Scenario: Each remaining stub verb exits with not-yet-implemented message

- **WHEN** any of `codebus goal`, `codebus query`, `codebus lint`, `codebus fix` is invoked
- **THEN** the binary SHALL write a line to stderr containing the substring `not yet implemented` AND SHALL exit with non-zero status AND SHALL NOT panic AND SHALL NOT block

#### Scenario: Init no longer matches stub behavior

- **WHEN** `codebus init` is invoked against a writable target directory
- **THEN** the binary SHALL NOT print `not yet implemented` AND SHALL NOT exit with non-zero status due to stub behavior; init's behavior is governed by `Init Subcommand Behavior` instead

#### Scenario: Stub verbs do not accept positional arguments yet

- **WHEN** `codebus goal "some text"` is invoked
- **THEN** clap MAY accept the positional argument silently OR reject it; in either case the binary SHALL exit with non-zero status and SHALL NOT panic. Real positional argument handling for `goal` / `query` is added in subsequent changes

#### Scenario: Stub verbs accept --debug silently

- **WHEN** any of `codebus goal --debug`, `codebus query --debug`, `codebus lint --debug`, `codebus fix --debug` is invoked
- **THEN** clap SHALL accept the flag without rejecting it AND the verb SHALL still exit non-zero with the stub `not yet implemented` message AND SHALL NOT emit any `[debug]` line


<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->

---
### Requirement: Init Subcommand Behavior

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, raw mirror, source repo `.gitignore` mutation, per-repo schema file write, manifest write, skill bundle authoring, and Obsidian vault registration. `init` SHALL print one human-readable progress line to stdout per major step in default mode (the line SHALL begin with the marker `✓` followed by a step description). The raw mirror progress line SHALL include the total PII match count observed during the scan. When `--debug` is passed, `init` SHALL also emit additional `[debug]` lines per step describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs.

#### Scenario: Init with --repo flag targets the specified directory

- **WHEN** `codebus init --repo /tmp/testrepo` is invoked
- **THEN** the system SHALL initialize the vault under `/tmp/testrepo/.codebus/` AND SHALL NOT touch the current working directory's filesystem

#### Scenario: Init with --no-obsidian-register skips Obsidian step

- **WHEN** `codebus init --no-obsidian-register` is invoked against a system with Obsidian installed
- **THEN** the system SHALL complete all other init steps AND the Obsidian config file SHALL be unchanged AND init SHALL exit with status zero

#### Scenario: Init prints default progress lines for each major step

- **WHEN** `codebus init` runs successfully against a fresh git repository on a system without Obsidian, without `--debug`
- **THEN** stdout SHALL contain at least these distinct progress markers in order, each beginning with `✓`: vault layout, raw mirror, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring (the Obsidian step SHALL emit a stderr hint instead of a stdout progress line when Obsidian is unavailable). The output SHALL NOT contain any line beginning with `[debug]`.

#### Scenario: Raw mirror progress line includes PII match count

- **WHEN** `codebus init` runs against a repository whose mirrored content yields exactly N PII matches
- **THEN** the stdout progress line beginning with `✓` and identifying the raw mirror step SHALL contain the substring `<N> PII matches` where `<N>` is the integer total

#### Scenario: Raw mirror progress line includes zero count when no PII present

- **WHEN** `codebus init` runs against a repository whose mirrored content yields zero PII matches
- **THEN** the stdout progress line for the raw mirror step SHALL contain the substring `0 PII matches`

#### Scenario: Init exits zero on first successful run

- **WHEN** init runs to completion against a fresh repository
- **THEN** the binary SHALL exit with status zero

#### Scenario: Init writes 3 skill bundles to per-project Claude Code directory

- **WHEN** `codebus init --repo <path>` runs against a target with no existing skill bundles at `<path>/.claude/skills/codebus-{goal,query,fix}/`
- **THEN** after init the three bundle directories SHALL exist under `<path>/.claude/skills/` AND each SHALL contain a `SKILL.md` file. The system SHALL NOT write to `~/.claude/skills/codebus-{goal,query,fix}/` for this invocation.


<!-- @trace
source: v3-pii
updated: 2026-05-09
code:
  - codebus-core/src/pii/scanners/null_scanner.rs
  - codebus-core/src/pii/scanners/mod.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/lib.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/pii/provider.rs
  - codebus-core/src/pii/mod.rs
  - codebus-core/src/pii/scanners/regex_basic.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit additional `[debug]` lines describing internal state and operations beyond the default `✓` progress lines. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]`.

In this change, only the `init` verb implements debug output content; the four stub verbs (`goal` / `query` / `lint` / `fix`) MAY accept the `--debug` flag silently without emitting any debug content (their stub behavior is unchanged).

#### Scenario: --debug flag accepted at top-level position

- **WHEN** `codebus --debug init --repo <path>` is invoked against a writable target
- **THEN** stdout SHALL contain at least one line beginning with `[debug]` AND init SHALL otherwise complete successfully AND exit with status zero

#### Scenario: --debug flag accepted at subcommand position

- **WHEN** `codebus init --debug --repo <path>` is invoked against a writable target
- **THEN** the binary SHALL behave identically to `codebus --debug init --repo <path>` (same exit code, same `[debug]` line presence)

#### Scenario: Without --debug, no debug lines are emitted

- **WHEN** `codebus init --repo <path>` runs successfully (no `--debug` flag anywhere)
- **THEN** stdout SHALL NOT contain any line beginning with `[debug]`

<!-- @trace
source: v3-init
updated: 2026-05-08
code:
  - codebus-core/src/vault/raw_sync.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/source_gitignore.rs
  - codebus-core/src/schema/mod.rs
  - Cargo.toml
  - codebus-core/src/schema/neutral.md
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-cli/src/main.rs
  - codebus-core/Cargo.toml
  - codebus-core/src/vault/obsidian_register.rs
  - codebus-core/src/vault/layout.rs
  - codebus-core/src/lib.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/sanity_check.rs
  - codebus-core/src/vault/mod.rs
  - codebus-cli/Cargo.toml
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
  - codebus-core/tests/schema_neutrality.rs
-->