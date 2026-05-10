## MODIFIED Requirements

### Requirement: Init Subcommand Behavior

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, raw mirror, source repo `.gitignore` mutation, per-repo schema file write, manifest write, skill bundle authoring, Obsidian vault registration, and global config starter write. The global config starter write step SHALL invoke the `write_starter_config_if_missing` primitive against `~/.codebus/config.yaml`; when the file is absent the system SHALL create the parent directory if necessary and write the starter content; when the file already exists the system SHALL NOT read or overwrite it. `init` SHALL print one human-readable progress line to stdout per major step in default mode (the line SHALL begin with the marker `✓` followed by a step description). The raw mirror progress line SHALL include the total PII match count observed during the scan. When `--debug` is passed, `init` SHALL also emit additional `[debug]` lines per step describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs. A failure of the global config starter write step SHALL emit a stderr warning prefixed with `warning: global config` AND SHALL NOT cause `init` to exit non-zero (the rest of init having succeeded means the per-vault state is usable).

#### Scenario: Init with --repo flag targets the specified directory

- **WHEN** `codebus init --repo /tmp/testrepo` is invoked
- **THEN** the system SHALL initialize the vault under `/tmp/testrepo/.codebus/` AND SHALL NOT touch the current working directory's filesystem

#### Scenario: Init with --no-obsidian-register skips Obsidian step

- **WHEN** `codebus init --no-obsidian-register` is invoked against a system with Obsidian installed
- **THEN** the system SHALL complete all other init steps AND the Obsidian config file SHALL be unchanged AND init SHALL exit with status zero

#### Scenario: Init prints default progress lines for each major step

- **WHEN** `codebus init` runs successfully against a fresh git repository on a system without Obsidian, without `--debug`
- **THEN** stdout SHALL contain at least these distinct progress markers in order, each beginning with `✓`: vault layout, raw mirror, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring, global config (the Obsidian step SHALL emit a stderr hint instead of a stdout progress line when Obsidian is unavailable). The output SHALL NOT contain any line beginning with `[debug]`.

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

#### Scenario: Init writes global config starter when missing

- **WHEN** `codebus init` runs against a system where `~/.codebus/config.yaml` does not exist
- **THEN** after init the file `~/.codebus/config.yaml` SHALL exist AND SHALL parse successfully as YAML AND the parsed `pii.scanner` field SHALL equal `regex_basic` AND the parsed `pii.on_hit` field SHALL equal `mask` AND stdout SHALL contain a progress line beginning with `✓` containing the substring `global config`

#### Scenario: Init does not overwrite existing global config

- **WHEN** `codebus init` runs against a system where `~/.codebus/config.yaml` already exists with custom user content (for example `pii:\n  on_hit: warn\n`)
- **THEN** after init the file SHALL contain byte-identical content to its pre-init state AND the parsed `pii.on_hit` SHALL equal `warn` AND stdout SHALL contain a progress line beginning with `✓` indicating the file already exists

#### Scenario: Global config write failure does not abort init

- **WHEN** `codebus init` runs against a system where the parent directory of `~/.codebus/config.yaml` cannot be created (for example due to filesystem permissions)
- **THEN** the global config write step SHALL emit a stderr warning prefixed with `warning: global config` AND init SHALL exit with status zero AND all other init steps SHALL complete successfully

## ADDED Requirements

### Requirement: Claude Code Configuration Schema

The system SHALL load Claude Code agent configuration from `~/.codebus/config.yaml` under the top-level key `claude_code`. The schema SHALL define exactly three optional subsections (`goal`, `query`, `fix`), each with two optional fields (`model`, `effort`). The default values SHALL be: `goal: { model: opus, effort: high }`, `query: { model: haiku, effort: low }`, `fix: { model: sonnet, effort: medium }`. When the file is missing, the `claude_code` section is absent, a verb subsection is absent, or any individual field is absent, the system SHALL apply the corresponding default value. The system SHALL NOT validate the value of `model` or `effort` against any enumeration — strings SHALL be passed through to the agent CLI verbatim, allowing future model and effort identifiers without codebus-side schema changes. Unknown keys inside the `claude_code` section or its subsections SHALL be silently ignored to preserve forward-compatibility.

#### Scenario: Default config when file missing

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the loaded `ClaudeCodeConfig` SHALL equal `{ goal: { model: opus, effort: high }, query: { model: haiku, effort: low }, fix: { model: sonnet, effort: medium } }`

#### Scenario: Per-verb override applies only to that verb

- **WHEN** `~/.codebus/config.yaml` contains `claude_code:\n  goal:\n    model: sonnet\n` and no other `claude_code.*` keys
- **THEN** the loaded `ClaudeCodeConfig.goal.model` SHALL equal `sonnet` AND `ClaudeCodeConfig.goal.effort` SHALL equal `high` (the default) AND `ClaudeCodeConfig.query` SHALL equal `{ model: haiku, effort: low }` (defaults preserved)

#### Scenario: Arbitrary model string is accepted

- **WHEN** `~/.codebus/config.yaml` contains `claude_code:\n  goal:\n    model: claude-opus-4-7\n`
- **THEN** the loader SHALL succeed AND the loaded `ClaudeCodeConfig.goal.model` SHALL equal the literal string `claude-opus-4-7` AND no validation against an allowed-models list SHALL occur

#### Scenario: Unknown subkey silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `claude_code:\n  goal:\n    model: opus\n    future_field: hello\n`
- **THEN** the loader SHALL succeed AND the loaded `ClaudeCodeConfig.goal.model` SHALL equal `opus` AND the unknown `future_field` SHALL have no observable effect

### Requirement: Agent Spawn Model and Effort Forwarding

When the `goal`, `query`, or `fix` subcommand spawns its agent CLI child process, the system SHALL append `--model <X>` and `--effort <Y>` to the spawned command line, where `<X>` and `<Y>` are the values from the corresponding `claude_code.<verb>.model` and `claude_code.<verb>.effort` config fields after default fill-in. The forwarded flags SHALL be appended in addition to the existing `--tools`, `--allowedTools`, and `--permission-mode` flags defined by the per-verb subcommand requirements; their presence SHALL NOT remove or alter any other flag. When a config field's effective value is `None` (not currently reachable through the documented schema, but possible through a future deletion-style override), the system SHALL omit the corresponding flag entirely (rather than passing an empty string).

#### Scenario: Goal subcommand forwards configured model and effort

- **WHEN** the goal subcommand spawns the agent against a config containing `claude_code.goal: { model: opus, effort: high }`
- **THEN** the spawned command line SHALL include the flag pair `--model opus` AND the flag pair `--effort high`

#### Scenario: Query subcommand forwards configured model and effort

- **WHEN** the query subcommand spawns the agent against a config containing `claude_code.query: { model: haiku, effort: low }`
- **THEN** the spawned command line SHALL include the flag pair `--model haiku` AND the flag pair `--effort low`

#### Scenario: Fix subcommand forwards configured model and effort

- **WHEN** the fix subcommand spawns its agent against a config containing `claude_code.fix: { model: sonnet, effort: medium }`
- **THEN** the spawned command line SHALL include the flag pair `--model sonnet` AND the flag pair `--effort medium`

#### Scenario: User-provided non-default values flow through

- **WHEN** `~/.codebus/config.yaml` overrides `claude_code.goal.model` to `claude-opus-4-7` and the goal subcommand spawns
- **THEN** the spawned command line SHALL include the flag pair `--model claude-opus-4-7`

#### Scenario: Per-verb defaults differ across verbs

- **WHEN** `~/.codebus/config.yaml` is absent and the user runs `codebus goal "X"` then `codebus query "Y"` then `codebus fix`
- **THEN** the goal spawn SHALL include `--model opus --effort high`, the query spawn SHALL include `--model haiku --effort low`, and the fix spawn SHALL include `--model sonnet --effort medium`
