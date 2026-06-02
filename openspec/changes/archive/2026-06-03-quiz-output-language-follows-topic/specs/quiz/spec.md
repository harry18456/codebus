## ADDED Requirements

### Requirement: Quiz Generate Spawn Carries Topic Language Signal

`run_quiz_generate` SHALL reuse the existing `QuizGenerateOptions.topic: Option<String>` field as the language signal for the generate spawn. When `topic` is `Some`, the generate spawn `input` string SHALL include a `topic=<topic>` segment alongside the existing `pages=[<path1>,<path2>,...] count=<N>` segment, so the generate agent can follow the topic language per the §0 Language Policy quiz rule. When `topic` is `None` (the Page flow / wiki-preview `[Quiz me on this]`), the generate spawn `input` SHALL retain its current shape with no `topic=` segment, preserving existing Page-flow behavior exactly.

The same `topic=<topic>` segment SHALL be carried on the content-verify repair (regenerate) spawn `input` when `topic` is `Some`, so a repair iteration cannot drift the quiz language away from the topic; when `topic` is `None` the repair spawn `input` SHALL omit the `topic=` segment. This change SHALL NOT alter the IPC or CLI plumbing of `topic` (both callers already populate `QuizGenerateOptions.topic`); it only reuses the already-plumbed value at the generate/repair spawn `input` composition site.

The composition of the generate spawn `input` SHALL follow the same style as the existing `compose_verify_input` helper (a small pure function producing the `input` string from `topic`, `pages`, and `count`), keeping the topic-prefix shape consistent and unit-testable.

#### Scenario: Goal-flow generate spawn input carries the topic

- **WHEN** `run_quiz_generate` is invoked with `QuizGenerateOptions { pages, question_count, topic: Some("JWT 簽發與驗證") }`
- **THEN** the generate spawn `input` string SHALL contain the segment `topic=JWT 簽發與驗證` AND SHALL contain the `pages=[...] count=<N>` segment

#### Scenario: Page-flow generate spawn input omits the topic

- **WHEN** `run_quiz_generate` is invoked with `QuizGenerateOptions { pages, question_count, topic: None }`
- **THEN** the generate spawn `input` string SHALL be the `pages=[...] count=<N>` shape with no `topic=` segment, identical to the pre-change Page-flow behavior

#### Scenario: Repair regenerate spawn preserves the topic signal

- **WHEN** content-verify is enabled, `topic` is `Some`, and the verify spawn flags a question so a repair (regenerate) spawn runs
- **THEN** the repair spawn `input` SHALL also contain the `topic=<topic>` segment alongside its `pages=[...] count=<N>` and defect-revision instructions

##### Example: Generate input shape by topic presence

| `topic` | Generate spawn `input` (modulo whitespace) |
| ------- | ------------------------------------------ |
| `Some("中文主題")` | `topic=中文主題` followed by `pages=[...] count=5` |
| `Some("JWT issuance")` | `topic=JWT issuance` followed by `pages=[...] count=5` |
| `None` | `pages=[...] count=5` (no `topic=` segment) |
