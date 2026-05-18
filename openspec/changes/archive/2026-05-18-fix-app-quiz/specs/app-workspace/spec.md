## MODIFIED Requirements

### Requirement: Quiz Answering and Summary

The answering view SHALL present one question per screen with four choices. After the user selects a choice and submits, the system SHALL reveal whether it was correct by comparing the selection to the quiz markdown `Answer` field client-side (no agent spawn) and SHALL show the `Explanation`. For an incorrect answer the system SHALL additionally render a `[← Back to wiki page]` affordance. After the final question, a summary SHALL display the score and a pass/fail outcome computed client-side using `app.quiz.pass_threshold`. The threshold value SHALL be sourced from the application settings store (the same `app.quiz.pass_threshold` key the Settings modal binds); it SHALL NOT be a hardcoded component constant. When the `app.quiz.pass_threshold` key is absent the value SHALL default to 80; changing the setting SHALL change the summary pass/fail boundary on the next finished quiz.

#### Scenario: Correct answer revealed without spawn

- **WHEN** the user submits the choice matching the question's `Answer` field
- **THEN** the system SHALL mark it correct AND show the `Explanation` AND SHALL NOT spawn an agent to grade

#### Scenario: Incorrect answer offers wiki return

- **WHEN** the user submits a choice not matching the `Answer` field
- **THEN** the system SHALL mark it incorrect, show the `Explanation`, AND render a `[← Back to wiki page]` affordance

#### Scenario: Summary applies pass threshold

- **GIVEN** `app.quiz.pass_threshold` is 80
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a passing outcome

#### Scenario: Changing the threshold setting changes the outcome

- **GIVEN** `app.quiz.pass_threshold` is set to 90 in the settings store
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a failing outcome

### Requirement: Quiz History List

The Quiz tab SHALL list prior attempts grouped by page or topic slug, derived by scanning `<vault>/.codebus/quiz/`. Selecting an attempt row SHALL open that attempt's persisted markdown (the attempt detail view). The view-generation-log affordance SHALL live inside that opened attempt detail view (not on the history row) and SHALL be present only when the attempt has a non-null `events_log`. Activating it SHALL open a centered modal dialog (with a backdrop, dismissible, consistent with the app's existing modal pattern) that renders that attempt's generate-spawn events through the existing agent stream rendering pipeline; displaying only the `events_log` file path SHALL NOT satisfy this requirement. Dismissing the modal SHALL return to the attempt detail view. The history row itself SHALL NOT inline-expand a generation-log panel.

#### Scenario: History reflects non-destructive retries

- **WHEN** two quizzes have been generated for the same topic slug
- **THEN** the history SHALL list two distinct attempt rows under that slug AND opening either SHALL show that attempt's own questions and answers

#### Scenario: View-generation-log lives in the attempt detail view

- **WHEN** the user opens an attempt that has a non-null `events_log`
- **THEN** the attempt detail view SHALL expose a view-generation-log affordance AND the history row SHALL NOT itself render or inline-expand a generation-log panel

#### Scenario: View-generation-log opens a modal timeline

- **WHEN** the user activates the view-generation-log affordance inside an opened attempt
- **THEN** the system SHALL open a centered modal dialog rendering that attempt's generate-spawn events.jsonl through the existing agent stream rendering AND dismissing the modal SHALL return to the attempt detail view

#### Scenario: View-generation-log is not a bare path

- **WHEN** the user activates the view-generation-log affordance inside an opened attempt
- **THEN** the modal content SHALL contain stream-rendered event items (thought / tool-use / result) AND SHALL NOT be limited to the events.jsonl path string

#### Scenario: No view-generation-log affordance without an events log

- **WHEN** the user opens an attempt whose `events_log` is null
- **THEN** the attempt detail view SHALL NOT render a view-generation-log affordance

### Requirement: Tauri IPC Commands for Quiz Plan and Generate Lifecycle

The system SHALL register exactly six Tauri commands for the quiz GUI flow — `spawn_quiz_plan`, `spawn_quiz_generate`, `cancel_quiz` (lifecycle, mirroring the goal/chat background-thread + `quiz-stream` + terminal-channel pattern), `list_quiz_attempts` and `read_quiz_attempt` (history), plus `read_quiz_events` (history-log timeline). The `app-shell` IPC Command Registry total count SHALL account for these six (foundation 9 + workspace 6 + chat 2 + quiz 6 = 23); no other Tauri command SHALL be registered by this change.

`list_quiz_attempts(vault_path)` SHALL scan `<vault>/.codebus/quiz/<slug>/*.md`, parse each attempt's frontmatter, and return a newest-first list of attempt metadata (`slug`, `quiz_id`, `trigger`, `topic`/`target_page`, `events_log`, `path`); a missing quiz directory SHALL yield an empty list (not an error). `read_quiz_attempt(vault_path, path)` SHALL return the attempt markdown, rejecting any `path` that does not resolve under the vault's `.codebus/quiz/` tree with `AppError::Invalid { field: "path" }`.

`read_quiz_events(vault_path, path)` SHALL read the events.jsonl file at `path` and return its contents parsed as an ordered list of `EventEnvelope` (one per line, malformed lines skipped rather than failing the whole read), so the history view-generation-log affordance can replay the attempt's generate spawn through the existing agent stream rendering. It SHALL reject any `path` that does not resolve under the vault's `.codebus/` tree with `AppError::Invalid { field: "path" }` (mirroring the `read_quiz_attempt` containment guard). A missing file SHALL yield `AppError::Invalid { field: "path" }` rather than a panic.

`spawn_quiz_plan(vault_path, topic)` SHALL run `run_quiz_plan` on a background thread, emit each `VerbEvent` as a `QuizStreamPayload` on the `quiz-stream` channel, and on completion emit exactly one `QuizPlanTerminalPayload` on the `quiz-plan-terminal` channel whose `result` is `Scope { pages }`, `NoMatch { reason }`, `Failed { message }`, or `Cancelled`. It SHALL return a `quiz-plan-<slug>` run id synchronously. It SHALL NOT start a generate spawn — the frontend interposes the confirm gate and separately calls `spawn_quiz_generate`.

`spawn_quiz_generate(vault_path, pages, question_count)` SHALL run `run_quiz_generate` on a background thread, emit `VerbEvent`s on `quiz-stream`, and on completion emit exactly one `QuizGenerateTerminalPayload` on `quiz-generate-terminal` whose `result` on success carries the fence-stripped `quiz_md`, `planned_pages`, and `events_log` (for the answering view and history persistence), or `Failed { message }` / `Cancelled`. It SHALL return a `quiz-generate-<slug>` run id synchronously.

`cancel_quiz(run_id)` SHALL flip the cancel flag for `run_id` when present and SHALL be idempotent (no-op + `Ok(())` for an unknown or already-finished run id).

#### Scenario: Plan spawn does not start generation

- **WHEN** the frontend calls `spawn_quiz_plan` and the plan returns a scope
- **THEN** a `QuizPlanTerminalPayload` with `result: Scope { pages }` SHALL be emitted AND no generate spawn SHALL have run (the frontend must call `spawn_quiz_generate` separately after the user confirms)

#### Scenario: No-match plan emits no-match terminal and no generate

- **WHEN** the frontend calls `spawn_quiz_plan` and no wiki page covers the topic
- **THEN** a `QuizPlanTerminalPayload` with `result: NoMatch { reason }` SHALL be emitted AND no generate spawn SHALL run AND no quiz file SHALL be persisted

#### Scenario: Generate terminal carries quiz body for the answering view

- **WHEN** the frontend calls `spawn_quiz_generate` with a confirmed page list and it succeeds
- **THEN** a `QuizGenerateTerminalPayload` with `result: Succeeded { quiz_md, planned_pages, events_log }` SHALL be emitted on `quiz-generate-terminal`

#### Scenario: Read quiz events returns the parsed timeline

- **WHEN** the frontend calls `read_quiz_events` with an attempt's `events_log` path that resolves under the vault `.codebus/` tree
- **THEN** the command SHALL return an ordered `EventEnvelope` list parsed from the file's lines so the timeline can be rendered through the existing agent stream rendering

#### Scenario: Read quiz events rejects an out-of-tree path

- **WHEN** the frontend calls `read_quiz_events` with a `path` that does not resolve under the vault `.codebus/` tree
- **THEN** the command SHALL reject with `AppError` having `kind: "invalid"` and `field: "path"`

#### Scenario: cancel_quiz idempotent on unknown run

- **WHEN** the frontend calls `cancel_quiz` with a run id that is not active
- **THEN** the command SHALL return `Ok(())` without error

### Requirement: Quiz Tab Plan-Confirm-Generate Flow

The Quiz tab SHALL offer two entry points that converge after generation. `+ New quiz` SHALL open a free-text topic input; submitting it SHALL run a plan spawn whose agent activity is rendered live via the existing agent stream rendering. When the plan spawn yields a planned scope, the system SHALL display the planned `wiki/` page list with a `[改]` (revise) and `[確認]` (confirm) control and SHALL NOT start the generate spawn until the user confirms. On confirm, the system SHALL run the generate spawn with live activity rendering, then transition to the answering view. When the plan spawn yields no match, the system SHALL display the no-match reason and SHALL NOT start a generate spawn. While the plan or generate spawn is running, the system SHALL subscribe to the `quiz-stream` channel and render the streamed `VerbEvent`s through the existing agent stream rendering (the same `foldTimeline` + thought / activity-item pipeline used by the run detail view); a static "planning…" / "generating…" label alone SHALL NOT satisfy this — the live agent activity SHALL be visible as it happens, mirroring the goal flow.

The `[Quiz me on this]` control SHALL appear at the bottom of a wiki content page preview (it SHALL NOT appear for `index.md` or `log.md`). Activating it SHALL skip the plan spawn and run the generate spawn directly using that page plus its one-hop wikilinked pages.

#### Scenario: New quiz shows planned scope before generating

- **WHEN** the user submits a topic via `+ New quiz` and the plan spawn emits a scope
- **THEN** the planned `wiki/` page list SHALL be shown with confirm/revise controls AND the generate spawn SHALL NOT start until the user confirms

#### Scenario: No-match topic shows reason and stops

- **WHEN** the plan spawn emits `[CODEBUS_QUIZ_NO_MATCH]`
- **THEN** the UI SHALL display the no-match reason AND SHALL NOT start a generate spawn AND SHALL NOT persist any quiz file

#### Scenario: Quiz-me-on-this skips planning

- **WHEN** the user activates `[Quiz me on this]` on a wiki content page preview
- **THEN** no plan spawn SHALL run AND the generate spawn SHALL run using that page plus its one-hop wikilinks

#### Scenario: Quiz-me-on-this hidden on nav pages

- **WHEN** the wiki preview shows `index.md` or `log.md`
- **THEN** the `[Quiz me on this]` control SHALL NOT be rendered

#### Scenario: Plan/generate agent activity is rendered live

- **GIVEN** the user submitted a topic via `+ New quiz` (or activated `[Quiz me on this]`)
- **WHEN** the plan or generate spawn emits `VerbEvent`s on the `quiz-stream` channel before its terminal payload
- **THEN** the Quiz tab SHALL render those events through the existing agent stream rendering (thought / tool-use / result items appear as they stream) AND SHALL NOT show only a static placeholder label

#### Scenario: + New quiz is not shown while inside a quiz

- **GIVEN** the Quiz tab is in a quiz flow or attempt view (planning, confirm, generating, answering, no-match, error, or an opened attempt)
- **THEN** the `+ New quiz` control SHALL NOT be rendered
- **AND** the `+ New quiz` control SHALL be rendered only in the history list and the topic-input compose screen
