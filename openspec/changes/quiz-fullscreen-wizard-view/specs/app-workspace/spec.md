## ADDED Requirements

### Requirement: Quiz Tab Wizard Content Header And Layout

The Quiz tab SHALL, when a new quiz is being created or an attempt is being reviewed, render a wizard view inside the Quiz tab's content area. The Workspace shell (the sidebar with vault display name, back-to-lobby, and the Goals / Wiki / Quiz nav rows) SHALL remain visible and unmodified while the wizard is active; the wizard SHALL NOT hide or replace the Workspace sidebar, and the Workspace component SHALL NOT participate in wizard-active gating. Wizard activation SHALL be triggered by the user starting a new quiz from the quiz history list (the `+ New quiz` control) and SHALL be deactivated when the wizard exits to the quiz history list (cancel, completion exit, or back-to-history navigation).

During wizard activation the existing `TabContentHeader` row at the top of the Quiz tab content area SHALL be re-purposed as the wizard chrome:

- The `+ New quiz` CTA SHALL NOT be rendered.
- The header title SHALL change from the quiz history title to the wizard's per-step title (for example "New quiz" during topic / scope_confirm / generating, "Quiz: <topic>" during reviewing, or "Quiz <topic> · result" during completion); the exact i18n key MAY vary per step, but the title SHALL be sourced from the application i18n system, not a hardcoded literal.
- A step indicator (dots + step label of the form `Step <n> / <total> · <step name>`) SHALL be rendered next to the title for the topic, scope_confirm, generating, and review_pending steps; the dots SHALL reflect done / current / pending state per step (the visual contract follows AUDIT QC2: 7px dot, done filled-fg-tertiary, current filled-amber with outer accent-tint ring, pending ring-only border-strong).
- During the reviewing sub-state the step indicator SHALL be replaced by a `Q<n> / <total>` counter on the right side of the header; during the completion sub-state the header SHALL render only a "← back to history" link plus the result title, without step dots.

The wizard SHALL host a state machine with four primary steps and the fourth step SHALL have three sub-states:

- `topic`: free-text topic input, with example pills sourced from the vault's existing wiki page titles (when available) or from a fixed fallback set; an empty topic submission SHALL be rejected with an inline amber border + tooltip on the input rather than disabling the next button.
- `scope_confirm`: render the planned scope grouped by the bucket taxonomy defined in capability `quiz` § Quiz Scope Plan Bucket Taxonomy.
- `generating`: render a brand banner and a live stream tail using the shared stream rendering helpers (see § Stream Event Summary Helper Module and § Activity Stream Two-Phase Cluster Rendering).
- `review_pending`: the quiz is generated but not started; show a Start control.
- `reviewing`: hosts the existing Quiz Answering and Summary view inside the wizard content area (its internal behavior SHALL NOT change).
- `completion`: show the fail or pass summary plus a redo control and a result-detail control ("look at wrong questions" on fail, "look at process" on pass).

The wizard SHALL NOT introduce large transition animations. When the user has `prefers-reduced-motion` set, step transitions SHALL be instant. The wizard SHALL never display a static placeholder label without the live stream tail during the `generating` step; this requirement is the wizard-host extension of the existing live stream rendering rule.

#### Scenario: Wizard launches from + New quiz

- **GIVEN** the Quiz tab is on the quiz history list with the existing quiz-history `TabContentHeader` (title "Quiz history" + `+ New quiz` CTA) rendered
- **WHEN** the user activates `+ New quiz`
- **THEN** the wizard SHALL enter the `topic` step AND the same `TabContentHeader` row SHALL be re-rendered with the wizard title ("New quiz") and a step indicator showing Step 1 of 4 as current AND the `+ New quiz` CTA SHALL no longer be rendered AND the Workspace sidebar SHALL remain visible and unchanged

#### Scenario: Wizard exits restore the quiz-history content header

- **GIVEN** the wizard is active in any step
- **WHEN** the wizard exits (cancel, completion exit, or back-to-history navigation)
- **THEN** the `TabContentHeader` SHALL be re-rendered with the quiz-history title and the `+ New quiz` CTA AND the Quiz tab SHALL render the quiz history list AND no Workspace shell change SHALL be requested

#### Scenario: Workspace shell does not participate in wizard gating

- **GIVEN** the Quiz tab is rendering a wizard view in any step
- **WHEN** the Workspace component decides whether to render its sidebar and route between tabs
- **THEN** the Workspace component SHALL NOT inspect any wizard-related state AND SHALL NOT branch on `activeTab === "quiz"` to alter chrome AND SHALL render exactly the same layout it renders when the quiz tab is in its history view

#### Scenario: Step indicator reflects current step

- **GIVEN** the wizard is in the `scope_confirm` step
- **WHEN** the `TabContentHeader` is rendered
- **THEN** the step indicator SHALL show four dots, the first dot as done, the second dot as current, and the third and fourth dots as pending AND the step label SHALL read "Step 2 / 4 · Scope" (or the locale-appropriate i18n equivalent)

#### Scenario: Reviewing sub-state replaces step indicator with question counter

- **GIVEN** the wizard is in the `reviewing` sub-state on question 2 of a 5-question quiz
- **WHEN** the `TabContentHeader` is rendered
- **THEN** the header title SHALL read "Quiz: <topic>" AND the step indicator dots SHALL NOT be rendered AND a counter reading `Q2 / 5` SHALL appear on the right side of the header

#### Scenario: Completion sub-state header is a back link plus result title

- **GIVEN** the wizard is in the `completion` sub-state
- **WHEN** the `TabContentHeader` is rendered
- **THEN** the header SHALL contain a "← back to history" link AND a result title of the form "Quiz <topic> · result" AND no step indicator dots AND no `+ New quiz` CTA

#### Scenario: Generating step renders live stream tail

- **GIVEN** the wizard is in the `generating` step
- **WHEN** the generate spawn emits `VerbEvent`s on the `quiz-stream` channel before its terminal payload
- **THEN** the wizard SHALL render those events through the existing agent stream rendering helpers AND SHALL NOT show only a static placeholder label

#### Scenario: Reviewing step hosts existing answering view

- **GIVEN** the wizard is in the `reviewing` sub-state
- **WHEN** the answering view renders a question
- **THEN** the existing per-question behavior (letter chip, four-choice radio, citation blockquote, wikilink resolution) SHALL be preserved unchanged AND the answering view SHALL appear inside the wizard content area below the `TabContentHeader` row

#### Scenario: Empty topic submission shows inline validation

- **GIVEN** the wizard is in the `topic` step with an empty input
- **WHEN** the user activates the next-step control
- **THEN** the wizard SHALL NOT transition to `scope_confirm` AND the input SHALL display an amber border AND a tooltip SHALL communicate that the topic cannot be empty AND the next-step control SHALL remain enabled

#### Scenario: Reduced-motion users get instant step transitions

- **GIVEN** the user has `prefers-reduced-motion` set
- **WHEN** the wizard transitions between steps
- **THEN** the transitions SHALL be instant AND no animated motion SHALL be applied

---

### Requirement: Quiz Wizard URL State Persistence

The Quiz wizard SHALL persist its current step into the application URL search-string so that reloading the application restores the wizard at the same step. The wizard SHALL own exactly two URL query parameters: `quiz_step` (holding the step identifier such as `topic`, `scope_confirm`, `generating`, `review_pending`, `reviewing`, `completion`) and `staged_id` (holding the wizard's per-attempt identifier). The wizard SHALL update these parameters via `window.history.pushState` only on user-initiated step transitions; mount and unmount SHALL NOT push history entries. The wizard SHALL NOT remove or modify any other URL query parameter. On mount, the wizard SHALL read these two parameters and SHALL restore the step accordingly; when `staged_id` references an identifier that is not present in the wizard store (for example after an application restart), the wizard SHALL silently fall back to the `topic` step and SHALL log a debug-level warning that records the missing identifier.

#### Scenario: Step transition pushes URL state

- **GIVEN** the wizard is in the `topic` step
- **WHEN** the user submits a topic and the wizard transitions to `scope_confirm` with a freshly generated staged identifier
- **THEN** the URL search-string SHALL contain `quiz_step=scope_confirm` AND `staged_id=<the-identifier>` AND any previously present URL query parameters owned by other systems SHALL remain unchanged

#### Scenario: Reload restores the wizard step

- **GIVEN** the URL search-string contains `quiz_step=generating` and `staged_id=abc123` and the wizard store has an in-memory staged record for `abc123`
- **WHEN** the application reloads
- **THEN** the wizard SHALL mount in the `generating` step AND no new agent spawn SHALL be triggered by the restore itself

#### Scenario: Missing staged identifier falls back to topic

- **GIVEN** the URL search-string contains `quiz_step=reviewing` and `staged_id=xyz789` and the wizard store has no in-memory staged record for `xyz789` (for example because the application was restarted)
- **WHEN** the application reloads
- **THEN** the wizard SHALL mount in the `topic` step AND a debug-level warning SHALL be logged that records the missing `staged_id` value AND no error dialog SHALL be presented to the user

#### Scenario: Wizard owns only its two URL parameters

- **GIVEN** the URL search-string contains a parameter `vault=foo` set by another system
- **WHEN** the wizard transitions between any of its steps
- **THEN** the `vault=foo` parameter SHALL remain in the URL search-string unchanged

---

### Requirement: Quiz Wizard Cancel Cleanup

The Quiz wizard SHALL support cancelling from the `scope_confirm`, `generating`, and `review_pending` steps; cancelling SHALL return the user to the quiz history list without leaving wizard staged state behind. On cancel, the wizard SHALL reset its in-memory staged state, SHALL clear the `quiz_step` and `staged_id` URL parameters, SHALL restore the quiz history `TabContentHeader` (title and `+ New quiz` CTA), and SHALL invoke the existing `cancelQuiz(runId)` IPC for any in-flight plan or generate spawn associated with the staged identifier. If `cancelQuiz` rejects, the wizard SHALL still complete its frontend cleanup and SHALL log the backend failure at error level; the user-visible cancel result SHALL NOT be blocked by a backend rejection. Wizard staged state SHALL NOT be persisted to disk or to the application database; the cancelled state SHALL leave no on-disk artifact that requires a later garbage-collection pass.

#### Scenario: Cancel from scope_confirm returns to history

- **GIVEN** the wizard is in the `scope_confirm` step with a staged identifier
- **WHEN** the user activates the cancel control
- **THEN** the wizard SHALL return to the quiz history list AND the `quiz_step` and `staged_id` URL parameters SHALL be cleared AND the wizard in-memory staged state SHALL be reset AND the `TabContentHeader` SHALL be restored to its quiz-history title and `+ New quiz` CTA

#### Scenario: Cancel from generating invokes cancelQuiz

- **GIVEN** the wizard is in the `generating` step with an in-flight generate spawn for staged identifier `abc123`
- **WHEN** the user activates the cancel control
- **THEN** the wizard SHALL invoke `cancelQuiz` with the run identifier corresponding to the in-flight spawn AND the wizard SHALL return to the quiz history list whether or not `cancelQuiz` resolves successfully

#### Scenario: Backend cancel rejection does not block frontend cleanup

- **GIVEN** the wizard is in the `generating` step and `cancelQuiz` rejects with an error
- **WHEN** the user activates the cancel control
- **THEN** the wizard frontend cleanup SHALL complete (in-memory state reset, URL parameters cleared, `TabContentHeader` restored, quiz history list shown) AND an error-level log entry SHALL record the backend cancel failure

#### Scenario: Cancel leaves no on-disk wizard artifact

- **GIVEN** the wizard has just been cancelled from any step
- **WHEN** the application enumerates on-disk quiz attempts in the vault
- **THEN** no staged-only wizard artifact (markdown attempt or progress sidecar) corresponding to the cancelled `staged_id` SHALL be present (unless a generate spawn had already produced a completed attempt before cancel, in which case the completed attempt SHALL be retained because it is no longer staged state)

---

## MODIFIED Requirements

### Requirement: Quiz Tab Plan-Confirm-Generate Flow

The Quiz tab SHALL offer two entry points that converge after generation, and the `+ New quiz` entry SHALL host its plan-confirm-generate flow inside the wizard view defined by § Quiz Tab Wizard Content Header And Layout. `+ New quiz` SHALL open the wizard in its `topic` step (a free-text topic input); submitting it SHALL run a plan spawn whose agent activity is rendered live via the existing agent stream rendering, and the wizard SHALL transition to the `scope_confirm` step when the plan spawn yields a planned scope. The scope-confirm step SHALL display the planned scope grouped by the bucket taxonomy defined in capability `quiz` § Quiz Scope Plan Bucket Taxonomy alongside a revise control labeled `重新規劃` and a confirm control labeled `確認`, and SHALL NOT start the generate spawn until the user confirms. The confirm-view control labels SHALL be sourced from the application i18n system (the `useT()` / `messages.ts` mechanism, with `en` and `zh-tw` entries), and SHALL NOT be hardcoded literal strings in the component. Activating `重新規劃` SHALL return the wizard to the `topic` step (it SHALL NOT regenerate with the same scope and SHALL NOT spawn an agent). On confirm, the wizard SHALL transition to the `generating` step with live activity rendering, then on terminal success SHALL transition to `review_pending` and on user Start SHALL transition to `reviewing`. When the plan spawn yields no match, the wizard SHALL display the no-match reason and SHALL NOT start a generate spawn. While the plan or generate spawn is running, the system SHALL subscribe to the `quiz-stream` channel and render the streamed `VerbEvent`s through the existing agent stream rendering (the same `foldTimeline` + thought / activity-item pipeline used by the run detail view); a static "planning…" / "generating…" label alone SHALL NOT satisfy this — the live agent activity SHALL be visible as it happens, mirroring the goal flow.

The `[Quiz me on this]` control SHALL appear at the bottom of a wiki content page preview (it SHALL NOT appear for `index.md` or `log.md`). Activating it SHALL skip the plan spawn and SHALL launch the wizard directly into the `generating` step using that page plus its one-hop wikilinked pages.

While inside any in-quiz phase — `reviewing` (the answering view) or `completion` (the post-quiz summary) — the wizard SHALL render a back-to-quiz-history control (in the wizard `TabContentHeader` row for the completion sub-state, in the wizard answering footer or via wizard cancel control for the reviewing sub-state) that exits the wizard and returns to the quiz history list. Activating it SHALL NOT spawn an agent, and SHALL be non-destructive: answering progress is persisted, so reopening the attempt resumes at the saved position. This control is distinct from the wizard `cancel` control: cancel only applies while no committed attempt exists (the `scope_confirm`, `generating`, and `review_pending` steps); back-to-history applies once an attempt exists (`reviewing` and `completion`). Additionally, selecting the Quiz tab while it is already the active workspace tab SHALL exit the wizard (if active) and return the Quiz tab to its quiz-history view; selecting the Quiz tab from a different tab SHALL NOT reset an in-progress wizard.

The Quiz tab's pass/fail threshold and generated question count SHALL be sourced from the persisted application configuration loaded at workspace startup, and SHALL NOT require the Settings modal to have been opened in the session: the pass threshold SHALL come from `app.quiz.pass_threshold` (default 80 only when truly unset), and the generated question count SHALL come from the shared quiz length configuration (see capability `quiz`).

#### Scenario: New quiz shows planned scope before generating

- **WHEN** the user submits a topic via `+ New quiz` and the plan spawn emits a scope
- **THEN** the wizard SHALL transition to the `scope_confirm` step AND the planned scope SHALL be shown grouped by the bucket taxonomy AND confirm and revise controls SHALL be present AND the generate spawn SHALL NOT start until the user confirms

#### Scenario: Revise control is labeled and returns to topic input

- **GIVEN** the `scope_confirm` step is displayed with the revise control
- **WHEN** the user activates the revise control
- **THEN** the control SHALL read `重新規劃` (sourced from i18n, not a hardcoded string) AND the wizard SHALL return to the `topic` step AND SHALL NOT spawn an agent

#### Scenario: No-match topic shows reason and stops

- **WHEN** the plan spawn emits `[CODEBUS_QUIZ_NO_MATCH]`
- **THEN** the wizard SHALL display the no-match reason AND SHALL NOT start a generate spawn AND SHALL NOT persist any quiz file

#### Scenario: Quiz-me-on-this skips planning into generating step

- **WHEN** the user activates `[Quiz me on this]` on a wiki content page preview
- **THEN** no plan spawn SHALL run AND the wizard SHALL launch directly into the `generating` step AND the generate spawn SHALL run using that page plus its one-hop wikilinks

#### Scenario: Quiz-me-on-this hidden on nav pages

- **WHEN** the wiki preview shows `index.md` or `log.md`
- **THEN** the `[Quiz me on this]` control SHALL NOT be rendered

#### Scenario: Plan and generate agent activity is rendered live

- **GIVEN** the user submitted a topic via `+ New quiz` (or activated `[Quiz me on this]`)
- **WHEN** the plan or generate spawn emits `VerbEvent`s on the `quiz-stream` channel before its terminal payload
- **THEN** the wizard SHALL render those events through the existing agent stream rendering (thought, tool-use, and result items appear as they stream) AND SHALL NOT show only a static placeholder label

#### Scenario: + New quiz is not shown while the wizard is active

- **GIVEN** the wizard is in any step (topic, scope_confirm, generating, review_pending, reviewing, or completion)
- **THEN** the `+ New quiz` control SHALL NOT be rendered in the `TabContentHeader`
- **AND** the `+ New quiz` control SHALL be rendered only when the Quiz tab is on the quiz history list (and the wizard is not active)

#### Scenario: Back-to-history from the answering view

- **GIVEN** the wizard is in the `reviewing` sub-state (not yet on the completion summary)
- **WHEN** the user activates the back-to-quiz-history control
- **THEN** the wizard SHALL exit AND the Quiz tab SHALL show the quiz history list AND no `spawn_quiz_plan` or `spawn_quiz_generate` SHALL be invoked AND reopening that attempt SHALL resume at the saved position

#### Scenario: Back-to-history from the completion summary

- **GIVEN** the wizard is in the `completion` sub-state
- **WHEN** the user activates the back-to-quiz-history control
- **THEN** the wizard SHALL exit AND the Quiz tab SHALL show the quiz history list AND no agent SHALL be spawned

#### Scenario: Re-selecting the Quiz tab exits the wizard

- **GIVEN** the Quiz tab is the active workspace tab and the wizard is in any step
- **WHEN** the user selects the Quiz tab again
- **THEN** the wizard SHALL exit AND the Quiz tab SHALL show the quiz history list

#### Scenario: Threshold reflects persisted config without opening Settings

- **GIVEN** the persisted config has `app.quiz.pass_threshold` of 75 and the Settings modal has not been opened in this session
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the completion summary SHALL show a passing outcome (evaluated against 75, not the 80 default)
