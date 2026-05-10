## MODIFIED Requirements

### Requirement: Banner Output for Verb Commands

The system SHALL render run-lifecycle messages as a sequence of structured banners with codebus brand identity (the bus / boarding metaphor). The banner set SHALL contain at minimum these ten variants, each carrying a fixed set of payload fields: `Start { repo_path }`, `Goal { goal_text }`, `SyncStart`, `SyncDone { files, mib, elapsed_ms }`, `PiiSummary { scanner, scanned, hits, action }`, `LintStart`, `LintDone { errors, warns, elapsed_ms }`, `CommitDone { sha7 }`, `Done { wiki_path }`, `Hint { wiki_path }`. Each banner SHALL render to a single stdout line. The `PiiSummary` `action` field SHALL be formatted as `critical=<X>, warn=<Y>` where `<X>` is always `mask` (security floor per `pii-filter` capability `On-Hit Policy Default`) and `<Y>` is the resolved Warn-severity policy from `pii.on_hit` (one of `warn` / `skip` / `mask`). The system SHALL provide both an emoji-leading form (e.g., `🚌 來囉來囉~ CodeBus 駛入 <path>...` for `Start`) and a symbol-leading fallback form (e.g., `▶ 來囉來囉~ CodeBus 駛入 <path>...`); selection between forms SHALL be governed by the `Environment-Aware Output Styling` requirement. The verb command modules (`init`, `goal`, `query`, `fix`, `lint`) SHALL invoke the appropriate banner sequence at lifecycle transitions in their own stdout (NOT in the spawned agent's stdout — agent output remains a passthrough).

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

- **GIVEN** a fresh git repo with one tracked file, Obsidian installed and registered, default `pii.on_hit` (Warn)
- **WHEN** `codebus init` runs in default mode with emoji enabled
- **THEN** stdout banner sequence SHALL be: `🚌 來囉來囉~ CodeBus 駛入 ./...`, `✓ 同步完成 (1 檔, 0.0 MiB, <ms> ms)`, `🛡 PII：regex_basic, scanned 1, hits 0, action critical=mask, warn=warn`, `📌 commit <sha7>`, `🎉 掰掰~下車囉！wiki 已生成於 ./.codebus/wiki`, `💡 請用 Obsidian 開 ./.codebus/wiki`

#### Scenario: PiiSummary action field reflects per-severity dispatch

- **WHEN** `codebus init` runs with `pii.on_hit: mask` configured
- **THEN** the `PiiSummary` banner action field SHALL contain the substring `critical=mask, warn=mask`

##### Example: PiiSummary action under default Warn policy

- **GIVEN** a config file omitting `pii.on_hit` (so the loader applies the default)
- **WHEN** `codebus init` runs and emits the `PiiSummary` banner
- **THEN** the banner body SHALL contain the substring `action critical=mask, warn=warn`

#### Scenario: Symbol fallback used when emoji disabled

- **WHEN** `codebus init` runs in an environment where emoji are disabled (non-TTY OR `NO_COLOR=1`)
- **THEN** the `Start` banner SHALL begin with `▶` (not `🚌`) AND the `Done` banner SHALL begin with `✓` (not `🎉`) AND the `LintStart` banner SHALL begin with `~` (not `🔍`)
