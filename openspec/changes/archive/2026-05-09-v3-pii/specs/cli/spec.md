## MODIFIED Requirements

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
