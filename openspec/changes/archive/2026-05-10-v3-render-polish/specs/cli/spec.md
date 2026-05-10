## MODIFIED Requirements

### Requirement: Init Subcommand Behavior

The `init` subcommand SHALL accept the flags `--repo <PATH>` (default: current working directory) and `--no-obsidian-register` (boolean flag). When invoked, `init` SHALL orchestrate (in order): pre-flight sanity check, vault layout creation, raw mirror, source repo `.gitignore` mutation, per-repo schema file write, manifest write, skill bundle authoring, Obsidian vault registration, and global config starter write. The global config starter write step SHALL invoke the `write_starter_config_if_missing` primitive against `~/.codebus/config.yaml`; when the file is absent the system SHALL create the parent directory if necessary and write the starter content; when the file already exists the system SHALL NOT read or overwrite it. In default output mode `init` SHALL emit a sequence of banners (defined by the `Banner Output for Verb Commands` requirement) covering at minimum the `Start`, `SyncDone`, `PiiSummary`, `CommitDone`, and `Done` banner variants. Default mode SHALL NOT emit per-step `✓ <internal-detail>` progress lines (these are reserved for `--debug` mode). When `--debug` is passed, `init` SHALL emit the same banner sequence AND additionally emit the per-step `✓ <internal-detail>` progress lines (vault layout, source `.gitignore` mutation, schema file write, manifest write, skill bundle authoring, vault settings write, global config starter, and any others implementation chooses) AND the `[debug]` lines describing internal decisions, fs operations, computed source signal values, and target paths. `init` SHALL exit with status zero on success and non-zero only if a sanity-check refusal or unrecoverable filesystem error occurs. A failure of the global config starter write step SHALL emit a stderr warning prefixed with `warning: global config` AND SHALL NOT cause `init` to exit non-zero (the rest of init having succeeded means the per-vault state is usable).

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

### Requirement: Debug Flag Output

The `codebus` binary SHALL accept `--debug` as a global flag, available at the top-level command and inheritable by every subcommand (e.g., `codebus --debug init`, `codebus init --debug` SHALL behave equivalently). When `--debug` is set, the binary's verb handlers SHALL emit (in addition to the default-mode banner sequence) the per-step `✓ <internal-detail>` progress lines describing intermediate orchestration outcomes AND the `[debug]` lines describing internal decisions, fs operations, computed values, and target paths. When `--debug` is NOT set, the binary SHALL NOT emit any line beginning with `[debug]` AND SHALL NOT emit per-step `✓ <internal-detail>` progress lines (only the higher-level banner sequence emerges in default mode).

#### Scenario: Default mode suppresses [debug] lines

- **WHEN** `codebus init` runs without `--debug`
- **THEN** neither stdout nor stderr SHALL contain any line beginning with `[debug]`

#### Scenario: Debug mode emits both detail and trace lines

- **WHEN** `codebus init --debug` runs against any repository
- **THEN** stdout SHALL contain at least one per-step `✓ <internal-detail>` progress line AND at least one `[debug]` trace line

## ADDED Requirements

### Requirement: Banner Output for Verb Commands

The system SHALL render run-lifecycle messages as a sequence of structured banners with codebus brand identity (the bus / boarding metaphor). The banner set SHALL contain at minimum these ten variants, each carrying a fixed set of payload fields: `Start { repo_path }`, `Goal { goal_text }`, `SyncStart`, `SyncDone { files, mib, elapsed_ms }`, `PiiSummary { scanner, scanned, hits, action }`, `LintStart`, `LintDone { errors, warns, elapsed_ms }`, `CommitDone { sha7 }`, `Done { wiki_path }`, `Hint { wiki_path }`. Each banner SHALL render to a single stdout line. The system SHALL provide both an emoji-leading form (e.g., `🚌 來囉來囉~ CodeBus 駛入 <path>...` for `Start`) and a symbol-leading fallback form (e.g., `▶ 來囉來囉~ CodeBus 駛入 <path>...`); selection between forms SHALL be governed by the `Environment-Aware Output Styling` requirement. The verb command modules (`init`, `goal`, `query`, `fix`, `lint`) SHALL invoke the appropriate banner sequence at lifecycle transitions in their own stdout (NOT in the spawned agent's stdout — agent output remains a passthrough).

#### Scenario: Start banner appears at verb invocation

- **WHEN** any of `codebus init`, `codebus goal "X"`, `codebus query "X"`, `codebus fix`, or `codebus lint` is invoked against a valid repo, in default output mode with emoji enabled
- **THEN** the first stdout line SHALL be the `Start` banner — a single line containing `🚌` and the resolved repo path

#### Scenario: Done banner appears at successful completion

- **WHEN** `codebus init` or `codebus goal "X"` runs to successful completion in default output mode with emoji enabled
- **THEN** the last stdout banner before process exit SHALL be the `Done` banner — a single line containing `🎉` and the wiki path

#### Scenario: SyncDone banner reports counts and elapsed time

- **WHEN** the raw mirror sync runs (during init OR during goal with drift detected)
- **THEN** stdout SHALL contain the `SyncDone` banner — a single line whose body matches the pattern `<files> 檔, <mib> MiB, <elapsed_ms> ms` for some integer `files`, decimal `mib`, and integer `elapsed_ms`

#### Scenario: CommitDone banner reports short SHA

- **WHEN** the vault auto-commit step produces a non-empty commit
- **THEN** stdout SHALL contain the `CommitDone` banner — a single line whose body contains the 7-character short SHA

##### Example: init banner sequence on a fresh repo

- **GIVEN** a fresh git repo with one tracked file, Obsidian installed and registered
- **WHEN** `codebus init` runs in default mode with emoji enabled
- **THEN** stdout banner sequence SHALL be: `🚌 來囉來囉~ CodeBus 駛入 ./...`, `✓ 同步完成 (1 檔, 0.0 MiB, <ms> ms)`, `🛡 PII：regex_basic, scanned 1, hits 0, action mask`, `📌 commit <sha7>`, `🎉 掰掰~下車囉！wiki 已生成於 ./.codebus/wiki`, `💡 請用 Obsidian 開 ./.codebus/wiki`

#### Scenario: Symbol fallback used when emoji disabled

- **WHEN** `codebus init` runs in an environment where emoji are disabled (non-TTY OR `NO_COLOR=1`)
- **THEN** the `Start` banner SHALL begin with `▶` (not `🚌`) AND the `Done` banner SHALL begin with `✓` (not `🎉`) AND the `LintStart` banner SHALL begin with `~` (not `🔍`)

### Requirement: Environment-Aware Output Styling

The system SHALL detect terminal capabilities once at process start and apply them uniformly across all banners and lint text output. Three independent capability flags SHALL be derived: `use_emoji` SHALL be true when stdout is a TTY (per `std::io::IsTerminal`) and false otherwise; `use_color` SHALL be true when stdout is a TTY AND the environment variable `NO_COLOR` is unset; `use_hyperlinks` SHALL be true when `use_color` is true AND the `supports-hyperlinks` crate reports stdout supports OSC 8 hyperlinks. The system SHALL NOT consult `~/.codebus/config.yaml` for any output-styling field (no `emoji:` key, no `color:` key, no `hyperlinks:` key). The system SHALL NOT recognize a `--emoji on|off`, `--no-emoji`, or `NO_EMOJI` env override; presence of these inputs SHALL either fail clap parsing (for the flags) or be silently ignored (for the env var).

#### Scenario: NO_COLOR disables ANSI color but keeps emoji

- **WHEN** `codebus init` runs with `NO_COLOR=1` set in the environment, in a TTY
- **THEN** stdout banners SHALL still include emoji glyphs AND lint output SHALL NOT contain any ANSI escape sequence (no `\x1b[31m`, no `\x1b[33m`, no `\x1b]8;;...\x1b\\`)

#### Scenario: Non-TTY pipe disables emoji and color and hyperlinks

- **WHEN** `codebus init` is invoked with stdout redirected to a file or piped to another process
- **THEN** stdout content SHALL NOT contain any emoji glyph (no `🚌`, no `🎉`, no `🛡`) AND SHALL NOT contain any ANSI escape sequence — banners use only the symbol-fallback form

#### Scenario: --emoji flag is rejected by clap

- **WHEN** the user invokes `codebus init --emoji on`
- **THEN** clap argument parsing SHALL reject the unknown `--emoji` flag AND the binary SHALL exit with non-zero status

#### Scenario: NO_EMOJI env is silently ignored

- **WHEN** `codebus init` runs with `NO_EMOJI=1` set in the environment, in a TTY (with `NO_COLOR` unset)
- **THEN** stdout banners SHALL still include emoji glyphs (the env var has no observable effect on output)

#### Scenario: Detection runs once per process

- **WHEN** any single verb invocation emits multiple banners over its lifecycle
- **THEN** the emoji / color / hyperlink decisions SHALL be consistent across every banner in that invocation (no per-banner re-detection)
