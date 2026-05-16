## MODIFIED Requirements

### Requirement: Workspace Layout and Tab Navigation

The system SHALL replace the Workspace stub with a real Workspace shell composed of a left sidebar with exactly three tabs and a main content area. The three tabs SHALL be `Goals` (default selection), `Wiki`, and `Quiz`. The sidebar SHALL also render the active vault's display name + path block and a `← Back to Lobby` control. Selecting a tab SHALL switch the main content area to the corresponding view; unselected tabs SHALL not retain their internal scroll position across switches (each switch SHALL re-mount the inactive view).

The `Quiz` tab SHALL render the quiz history list and a `+ New quiz` control as defined in the Quiz Tab Flow requirements of this capability. The `Quiz` tab SHALL NOT render the v1 "Coming soon" placeholder.

#### Scenario: Workspace mounts with Goals tab selected

- **WHEN** the user opens a vault from the Lobby
- **THEN** the main view transitions to the Workspace AND the `Goals` tab is the active selection AND the `Goals` main content area is rendered

#### Scenario: Tab switch re-renders main content

- **WHEN** the user clicks the `Wiki` tab while `Goals` is active
- **THEN** the main content area renders the Wiki tab content AND the `Goals` main content area is unmounted

#### Scenario: Quiz tab shows history and new-quiz control

- **WHEN** the user clicks the `Quiz` tab
- **THEN** the main content area renders the quiz history list AND a `+ New quiz` control AND SHALL NOT render the literal text "Coming soon — quiz flow ships in v3-app-quiz"

#### Scenario: Back to Lobby control returns to Lobby

- **WHEN** the user clicks the `← Back to Lobby` control while in the Workspace
- **THEN** the main view returns to the Lobby state AND the Workspace component is unmounted AND any active goal run continues running in the background

## ADDED Requirements

### Requirement: Quiz Tab Plan-Confirm-Generate Flow

The Quiz tab SHALL offer two entry points that converge after generation. `+ New quiz` SHALL open a free-text topic input; submitting it SHALL run a plan spawn whose agent activity is rendered live via the existing agent stream rendering. When the plan spawn yields a planned scope, the system SHALL display the planned `wiki/` page list with a `[改]` (revise) and `[確認]` (confirm) control and SHALL NOT start the generate spawn until the user confirms. On confirm, the system SHALL run the generate spawn with live activity rendering, then transition to the answering view. When the plan spawn yields no match, the system SHALL display the no-match reason and SHALL NOT start a generate spawn.

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

### Requirement: Quiz Answering and Summary

The answering view SHALL present one question per screen with four choices. After the user selects a choice and submits, the system SHALL reveal whether it was correct by comparing the selection to the quiz markdown `Answer` field client-side (no agent spawn) and SHALL show the `Explanation`. For an incorrect answer the system SHALL additionally render a `[← Back to wiki page]` affordance. After the final question, a summary SHALL display the score and a pass/fail outcome computed client-side using `app.quiz.pass_threshold`.

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

### Requirement: Quiz History List

The Quiz tab SHALL list prior attempts grouped by page or topic slug, derived by scanning `<vault>/.codebus/quiz/`. Each attempt row SHALL expose a view-generation-log affordance that opens the events.jsonl timeline for that attempt's generate spawn using the existing agent stream rendering. Selecting an attempt row SHALL open that attempt's persisted markdown.

#### Scenario: History reflects non-destructive retries

- **WHEN** two quizzes have been generated for the same topic slug
- **THEN** the history SHALL list two distinct attempt rows under that slug AND opening either SHALL show that attempt's own questions and answers

#### Scenario: View-generation-log opens the events timeline

- **WHEN** the user activates the view-generation-log affordance on an attempt row
- **THEN** the system SHALL render that attempt's generate-spawn events.jsonl through the existing agent stream rendering

### Requirement: Tauri IPC Commands for Quiz Plan and Generate Lifecycle

The system SHALL register exactly five Tauri commands for the quiz GUI flow — `spawn_quiz_plan`, `spawn_quiz_generate`, `cancel_quiz` (lifecycle, mirroring the goal/chat background-thread + `quiz-stream` + terminal-channel pattern), plus `list_quiz_attempts` and `read_quiz_attempt` (history). The `app-shell` IPC Command Registry total count SHALL account for these five (foundation 9 + workspace 6 + chat 2 + quiz 5 = 22); no other Tauri command SHALL be registered by this change.

`list_quiz_attempts(vault_path)` SHALL scan `<vault>/.codebus/quiz/<slug>/*.md`, parse each attempt's frontmatter, and return a newest-first list of attempt metadata (`slug`, `quiz_id`, `trigger`, `topic`/`target_page`, `events_log`, `path`); a missing quiz directory SHALL yield an empty list (not an error). `read_quiz_attempt(vault_path, path)` SHALL return the attempt markdown, rejecting any `path` that does not resolve under the vault's `.codebus/quiz/` tree with `AppError::Invalid { field: "path" }`.

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

#### Scenario: cancel_quiz idempotent on unknown run

- **WHEN** the frontend calls `cancel_quiz` with a run id that is not active
- **THEN** the command SHALL return `Ok(())` without error
