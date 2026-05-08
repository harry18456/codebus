## ADDED Requirements

### Requirement: Init Subcommand Behavior

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, raw mirror, source repo `.gitignore` mutation, per-repo schema file write, manifest write, skill bundle authoring, and Obsidian vault registration. `init` SHALL print one human-readable progress line to stdout per major step in default mode (the line SHALL begin with the marker `✓` followed by a step description). When `--debug` is passed, `init` SHALL also emit additional `[debug]` lines per step describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs.

#### Scenario: Init with --repo flag targets the specified directory

- **WHEN** `codebus init --repo /tmp/testrepo` is invoked
- **THEN** the system SHALL initialize the vault under `/tmp/testrepo/.codebus/` AND SHALL NOT touch the current working directory's filesystem

#### Scenario: Init with --no-obsidian-register skips Obsidian step

- **WHEN** `codebus init --no-obsidian-register` is invoked against a system with Obsidian installed
- **THEN** the system SHALL complete all other init steps AND the Obsidian config file SHALL be unchanged AND init SHALL exit with status zero

#### Scenario: Init prints default progress lines for each major step

- **WHEN** `codebus init` runs successfully against a fresh git repository on a system without Obsidian, without `--debug`
- **THEN** stdout SHALL contain at least these distinct progress markers in order, each beginning with `✓`: vault layout, raw mirror, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring (the Obsidian step SHALL emit a stderr hint instead of a stdout progress line when Obsidian is unavailable). The output SHALL NOT contain any line beginning with `[debug]`.

#### Scenario: Init exits zero on first successful run

- **WHEN** init runs to completion against a fresh repository
- **THEN** the binary SHALL exit with status zero

#### Scenario: Init writes 3 skill bundles to per-project Claude Code directory

- **WHEN** `codebus init --repo <path>` runs against a target with no existing skill bundles at `<path>/.claude/skills/codebus-{goal,query,fix}/`
- **THEN** after init the three bundle directories SHALL exist under `<path>/.claude/skills/` AND each SHALL contain a `SKILL.md` file. The system SHALL NOT write to `~/.claude/skills/codebus-{goal,query,fix}/` for this invocation.

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

## MODIFIED Requirements

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
