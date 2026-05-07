## ADDED Requirements

### Requirement: Render stage banners during goal flow

The system SHALL emit a stage banner before and after each non-LLM stage of the `--goal` flow so the user can identify which stage is currently executing. The required stages are: raw sync, PII summary, lint, each fix iteration, and auto-commit. Each "done" banner SHALL carry the elapsed milliseconds for that stage. Banner emoji vs symbol prefix SHALL follow the existing emoji-mode resolution chain — this requirement does not introduce a parallel mode-resolution path.

#### Scenario: Sync start banner before raw_sync

- **WHEN** the goal flow is about to invoke raw_sync
- **THEN** the renderer emits a `SyncStart` banner whose body identifies the sync stage

#### Scenario: Sync done banner after raw_sync

- **WHEN** raw_sync returns successfully
- **THEN** the renderer emits a `SyncDone` banner carrying the number of files copied, total mebibytes copied, and elapsed milliseconds

#### Scenario: Lint start banner before lint_wiki

- **WHEN** the goal flow is about to invoke lint_wiki
- **THEN** the renderer emits a `LintStart` banner

#### Scenario: Lint done banner after lint_wiki

- **WHEN** lint_wiki returns a result
- **THEN** the renderer emits a `LintDone` banner carrying the error count, warning count, and elapsed milliseconds

#### Scenario: Fix iteration start banner

- **WHEN** the fix loop is about to start iteration `i` out of a maximum of `max`
- **THEN** the renderer emits a `FixIterStart { i, max }` banner

#### Scenario: Fix iteration done banner

- **WHEN** fix iteration `i` returns
- **THEN** the renderer emits a `FixIterDone` banner carrying the iteration index, number of issues fixed in this iteration, number of issues remaining, and elapsed milliseconds for this iteration

#### Scenario: Commit done banner after auto_commit

- **WHEN** auto_commit succeeds with sha `abc1234567...`
- **THEN** the renderer emits a `CommitDone` banner whose body contains the short (7-char) prefix of that sha

#### Scenario: Stage banners follow existing emoji mode

- **WHEN** the renderer emits any stage banner with emoji enabled
- **THEN** the line is prefixed with the same emoji-mode glyph the existing lifecycle banners use, not a new symbol set

### Requirement: Render PII summary banner

The system SHALL emit one PII summary banner after the raw sync stage completes, regardless of which scanner was selected (including `null`). The banner SHALL carry the scanner name, the count of files scanned, the count of files with PII matches, and the on-hit action that was applied. This requirement makes PII activity visible without requiring the user to enable verbose logging.

#### Scenario: PII summary with NullScanner

- **WHEN** the raw sync stage completes with the default `null` scanner over 1289 files
- **THEN** the renderer emits a `PiiSummary` banner reporting scanner `null`, scanned 1289, hits 0, action `warn`

#### Scenario: PII summary with regex_basic scanner and skip action

- **WHEN** the raw sync stage completes with `regex_basic` scanner that matched 3 files and on-hit `skip`
- **THEN** the renderer emits a `PiiSummary` banner reporting scanner `regex_basic`, scanned 1289, hits 3, action `skip`

### Requirement: Stage banners do not block on stdout failures

The system SHALL treat banner rendering as best-effort. If the underlying `println!` fails (e.g., closed pipe), the goal flow SHALL continue to completion and the run result SHALL NOT be marked as failed solely because a banner could not be written. This matches the behavior already in place for the existing four lifecycle banners.

#### Scenario: Goal flow completes when stdout pipe is closed

- **WHEN** stdout is closed mid-run and a stage banner cannot be written
- **THEN** the goal flow continues, the wiki is still updated, and the process exit code reflects the goal outcome — not the banner I/O error
