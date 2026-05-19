## MODIFIED Requirements

### Requirement: Quiz Tab Plan-Confirm-Generate Flow

The Quiz tab SHALL offer two entry points that converge after generation. `+ New quiz` SHALL open a free-text topic input; submitting it SHALL run a plan spawn whose agent activity is rendered live via the existing agent stream rendering. When the plan spawn yields a planned scope, the system SHALL display the planned `wiki/` page list together with a description stating that the quiz will be generated from those listed pages, alongside a revise control labeled `重新規劃` and a confirm control labeled `確認`, and SHALL NOT start the generate spawn until the user confirms. The confirm-view description text and the `重新規劃` / `確認` control labels SHALL be sourced from the application i18n system (the `useT()` / `messages.ts` mechanism, with `en` and `zh-tw` entries), and SHALL NOT be hardcoded literal strings in the component. Activating `重新規劃` SHALL return to the topic-input view to re-plan (it SHALL NOT regenerate with the same scope and SHALL NOT spawn an agent). On confirm, the system SHALL run the generate spawn with live activity rendering, then transition to the answering view. When the plan spawn yields no match, the system SHALL display the no-match reason and SHALL NOT start a generate spawn. While the plan or generate spawn is running, the system SHALL subscribe to the `quiz-stream` channel and render the streamed `VerbEvent`s through the existing agent stream rendering (the same `foldTimeline` + thought / activity-item pipeline used by the run detail view); a static "planning…" / "generating…" label alone SHALL NOT satisfy this — the live agent activity SHALL be visible as it happens, mirroring the goal flow.

The `[Quiz me on this]` control SHALL appear at the bottom of a wiki content page preview (it SHALL NOT appear for `index.md` or `log.md`). Activating it SHALL skip the plan spawn and run the generate spawn directly using that page plus its one-hop wikilinked pages.

While inside any in-quiz phase — answering or the post-quiz summary — the Quiz tab SHALL render a back-to-quiz-history control that returns to the quiz history list. Activating it SHALL NOT spawn an agent, and SHALL be non-destructive: answering progress is persisted, so reopening the attempt resumes at the saved position. This control is distinct from `+ New quiz` (which remains hidden inside a quiz). Additionally, selecting the Quiz tab while it is already the active workspace tab SHALL return the Quiz tab to its quiz-history view; selecting the Quiz tab from a different tab SHALL NOT reset an in-progress quiz.

The Quiz tab's pass/fail threshold and generated question count SHALL be sourced from the persisted application configuration loaded at workspace startup, and SHALL NOT require the Settings modal to have been opened in the session: the pass threshold SHALL come from `app.quiz.pass_threshold` (default 80 only when truly unset), and the generated question count SHALL come from the shared quiz length configuration (see capability `quiz`).

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

- **GIVEN** the Quiz tab is in a quiz flow or attempt view (planning, confirm, generating, answering, summary, no-match, error, or an opened attempt)
- **THEN** the `+ New quiz` control SHALL NOT be rendered
- **AND** the `+ New quiz` control SHALL be rendered only in the history list and the topic-input compose screen

#### Scenario: Back-to-history from the answering view

- **GIVEN** the user is answering a quiz (not yet on the summary)
- **WHEN** the user activates the back-to-quiz-history control
- **THEN** the Quiz tab SHALL show the quiz history list AND no `spawn_quiz_plan` or `spawn_quiz_generate` SHALL be invoked AND reopening that attempt SHALL resume at the saved position

#### Scenario: Back-to-history from the summary

- **GIVEN** the user has finished a quiz and the summary screen is shown
- **WHEN** the user activates the back-to-quiz-history control
- **THEN** the Quiz tab SHALL show the quiz history list AND no agent SHALL be spawned

#### Scenario: Re-selecting the Quiz tab returns to quiz history

- **GIVEN** the Quiz tab is the active workspace tab and is inside a quiz flow or attempt view
- **WHEN** the user selects the Quiz tab again
- **THEN** the Quiz tab SHALL return to its quiz-history view

#### Scenario: Threshold reflects persisted config without opening Settings

- **GIVEN** the persisted config has `app.quiz.pass_threshold` of 75 and the Settings modal has not been opened in this session
- **WHEN** the user finishes a 5-question quiz with 4 correct (80%)
- **THEN** the summary SHALL show a passing outcome (evaluated against 75, not the 80 default)
