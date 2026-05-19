## ADDED Requirements

### Requirement: Quiz Content Verify GUI Wiring

The `spawn_quiz_generate` Tauri IPC command SHALL participate in the optional content verification stage defined by the `quiz` capability's `Quiz Content Verification and Repair` requirement, with behavior parity to the CLI and without adding a new IPC command or a content-review UI element.

The command SHALL resolve `quiz.content_verify` from the shared `quiz.*` configuration using the same core loader the CLI uses (default `false`; a config load error SHALL fall back to `false` rather than silently enabling extra spawns). It SHALL derive the originating topic from the generation `trigger`: a `AiPlanned` trigger SHALL yield the topic (Goal flow, the off-topic content check runs), and a `WikiPreview` trigger SHALL yield no topic (Page flow, the off-topic check is skipped while the other four content checks still run). The resolved `content_verify` flag and topic SHALL be injected into the `QuizGenerateOptions` passed to `run_quiz_generate`, so a GUI-generated quiz is persisted with the same `content_review` frontmatter the CLI produces. When `quiz.content_verify` is `false`, the GUI generation flow SHALL be unchanged and no `content_review` SHALL be written.

#### Scenario: GUI threads config and topic into generation

- **WHEN** `spawn_quiz_generate` runs with an `AiPlanned` trigger and `quiz.content_verify` is `true`
- **THEN** the `QuizGenerateOptions` passed to `run_quiz_generate` SHALL carry `content_verify = true` and the originating topic AND the persisted quiz SHALL include the `content_review` frontmatter field

#### Scenario: GUI Page flow supplies no topic

- **WHEN** `spawn_quiz_generate` runs with a `WikiPreview` trigger and `quiz.content_verify` is `true`
- **THEN** the `QuizGenerateOptions` SHALL carry `content_verify = true` and no topic (off-topic check skipped; the other four content checks still run)

#### Scenario: GUI default-off leaves the flow unchanged

- **WHEN** `spawn_quiz_generate` runs and `quiz.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND the persisted quiz SHALL NOT contain a `content_review` field AND no new IPC command or content-review UI element SHALL be introduced
