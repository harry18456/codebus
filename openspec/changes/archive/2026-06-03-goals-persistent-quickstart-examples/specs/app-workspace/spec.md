## MODIFIED Requirements

### Requirement: Goals Overview List and Filter

The `Goals` tab main content area SHALL render a vertical list of goal-mode runs from the active vault, sorted by `started_at` descending (newest first). The list SHALL include only `RunLog` entries whose `mode` field equals the literal string `"goal"`. Runs with `mode` equal to `"chat"`, `"query"`, `"fix"`, or `"quiz"` SHALL NOT appear in this list. Each row SHALL display: an outcome icon (`⚪` for running, `✓` for done, `⏹` for cancelled, `⚠` for interrupted), the goal text (truncated to ~80 chars with ellipsis), and a relative timestamp (e.g., "2m ago", "1h ago").

The list SHALL also include virtual `outcome="interrupted"` entries detected per the `Interrupted Run Detection` requirement. Virtual entries SHALL render with the same row shape but with the `⚠` outcome icon.

The Goals tab MUST render a content header row at the top of the main content area using the shared `TabContentHeader` component (see capability `design-system`). The header row SHALL display: an h1 title (i18n key `workspace.goals.headerTitle`), a subtitle (i18n key `workspace.goals.headerSubtitle`), a `[+ New goal]` CTA on the right (i18n key `workspace.goals.newGoalButton`), and a single-character shortcut chip with the literal text `N` next to the CTA. The legacy standalone right-aligned topbar that wrapped only the `[+ New goal]` button SHALL NOT remain — the CTA SHALL live inside the content header row. Clicking the CTA SHALL open the New Goal modal.

The `N` shortcut chip SHALL label a working keyboard binding: while the Goals tab is the active tab and the user is not typing inside an `INPUT`, `TEXTAREA`, or `contenteditable` target, pressing the bare `N` key (no `Cmd`, `Ctrl`, `Alt`, or `Shift` modifier) SHALL open the New Goal modal. Pressing `N` while the New Goal modal is already open SHALL be a no-op (the user is presumed to be typing the letter into the modal's textarea). The bare `N` binding SHALL NOT compete with the `Cmd+N` / `Ctrl+N` "new vault" shortcut bound in the Lobby route by `useNewVaultShortcut`.

The Goals tab SHALL expose the same ordered set of exactly four example starter prompts in BOTH the empty state and the non-empty state, sourced from a single ordered set of i18n keys `workspace.goals.examplePlaceholder1` through `workspace.goals.examplePlaceholder4`. The example labels SHALL NOT be hard-coded literals in the component source. Clicking any example (pill or chip) SHALL open the New Goal modal with that example pre-filled in the textarea.

When the list is non-empty (one or more goal runs), the Goals tab MUST render, below the content header row and in top-to-bottom order: (1) a persistent quick-start examples region, then (2) a `RECENT` section label above the goal list. The `RECENT` section label SHALL use the `SectionLabel` component with `variant="caps"` (see capability `design-system`); the literal text `RECENT` is an identifier and SHALL NOT be translated. The quick-start examples region SHALL render a `SectionLabel` with `variant="caps"` whose text comes from i18n key `workspace.goals.quickStartLabel`, followed by exactly four quick-start chip buttons whose labels come from i18n keys `workspace.goals.examplePlaceholder1` through `workspace.goals.examplePlaceholder4` respectively.

When the list is empty (no goal runs in the vault), the main content area MUST render a three-region vertical layout below the content header row: (1) a centered hero region containing an emoji indicator, a headline (i18n key `workspace.goals.emptyHeroTitle`), and a subtitle (i18n key `workspace.goals.emptyHeroSubtitle`); (2) an examples region containing exactly four pre-fill example pill buttons. Each pill button's label SHALL come from i18n keys `workspace.goals.examplePlaceholder1`, `workspace.goals.examplePlaceholder2`, `workspace.goals.examplePlaceholder3`, and `workspace.goals.examplePlaceholder4` respectively.

#### Scenario: Goals overview filters to goal mode only

- **WHEN** the active vault contains `runs-*.jsonl` rows with `mode` values of `"goal"`, `"chat"`, `"query"`, `"fix"`, and `"quiz"`
- **THEN** the Goals overview list renders exactly the `"goal"`-mode rows AND no rows whose `mode` is `"chat"`, `"query"`, `"fix"`, or `"quiz"` appear

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

#### Scenario: Populated Goals overview shows persistent quick-start examples above RECENT

- **WHEN** the goal list has at least one goal row
- **THEN** a quick-start examples region renders above the `RECENT` section label containing a `SectionLabel` with `variant="caps"` whose text is the locale value of `workspace.goals.quickStartLabel` AND exactly four quick-start chip buttons whose labels are the locale values of `workspace.goals.examplePlaceholder1` through `workspace.goals.examplePlaceholder4` AND clicking any chip opens the New Goal modal with that example text pre-filled in the textarea

#### Scenario: Empty Goals overview shows three-region layout with i18n-keyed pre-fill examples

- **WHEN** the active vault has zero `mode=goal` rows in `runs-*.jsonl` AND no orphan `events-*.jsonl` files
- **THEN** the Goals tab renders a content header row at the top AND a centered hero region containing the headline from `workspace.goals.emptyHeroTitle` and the subtitle from `workspace.goals.emptyHeroSubtitle` AND an examples region with exactly four clickable pill buttons whose labels come from `workspace.goals.examplePlaceholder1` through `workspace.goals.examplePlaceholder4` AND clicking any pill opens the New Goal modal with that example text in the textarea

#### Scenario: Pre-fill example labels are not hard-coded English literals

- **WHEN** the runtime locale switches between Traditional Chinese and English
- **THEN** the empty-state pill labels and the non-empty-state quick-start chip labels both render the locale-specific value of `workspace.goals.examplePlaceholder1` through `workspace.goals.examplePlaceholder4` AND no English literal such as `"describe what this project does"` SHALL appear in the Traditional Chinese locale

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
