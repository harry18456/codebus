## ADDED Requirements

### Requirement: Quiz Spawn Single-Fire and Concurrency Guard

A single user action that starts a quiz (the wiki-preview "Quiz me on this" Page flow, or the Goal-flow confirm) SHALL produce exactly one quiz attempt. The system SHALL prevent a single trigger from spawning more than one quiz generation, both at the frontend trigger layer and at the backend IPC layer.

**Frontend single-fire:** The QuizTab Page-flow effect that reacts to the `pendingPage` prop SHALL invoke the generate path (`spawnQuizGenerate`) at most once per distinct `pendingPage` value, even when the effect is invoked multiple times for the same value (e.g., React `StrictMode` double-invocation in development, or repeated renders). The guard SHALL be a per-value latch (a ref recording the value already fired for), not a dependency change.

**Backend concurrency guard:** The `spawn_quiz_plan` and `spawn_quiz_generate` IPC commands SHALL reject a new spawn when a quiz run is already active, returning `AppError::Invalid { field: "active_runs", .. }` and SHALL NOT spawn a second background run nor register a second entry. "A quiz run is already active" SHALL be determined by `ActiveRuns::has_quiz_run()`, which reports whether any active run id carries the `quiz-` prefix (covering both `quiz-plan-*` and `quiz-generate-*` ids). This mirrors the existing goal (`has_goal_run`) and chat (`has_chat_turn`) concurrency rejection.

**Run id uniqueness:** The quiz run id SHALL be generated with sub-second (millisecond) precision so that two spawns occurring within the same second receive distinct ids and do not collide in the `ActiveRuns` map (which would otherwise overwrite the first run's cancel handle).

#### Scenario: Page flow fires generation once under repeated effect invocation

- **WHEN** the QuizTab Page-flow effect is invoked two or more times for the same non-null `pendingPage` value (e.g., StrictMode double-invoke)
- **THEN** `spawnQuizGenerate` SHALL be called exactly once

#### Scenario: Second concurrent quiz spawn is rejected

- **WHEN** a quiz run (id prefixed `quiz-`) is already present in `ActiveRuns` AND `spawn_quiz_generate` (or `spawn_quiz_plan`) is invoked again
- **THEN** the command SHALL return `AppError::Invalid { field: "active_runs" }` AND SHALL NOT start a second background run

#### Scenario: has_quiz_run distinguishes quiz from chat and goal ids

- **WHEN** `ActiveRuns` contains a key prefixed `quiz-`
- **THEN** `has_quiz_run()` SHALL return true; AND when it contains only `chat-` prefixed or unprefixed (goal) keys, `has_quiz_run()` SHALL return false

#### Scenario: Quiz run ids generated in the same second are distinct

- **WHEN** the quiz run id generator is called twice within the same wall-clock second
- **THEN** the two generated ids SHALL differ (millisecond-precision timestamp)

#### Scenario: One trigger yields one attempt file

- **WHEN** the user activates "Quiz me on this" once for a wiki page
- **THEN** exactly one quiz attempt markdown file SHALL be persisted under `<vault>/.codebus/quiz/<slug>/`
