## ADDED Requirements

### Requirement: Goal Content Verify CLI Behavior

The `codebus goal "<goal>"` subcommand SHALL participate in the optional content verification stage defined by the `verb-library` capability's `Goal Content Verification and Repair` requirement, without introducing any new top-level subcommand (the registered subcommand set is unchanged; content verification is an internal stage of `run_goal`).

The CLI SHALL resolve `goal.content_verify` from the shared `goal.*` config namespace (default `false`; the CLI SHALL NOT read the app-only `app.*` namespace for this key). When `false`, `codebus goal` behavior SHALL be unchanged from the deterministic-only flow. When `true`, the CLI SHALL pass the originating goal text into `run_goal` so the off-goal defect check can run, the live stream SHALL render the additional verify and repair spawns through the existing agent stream rendering, and the run-log / `GoalReport` SHALL reflect the content-review status. The subcommand exit status and `auto_commit` behavior SHALL be unchanged by content verification (residual content defects are best-effort, not a failure, and never block the commit).

#### Scenario: Default off leaves CLI flow unchanged

- **WHEN** `codebus goal "<goal>"` runs and `goal.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the exit code and `auto_commit` behavior SHALL be unchanged

#### Scenario: Enabled CLI runs the stage and surfaces the stream

- **WHEN** `codebus goal "describe auth"` runs with `goal.content_verify` set to `true`
- **THEN** the originating goal text SHALL be supplied to `run_goal` for the off-goal check AND the verify/repair spawns SHALL be rendered in the live stream AND the run reflects the content-review status

#### Scenario: No new subcommand is registered

- **WHEN** `codebus --help` is invoked with this change applied
- **THEN** the top-level subcommand list SHALL be unchanged AND no content-verify subcommand SHALL be added
