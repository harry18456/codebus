## MODIFIED Requirements

### Requirement: Init Subcommand Behavior

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, **source repo `.gitignore` mutation**, raw mirror, per-repo schema file write, manifest write, skill bundle authoring, Obsidian vault registration, and global config starter write. The source repo `.gitignore` mutation step SHALL precede the raw mirror so the byte-count signal recorded in the manifest reflects the post-init source state — otherwise subsequent verb invocations (`goal` / `query`) would compute a different source signal from a fresh walk and falsely conclude that drift has occurred. The global config starter write step SHALL invoke the `write_starter_config_if_missing` primitive against `~/.codebus/config.yaml`; when the file is absent the system SHALL create the parent directory if necessary and write the starter content; when the file already exists the system SHALL NOT read or overwrite it. In default output mode `init` SHALL emit a sequence of banners (defined by the `Banner Output for Verb Commands` requirement) covering at minimum the `Start`, `SyncDone`, `PiiSummary`, `CommitDone`, and `Done` banner variants. Default mode SHALL NOT emit per-step `✓ <internal-detail>` progress lines (these are reserved for `--debug` mode). When `--debug` is passed, `init` SHALL emit the same banner sequence AND additionally emit the per-step `✓ <internal-detail>` progress lines (vault layout, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring, vault settings write, global config starter, and any others implementation chooses) AND the `[debug]` lines describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs. A failure of the global config starter write step SHALL emit a stderr warning prefixed with `warning: global config` AND SHALL NOT cause `init` to exit non-zero (the rest of init having succeeded means the per-vault state is usable).

#### Scenario: Init with --repo flag targets the specified directory

- **WHEN** `codebus init --repo /tmp/testrepo` is invoked
- **THEN** the system SHALL initialize the vault under `/tmp/testrepo/.codebus/` AND SHALL NOT touch the current working directory's filesystem

#### Scenario: Init with --no-obsidian-register skips Obsidian step

- **WHEN** `codebus init --no-obsidian-register` is invoked against a system with Obsidian installed
- **THEN** the system SHALL complete all other init steps AND the Obsidian config file SHALL be unchanged AND init SHALL exit with status zero

#### Scenario: Default mode emits banner sequence not per-step progress lines

- **WHEN** `codebus init` runs successfully against a fresh git repository on a system without Obsidian, without `--debug`
- **THEN** stdout SHALL contain the `Start` banner (a line that includes the literal token `🚌` when emoji are enabled or the literal token `▶` when emoji are disabled), the `SyncDone` banner (a line beginning with `✓` or `✅` and containing the substring `同步完成` or equivalent translation key), the `PiiSummary` banner, the `CommitDone` banner, and the `Done` banner — in that order — AND stdout SHALL NOT contain a line matching the literal prefix `✓ vault layout:`, `✓ schema file:`, `✓ manifest:`, `✓ skill bundles:`, `✓ vault settings:`, `✓ vault internal .gitignore:`, `✓ source .gitignore:`, or `✓ global config:` (those are reserved for `--debug` mode)

#### Scenario: Debug mode adds per-step progress lines and [debug] traces

- **WHEN** `codebus init --debug` runs against the same fresh git repository
- **THEN** stdout SHALL contain every banner from the default-mode scenario AND SHALL ADDITIONALLY contain the per-step progress lines `✓ vault layout:`, `✓ schema file:`, `✓ manifest:`, `✓ skill bundles:`, `✓ vault settings:`, and `✓ global config:` AND stderr or stdout SHALL contain at least one line beginning with `[debug]`

#### Scenario: PII match count is reported in PiiSummary banner

- **WHEN** `codebus init` runs against a repository whose mirrored content yields exactly N PII matches
- **THEN** stdout SHALL contain the `PiiSummary` banner whose body contains the substring `hits <N>` where `<N>` is the integer total, regardless of whether `--debug` is set

#### Scenario: Init exits zero on first successful run

- **WHEN** init runs to completion against a fresh repository
- **THEN** the binary SHALL exit with status zero

#### Scenario: Init writes 3 skill bundles to per-project Claude Code directory

- **WHEN** `codebus init --repo <path>` runs against a target with no existing skill bundles at `<path>/.claude/skills/codebus-{goal,query,fix}/`
- **THEN** after init the three bundle directories SHALL exist under `<path>/.claude/skills/` AND each SHALL contain a `SKILL.md` file. The system SHALL NOT write to `~/.claude/skills/codebus-{goal,query,fix}/` for this invocation.

#### Scenario: Init writes global config starter when missing

- **WHEN** `codebus init` runs against a system where `~/.codebus/config.yaml` does not exist
- **THEN** after init the file `~/.codebus/config.yaml` SHALL exist AND SHALL parse successfully as YAML AND the parsed `pii.scanner` field SHALL equal `regex_basic` AND the parsed `pii.on_hit` field SHALL equal `mask`

#### Scenario: Init does not overwrite existing global config

- **WHEN** `codebus init` runs against a system where `~/.codebus/config.yaml` already exists with custom user content (for example `pii:\n  on_hit: warn\n`)
- **THEN** after init the file SHALL contain byte-identical content to its pre-init state AND the parsed `pii.on_hit` SHALL equal `warn`

#### Scenario: Global config write failure does not abort init

- **WHEN** `codebus init` runs against a system where the parent directory of `~/.codebus/config.yaml` cannot be created (for example due to filesystem permissions)
- **THEN** the global config write step SHALL emit a stderr warning prefixed with `warning: global config` AND init SHALL exit with status zero AND all other init steps SHALL complete successfully

#### Scenario: Subsequent goal invocation does not trigger redundant re-sync

- **WHEN** `codebus init --repo <repo>` runs to completion against a fresh git repository, immediately followed by `codebus goal "..." --repo <repo>` with no intervening source mutation
- **THEN** the goal invocation SHALL NOT emit the `SyncStart` or `SyncDone` banner (the source-signal drift detection SHALL conclude no drift exists, skipping raw mirror re-sync)
