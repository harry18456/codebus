## MODIFIED Requirements

### Requirement: Quiz Answering and Summary

The answering view SHALL present one question per screen with four choices. After the user selects a choice and submits, the system SHALL reveal whether it was correct by comparing the selection to the quiz markdown `Answer` field client-side (no agent spawn) and SHALL show the `Explanation`. After the final question, a summary SHALL display the score and a pass/fail outcome computed client-side using `app.quiz.pass_threshold`. The threshold value SHALL be sourced from the application settings store (the same `app.quiz.pass_threshold` key the Settings modal binds); it SHALL NOT be a hardcoded component constant. When the `app.quiz.pass_threshold` key is absent the value SHALL default to 80; changing the setting SHALL change the summary pass/fail boundary on the next finished quiz.

The revealed `Explanation` SHALL render each of its `[[slug]]` wikilink citations as an interactive wikilink, on BOTH correct and incorrect submissions (and likewise wherever the per-question explanation is shown in the Review view). A citation whose slug resolves to an existing wiki page SHALL be activatable; activating it SHALL navigate the workspace to that wiki page (the same navigation as selecting the page from the wiki tree). A citation whose slug does not resolve SHALL render in the standard unresolved-wikilink presentation and SHALL NOT be activatable. The system SHALL NOT render a separate `[← Back to wiki page]` affordance; the explanation's per-question citations are the source-navigation mechanism.

The answering view SHALL persist progress to the attempt's progress sidecar (see capability `quiz`) on every submission AND on every Next via the `write_quiz_progress` command: each submission appends/updates the answered question with the user's `selected` choice and `correct` boolean, sets `status: in_progress`, and sets `cursor` to `{ q: <that question>, revealed: true }`; submitting the final question SHALL set `status: completed` and `completed_at`; pressing Next SHALL set `cursor` to `{ q: <next question>, revealed: false }` (answers unchanged, `status: in_progress`). When an attempt is opened that already has an in-progress sidecar with a `cursor`, the answering view SHALL restore exactly that position: question `cursor.q`, shown in its submitted state (stored `selected` + verdict + `Explanation`) when `cursor.revealed` is true, or as a blank unanswered question when false. When the sidecar has no `cursor` (legacy), the answering view SHALL instead restore the last answered question (highest 1-based number in `answers`) in its submitted state. It SHALL NOT restart at question 1 for an in-progress attempt. Persistence SHALL NOT spawn an agent.

#### Scenario: Correct answer revealed without spawn

- **WHEN** the user submits the choice matching the question's `Answer` field
- **THEN** the system SHALL mark it correct AND show the `Explanation` AND SHALL NOT spawn an agent to grade

#### Scenario: Explanation citations render as navigable wikilinks on both outcomes

- **GIVEN** a question whose `Explanation` cites `[[auth-middleware-verification]]` and that slug resolves to an existing wiki page
- **WHEN** the user submits an answer (whether correct or incorrect) and the `Explanation` is revealed
- **THEN** the citation SHALL render as an activatable wikilink AND activating it SHALL navigate the workspace to the `auth-middleware-verification` wiki page AND no `[← Back to wiki page]` affordance SHALL be rendered

#### Scenario: Unresolvable citation is not activatable

- **GIVEN** a question whose `Explanation` cites `[[no-such-page]]` and that slug resolves to no wiki page
- **WHEN** the `Explanation` is revealed
- **THEN** the citation SHALL render in the standard unresolved-wikilink presentation AND SHALL NOT navigate anywhere when activated

#### Scenario: Summary applies pass threshold

- **GIVEN** `app.quiz.pass_threshold` is 80
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a passing outcome

#### Scenario: Changing the threshold setting changes the outcome

- **GIVEN** `app.quiz.pass_threshold` is set to 90 in the settings store
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a failing outcome

#### Scenario: Each submission persists progress

- **WHEN** the user submits an answer to a question
- **THEN** the system SHALL call `write_quiz_progress` recording that question's `selected` and `correct` with `status: in_progress` (or `completed` on the final question) AND SHALL NOT spawn an agent

#### Scenario: Resume restores the exact cursor position (advanced past the answered question)

- **GIVEN** an attempt whose sidecar has answers for questions 1–3 of 5, `status: in_progress`, and `cursor: { q: 4, revealed: false }` (the user submitted Q3 then pressed Next, then left)
- **WHEN** the user opens that attempt
- **THEN** the answering view SHALL show question 4 as a blank unanswered question AND SHALL NOT show question 3's submitted state

#### Scenario: Resume restores the exact cursor position (not yet advanced)

- **GIVEN** an attempt whose sidecar has answers for questions 1–3 of 5, `status: in_progress`, and `cursor: { q: 3, revealed: true }` (the user submitted Q3 and left without pressing Next)
- **WHEN** the user opens that attempt
- **THEN** the answering view SHALL restore question 3 in its submitted state — the stored `selected` choice for question 3, its verdict, and its `Explanation`

#### Scenario: Legacy sidecar without a cursor falls back to last answered

- **GIVEN** an attempt whose sidecar has answers for questions 1 and 2 of 5, `status: in_progress`, and NO `cursor` field
- **WHEN** the user opens that attempt
- **THEN** the answering view SHALL restore question 2 (the last answered) in its submitted state AND SHALL NOT restart at question 1

### Requirement: Quiz History List

The Quiz tab SHALL list prior attempts grouped by page or topic slug, derived by scanning `<vault>/.codebus/quiz/`. For each attempt row the system SHALL derive a status from that attempt's progress sidecar (via `read_quiz_progress`) and SHALL show a status badge: not-started shows `0/N`, in-progress shows `X/N`, completed shows `X/N` plus the score percentage and a pass/fail outcome computed with `app.quiz.pass_threshold` (where N is the attempt's question count, X is the answered count, both derived — see capability `quiz`).

Selecting an attempt row SHALL route by derived status: a not-started or in-progress attempt SHALL open the answering view (resuming per the Quiz Answering and Summary requirement); a completed attempt SHALL open a read-only Review view. The Review view SHALL render each question with the user's chosen answer, the correct answer, and the explanation; it SHALL NOT render the attempt as raw markdown. The Review view SHALL expose a `[重做此份]` (redo-this) affordance and the view-generation-log affordance.

`[重做此份]` SHALL reset that attempt's progress sidecar to not-started and re-enter the answering view at question 1 with the same generated questions; it SHALL NOT spawn an agent (it is distinct from `+ New quiz`, which produces a fresh generated attempt). The view-generation-log affordance SHALL be present only when the attempt has a non-null `events_log`; activating it SHALL open a centered modal dialog (with a backdrop, dismissible, consistent with the app's existing modal pattern) that renders that attempt's generate-spawn events through the existing agent stream rendering pipeline; displaying only the `events_log` file path SHALL NOT satisfy this requirement; dismissing the modal SHALL return to the Review view. The history row itself SHALL NOT inline-expand a generation-log panel and SHALL NOT render the attempt as raw markdown.

#### Scenario: History reflects non-destructive retries

- **WHEN** two quizzes have been generated for the same topic slug
- **THEN** the history SHALL list two distinct attempt rows under that slug AND opening either SHALL show that attempt's own questions and answers

#### Scenario: Row badge reflects derived status

- **GIVEN** three attempts under one slug: one with no sidecar, one with 2 of 5 answered (`in_progress`), one with 5 of 5 answered (`completed`, 4 correct) and `app.quiz.pass_threshold` 80
- **THEN** their badges SHALL be `0/5` (not-started), `2/5` (in-progress), and `5/5 · 80% · pass` (completed) respectively

#### Scenario: Completed attempt opens Review, not raw markdown

- **WHEN** the user selects a completed attempt row
- **THEN** the system SHALL open the Review view rendering per-question user-choice vs correct answer plus explanation AND SHALL NOT render the attempt's raw markdown in a preformatted block

#### Scenario: Not-started or in-progress attempt opens answering

- **WHEN** the user selects a not-started or in-progress attempt row
- **THEN** the system SHALL open the answering view (starting at question 1 for not-started, or resuming for in-progress)

#### Scenario: Redo this resets without spawning

- **WHEN** the user activates `[重做此份]` in the Review view
- **THEN** that attempt's sidecar SHALL be reset to not-started AND the answering view SHALL re-enter at question 1 with the same questions AND no `spawn_quiz_plan` or `spawn_quiz_generate` SHALL be invoked

#### Scenario: View-generation-log opens a modal timeline from Review

- **WHEN** the user activates the view-generation-log affordance in the Review view of an attempt with a non-null `events_log`
- **THEN** the system SHALL open a centered modal dialog rendering that attempt's generate-spawn events through the existing agent stream rendering AND dismissing it SHALL return to the Review view

#### Scenario: No view-generation-log affordance without an events log

- **WHEN** the user opens a completed attempt whose `events_log` is null
- **THEN** the Review view SHALL NOT render a view-generation-log affordance

### Requirement: Tauri IPC Commands for Quiz Plan and Generate Lifecycle

The system SHALL register exactly eight Tauri commands for the quiz GUI flow — `spawn_quiz_plan`, `spawn_quiz_generate`, `cancel_quiz` (lifecycle, mirroring the goal/chat background-thread + `quiz-stream` + terminal-channel pattern), `list_quiz_attempts` and `read_quiz_attempt` (history), `read_quiz_events` (history-log timeline), plus `read_quiz_progress` and `write_quiz_progress` (per-attempt answering progress). The `app-shell` IPC Command Registry total count SHALL account for these eight (foundation 9 + workspace 6 + chat 2 + quiz 8 = 25); no other Tauri command SHALL be registered by this change.

`list_quiz_attempts(vault_path)` SHALL scan `<vault>/.codebus/quiz/<slug>/*.md`, parse each attempt's frontmatter, and return a newest-first list of attempt metadata (`slug`, `quiz_id`, `trigger`, `topic`/`target_page`, `events_log`, `path`); a missing quiz directory SHALL yield an empty list (not an error). `read_quiz_attempt(vault_path, path)` SHALL return the attempt markdown, rejecting any `path` that does not resolve under the vault's `.codebus/quiz/` tree with `AppError::Invalid { field: "path" }`.

`read_quiz_events(vault_path, path)` SHALL read the events.jsonl file at `path` and return its contents parsed as an ordered list of `EventEnvelope` (one per line, malformed lines skipped rather than failing the whole read), so the history view-generation-log affordance can replay the attempt's generate spawn through the existing agent stream rendering. It SHALL reject any `path` that does not resolve under the vault's `.codebus/` tree with `AppError::Invalid { field: "path" }` (mirroring the `read_quiz_attempt` containment guard). A missing file SHALL yield `AppError::Invalid { field: "path" }` rather than a panic.

`read_quiz_progress(vault_path, path)` SHALL return the progress sidecar state for the attempt whose progress file is `path` (the not-started state when the file is absent; the tolerantly-parsed state otherwise — see capability `quiz`). `write_quiz_progress(vault_path, path, progress)` SHALL atomically persist the given progress to `path`. Both SHALL reject any `path` that does not resolve under the vault's `.codebus/` tree with `AppError::Invalid { field: "path" }` (same containment-guard strength as `read_quiz_attempt`); neither SHALL read or write outside that tree.

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

#### Scenario: Read quiz progress returns not-started when sidecar absent

- **WHEN** the frontend calls `read_quiz_progress` for an attempt that has no `.progress.json`
- **THEN** the command SHALL return the not-started state (not an error)

#### Scenario: Write then read quiz progress round-trips

- **WHEN** the frontend calls `write_quiz_progress` with a progress payload then `read_quiz_progress` for the same path
- **THEN** the read SHALL return the persisted answers, status, and timestamps

#### Scenario: Quiz progress commands reject out-of-tree paths

- **WHEN** the frontend calls `read_quiz_progress` or `write_quiz_progress` with a `path` that does not resolve under the vault `.codebus/` tree
- **THEN** the command SHALL reject with `AppError` having `kind: "invalid"` and `field: "path"`

#### Scenario: cancel_quiz idempotent on unknown run

- **WHEN** the frontend calls `cancel_quiz` with a run id that is not active
- **THEN** the command SHALL return `Ok(())` without error

### Requirement: Quiz Tab Plan-Confirm-Generate Flow

The Quiz tab SHALL offer two entry points that converge after generation. `+ New quiz` SHALL open a free-text topic input; submitting it SHALL run a plan spawn whose agent activity is rendered live via the existing agent stream rendering. When the plan spawn yields a planned scope, the system SHALL display the planned `wiki/` page list together with a description stating that the quiz will be generated from those listed pages, alongside a revise control labeled `重新規劃` and a confirm control labeled `確認`, and SHALL NOT start the generate spawn until the user confirms. The confirm-view description text and the `重新規劃` / `確認` control labels SHALL be sourced from the application i18n system (the `useT()` / `messages.ts` mechanism, with `en` and `zh-tw` entries), and SHALL NOT be hardcoded literal strings in the component. Activating `重新規劃` SHALL return to the topic-input view to re-plan (it SHALL NOT regenerate with the same scope and SHALL NOT spawn an agent). On confirm, the system SHALL run the generate spawn with live activity rendering, then transition to the answering view. When the plan spawn yields no match, the system SHALL display the no-match reason and SHALL NOT start a generate spawn. While the plan or generate spawn is running, the system SHALL subscribe to the `quiz-stream` channel and render the streamed `VerbEvent`s through the existing agent stream rendering (the same `foldTimeline` + thought / activity-item pipeline used by the run detail view); a static "planning…" / "generating…" label alone SHALL NOT satisfy this — the live agent activity SHALL be visible as it happens, mirroring the goal flow.

The `[Quiz me on this]` control SHALL appear at the bottom of a wiki content page preview (it SHALL NOT appear for `index.md` or `log.md`). Activating it SHALL skip the plan spawn and run the generate spawn directly using that page plus its one-hop wikilinked pages.

#### Scenario: New quiz shows planned scope before generating

- **WHEN** the user submits a topic via `+ New quiz` and the plan spawn emits a scope
- **THEN** the planned `wiki/` page list SHALL be shown with a description stating the quiz will be generated from those pages AND with confirm/revise controls AND the generate spawn SHALL NOT start until the user confirms

#### Scenario: Revise control is labeled and returns to topic input

- **GIVEN** the planned scope is displayed with the revise control
- **WHEN** the user activates the revise control
- **THEN** the control SHALL read `重新規劃` (sourced from i18n, not a hardcoded string) AND the system SHALL return to the free-text topic-input view AND SHALL NOT spawn an agent

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
