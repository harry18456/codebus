## ADDED Requirements

### Requirement: Quiz Content Verify CLI Behavior

The `codebus quiz "<topic>"` subcommand SHALL participate in the optional content verification stage defined by the `quiz` capability's `Quiz Content Verification and Repair` requirement, without introducing any new top-level subcommand or sub-action (content verification is an internal spawn; Subcommand Registration is unchanged).

The CLI SHALL resolve `quiz.content_verify` from the shared `quiz.*` config namespace (default `false`; the CLI SHALL NOT read the app-only `app.*` namespace for this key). When `false`, `codebus quiz` behaviour SHALL be unchanged from the deterministic-only flow. When `true`, the CLI SHALL pass the originating topic (the `"<topic>"` argument — the CLI is always the Goal flow) into `run_quiz_generate` so the off-topic defect check can run, and the live stream SHALL render the additional verify and repair spawns through the existing agent stream rendering. The persisted quiz file's caller frontmatter SHALL include the `content_review` field as defined by the `quiz` capability. Exit status SHALL remain unchanged by content verification (residual content defects are best-effort, not a failure).

#### Scenario: Default off leaves CLI flow unchanged

- **WHEN** `codebus quiz "<topic>"` runs and `quiz.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the persisted quiz SHALL NOT contain a `content_review` field AND the exit status contract SHALL be unchanged

#### Scenario: Enabled CLI threads topic and surfaces stream

- **WHEN** `codebus quiz "JWT lifecycle"` runs with `quiz.content_verify` set to `true`
- **THEN** the originating topic SHALL be supplied to `run_quiz_generate` for the off-topic check AND the verify/repair spawns SHALL be rendered in the live stream AND the persisted frontmatter SHALL carry `content_review`

#### Scenario: No new subcommand is registered

- **WHEN** `codebus --help` is invoked with this change applied
- **THEN** the top-level subcommand list SHALL be unchanged AND no content-verify subcommand or `quiz` sub-action SHALL be added
