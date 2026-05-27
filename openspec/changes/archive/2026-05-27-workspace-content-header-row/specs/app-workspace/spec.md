## MODIFIED Requirements

### Requirement: Goals Overview List and Filter

The `Goals` tab main content area SHALL render a vertical list of goal-mode runs from the active vault, sorted by `started_at` descending (newest first). The list SHALL include only `RunLog` entries whose `mode` field equals the literal string `"goal"`. Runs with `mode` equal to `"chat"`, `"query"`, or `"fix"` SHALL NOT appear in this list. Each row SHALL display: an outcome icon (`⚪` for running, `✓` for done, `⏹` for cancelled, `⚠` for interrupted), the goal text (truncated to ~80 chars with ellipsis), and a relative timestamp (e.g., "2m ago", "1h ago").

The list SHALL also include virtual `outcome="interrupted"` entries detected per the `Interrupted Run Detection` requirement. Virtual entries SHALL render with the same row shape but with the `⚠` outcome icon.

The Goals tab MUST render a content header row at the top of the main content area using the shared `TabContentHeader` component (see capability `design-system`). The header row SHALL display: an h1 title (i18n key `workspace.goals.headerTitle`), a subtitle (i18n key `workspace.goals.headerSubtitle`), a `[+ New goal]` CTA on the right (i18n key `workspace.goals.newGoalButton`), and a single-character shortcut chip with the literal text `N` next to the CTA. The legacy standalone right-aligned topbar that wrapped only the `[+ New goal]` button SHALL NOT remain — the CTA SHALL live inside the content header row. Clicking the CTA SHALL open the New Goal modal.

The `N` shortcut chip SHALL label a working keyboard binding: while the Goals tab is the active tab and the user is not typing inside an `INPUT`, `TEXTAREA`, or `contenteditable` target, pressing the bare `N` key (no `Cmd`, `Ctrl`, `Alt`, or `Shift` modifier) SHALL open the New Goal modal. Pressing `N` while the New Goal modal is already open SHALL be a no-op (the user is presumed to be typing the letter into the modal's textarea). The bare `N` binding SHALL NOT compete with the `Cmd+N` / `Ctrl+N` "new vault" shortcut bound in the Lobby route by `useNewVaultShortcut`.

When the list is non-empty (one or more goal runs), the Goals tab MUST render a `RECENT` section label above the goal list using the `SectionLabel` component with `variant="caps"` (see capability `design-system`). The literal text `RECENT` is an identifier and SHALL NOT be translated.

When the list is empty (no goal runs in the vault), the main content area MUST render a three-region vertical layout below the content header row: (1) a centered hero region containing an emoji indicator, a headline (i18n key `workspace.goals.emptyHeroTitle`), and a subtitle (i18n key `workspace.goals.emptyHeroSubtitle`); (2) an examples region containing exactly three pre-fill example pill buttons. Each pill button's label SHALL come from i18n keys `workspace.goals.examplePlaceholder1`, `workspace.goals.examplePlaceholder2`, and `workspace.goals.examplePlaceholder3` respectively; the pill label text SHALL NOT be a hard-coded English literal in the component source. Clicking any pill button SHALL open the New Goal modal with that example pre-filled in the textarea.

#### Scenario: Goals overview filters to goal mode only

- **WHEN** the active vault contains `runs-*.jsonl` rows with `mode` values of `"goal"`, `"chat"`, `"query"`, and `"fix"`
- **THEN** the Goals overview list renders exactly the `"goal"`-mode rows AND no rows from the other three modes appear

#### Scenario: Goals tab renders content header row with CTA and shortcut chip

- **WHEN** the user opens a vault and the Workspace mounts on the Goals tab (regardless of whether the goal list is empty or populated)
- **THEN** the Goals tab content area renders a content header row containing an h1 title from `workspace.goals.headerTitle`, a subtitle from `workspace.goals.headerSubtitle`, a `[+ New goal]` CTA from `workspace.goals.newGoalButton`, and an `N` shortcut chip AND no other independent right-aligned topbar row wrapping only the CTA SHALL exist

#### Scenario: Bare N opens the New Goal modal while Goals tab is active

- **WHEN** the user is on the Goals tab with no input/textarea focused and presses the bare `N` key (no Cmd/Ctrl/Alt/Shift modifier)
- **THEN** the New Goal modal SHALL open

#### Scenario: N is ignored while typing inside the New Goal modal textarea

- **WHEN** the New Goal modal is open and the user types the letter `n` inside the textarea
- **THEN** the letter `n` SHALL appear in the textarea AND the modal SHALL NOT re-open or re-fire the shortcut

#### Scenario: Populated Goals overview renders RECENT section label

- **WHEN** the goal list has at least one goal row
- **THEN** a `SectionLabel` with `variant="caps"` and visible text `RECENT` renders above the goal list `ul`

#### Scenario: Empty Goals overview shows three-region layout with i18n-keyed pre-fill examples

- **WHEN** the active vault has zero `mode=goal` rows in `runs-*.jsonl` AND no orphan `events-*.jsonl` files
- **THEN** the Goals tab renders a content header row at the top AND a centered hero region containing the headline from `workspace.goals.emptyHeroTitle` and the subtitle from `workspace.goals.emptyHeroSubtitle` AND an examples region with exactly three clickable pill buttons whose labels come from `workspace.goals.examplePlaceholder1`, `workspace.goals.examplePlaceholder2`, and `workspace.goals.examplePlaceholder3` AND clicking any pill opens the New Goal modal with that example text in the textarea

#### Scenario: Pre-fill example labels are not hard-coded English literals

- **WHEN** the runtime locale switches between Traditional Chinese and English
- **THEN** the three pre-fill example pill labels render the locale-specific value of `workspace.goals.examplePlaceholder1`, `workspace.goals.examplePlaceholder2`, and `workspace.goals.examplePlaceholder3` AND no English literal such as `"describe the authentication flow"` SHALL appear in the Traditional Chinese locale

#### Scenario: Run row outcome icon matches RunLog outcome

- **WHEN** the Goals overview list renders a row corresponding to a RunLog entry with `outcome="cancelled"`
- **THEN** the row's leading icon SHALL be `⏹` AND the row remains clickable to navigate to the Cancelled detail view

##### Example: row icon mapping

| RunLog outcome | Row icon |
| -------------- | -------- |
| (active run in progress, no RunLog row yet) | `⚪` |
| `succeeded` | `✓` |
| `cancelled` | `⏹` |
| `failed` | `⚠` |
| virtual `interrupted` (events have no RunLog row) | `⚠` |

## ADDED Requirements

### Requirement: Quiz Tab Content Header Row

The Quiz tab MUST render a content header row at the top of the main content area using the shared `TabContentHeader` component (see capability `design-system`) whenever the Quiz tab is in its history-listing view (the phase that renders the quiz history list or its empty-history hint). The header row SHALL display: an h1 title (i18n key `workspace.quiz.headerTitle`), a subtitle (i18n key `workspace.quiz.headerSubtitle`), and a `[+ New quiz]` CTA on the right (i18n key `workspace.quiz.tab.newButton`). The header row SHALL NOT render a shortcut chip on the Quiz tab. Clicking the `[+ New quiz]` CTA SHALL transition the Quiz tab to its new-quiz input phase (existing behavior).

When the Quiz tab is in any non-history phase (planning, confirm, generating, ready, no-match, error, attempt, review, idle), the Quiz tab SHALL NOT render the content header row. Those phases retain their existing in-flow layout and are out of scope for this requirement.

The legacy h2 heading that consumed `workspace.quiz.tab.heading` directly inside the Quiz tab's history view SHALL be replaced by the content header row's h1 (sourced from `workspace.quiz.headerTitle`); the `workspace.quiz.tab.heading` i18n key SHALL remain defined for backward compatibility but is no longer consumed by the Quiz tab history view.

#### Scenario: Quiz tab history view renders content header row

- **WHEN** the user opens the Quiz tab in its history-listing view (empty or populated)
- **THEN** a content header row renders at the top of the main content area containing an h1 title from `workspace.quiz.headerTitle`, a subtitle from `workspace.quiz.headerSubtitle`, and a `[+ New quiz]` CTA from `workspace.quiz.tab.newButton` AND no shortcut chip renders next to the CTA

#### Scenario: Non-history Quiz phases omit the content header row

- **WHEN** the Quiz tab transitions to any of the planning, confirm, generating, ready, no-match, error, attempt, review, or idle phases
- **THEN** the content header row SHALL NOT render in those phases AND the in-flow phase content occupies the main content area without it

#### Scenario: Quiz tab CTA opens new-quiz input

- **WHEN** the user clicks the `[+ New quiz]` CTA in the Quiz tab content header row
- **THEN** the Quiz tab transitions to its new-quiz input phase (existing behavior preserved)
