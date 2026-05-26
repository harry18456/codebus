## ADDED Requirements

### Requirement: Quiz History Row Title Displays User-Authored Topic

The codebus-app Quiz history list SHALL display each quiz attempt row using the user-authored topic (the free-text string the user typed when starting the quiz, or the page title for the Page flow) as the row's primary title. The internal hash-derived slug used for filesystem layout (e.g., `topic-a7fb67fc`, derived from `<vault>/.codebus/quiz/<slug>/` per the Quiz Storage Layout requirement) SHALL NOT be the row's primary title.

When the user-authored topic is available — either from the quiz file's caller-injected `topic` frontmatter (Goal flow) or `target_page` frontmatter (Page flow) — the row's primary title SHALL be that value verbatim. The slug MAY be retained as supporting metadata in a secondary visual position (subtitle, tooltip, or hidden) but SHALL NOT visually dominate the row.

When neither `topic` nor `target_page` frontmatter is present (e.g., a legacy attempt file or an unparseable frontmatter), the row SHALL fall back to the slug as the primary title rather than rendering an empty string, so the row remains identifiable even for degraded data.

#### Scenario: Goal-flow quiz shows the user's topic, not the slug

- **GIVEN** a quiz attempt persisted under `<vault>/.codebus/quiz/topic-a7fb67fc/2026-05-25T16-53-17Z.md` whose caller-injected frontmatter is `topic: 專案目的`
- **WHEN** the codebus-app Quiz history list renders this attempt's row
- **THEN** the row's primary title SHALL be `專案目的` AND SHALL NOT be `topic-a7fb67fc`

#### Scenario: Page-flow quiz shows the target page name

- **GIVEN** a quiz attempt persisted under `<vault>/.codebus/quiz/desktop-workspace/2026-05-25T17-10-04Z.md` whose caller-injected frontmatter is `target_page: 桌面工作台` and has no `topic` field
- **WHEN** the codebus-app Quiz history list renders this attempt's row
- **THEN** the row's primary title SHALL be `桌面工作台`

#### Scenario: Legacy attempt without topic frontmatter falls back to slug

- **GIVEN** a quiz attempt whose frontmatter contains neither `topic` nor `target_page`
- **WHEN** the codebus-app Quiz history list renders this attempt's row
- **THEN** the row's primary title SHALL be the slug from the attempt's directory path AND SHALL NOT be an empty string
