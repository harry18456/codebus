## ADDED Requirements

### Requirement: Workspace Sidebar Nav Row Visual Contract

The Workspace sidebar SHALL render each of its three tab navigation rows (`Goals`, `Wiki`, `Quiz`) as a single horizontal row composed, in order, of:

1. an inline emoji prefix rendered inside `<span aria-hidden="true">` (🚏 for `Goals`, 📂 for `Wiki`, 🎓 for `Quiz`),
2. the localized tab label, and
3. a right-aligned mono-numeric count rendered in a tabular-nums monospace style with tertiary foreground color.

The emoji prefix SHALL be encoded directly in the component source, SHALL NOT be sourced from an i18n message value, and SHALL be visually separated from the label by a fixed gap (not by a literal whitespace character inside the label string).

The mono count SHALL display `0` as a literal numeric `0` when the underlying store is empty or still loading; it SHALL NOT be hidden, suppressed, or replaced with a placeholder character in that case.

The count source for each row SHALL be read from a global store via a selector. Specifically: `Goals` count SHALL be `useGoalsStore().runs.length`, `Wiki` count SHALL be `useWikiStore().pages.length`, and `Quiz` count SHALL be derived from a dedicated quiz-history store whose `attempts` collection is loaded and reset on Workspace mount / unmount and is kept in sync via the `quiz-changed` watcher event (the same channel `QuizTab` already subscribes to). Counts SHALL NOT be passed into the sidebar via component props (no prop drilling).

The currently active nav row SHALL display a 2px-wide vertical accent-color bar at its left edge as the primary "you are here" indicator. Non-active rows SHALL NOT render this bar (zero-opacity placeholders are not permitted). The active row's prior whole-row accent-tint fill (`bg-accent/20 text-accent`) SHALL be removed or weakened so the left bar is the dominant active-state signal; any residual active-label emphasis (color, weight, or tint) SHALL remain subtle enough that it does not compete with the left bar.

Keyboard focus rings, hover affordances, and the existing `data-testid="workspace-tab-<id>"` and `data-active` attributes on each row SHALL be preserved.

#### Scenario: Each nav row renders emoji prefix, label, and right-aligned count

- **WHEN** the user opens a vault and the Workspace sidebar renders
- **THEN** each of the three nav rows displays its emoji prefix (🚏 / 📂 / 🎓) inside an `aria-hidden` span, followed by the localized label, followed by a right-aligned mono-numeric count whose value matches the corresponding store length

##### Example: row composition

| Tab id | Emoji | Label (en) | Count source |
| ------ | ----- | ---------- | ------------ |
| `goals` | 🚏 | `Goals` | `useGoalsStore().runs.length` |
| `wiki` | 📂 | `Wiki` | `useWikiStore().pages.length` |
| `quiz` | 🎓 | `Quiz` | `useQuizHistoryStore().attempts.length` |

#### Scenario: Active row shows a left amber bar and inactive rows do not

- **WHEN** the user is on the `Wiki` tab
- **THEN** the `Wiki` nav row renders a 2px accent-color vertical bar at its left edge AND the `Goals` and `Quiz` rows do not render such a bar

#### Scenario: Switching active tab moves the left amber bar without residue

- **WHEN** the user clicks `Goals`, then `Wiki`, then `Quiz` in succession
- **THEN** at each step exactly one row carries the 2px left accent bar, the bar follows the currently active row, and no DOM element retains a stale active-state class or visible bar

#### Scenario: Nav count reflects store changes in real time

- **GIVEN** the Workspace is open on a vault with two existing goal runs
- **WHEN** the user creates a new goal via `+ New goal` and the spawn resolves
- **THEN** the `Goals` nav row count increments to 3 without any tab switch, page reload, or sidebar remount

#### Scenario: Empty store still displays a literal zero count

- **GIVEN** a freshly opened vault whose wiki page index, goal runs, and quiz attempts are all empty
- **WHEN** the Workspace sidebar finishes its initial render
- **THEN** each of the three nav rows displays a literal `0` as its right-aligned mono count (not blank, not a dash, not hidden)

#### Scenario: Emoji prefix is not part of the i18n message value

- **WHEN** the i18n message catalog is inspected for `workspace.tab.goals`, `workspace.tab.wiki`, and `workspace.tab.quiz`
- **THEN** none of the three message values contain the emoji characters 🚏, 📂, or 🎓; the emoji characters appear only in the sidebar component source

---

### Requirement: Workspace Sidebar Footer

The Workspace sidebar SHALL render, at the bottom of the `<aside>` element, a footer row positioned via `mt-auto` so it remains pinned to the sidebar's lower edge. The footer row SHALL contain, at minimum:

1. a Settings icon button on the left whose accessible name and tooltip reuse the existing `bottomStrip.settings` i18n message key and whose click handler opens the same application-shell-level Settings modal that the Lobby BottomStrip's gear opens, and
2. a visual keyboard-shortcut chip on the right rendered as `<kbd>⌘</kbd><kbd>K</kbd>` (or equivalent paired `<kbd>` elements), marked `aria-hidden`, indicating the `Cmd+K` / `Ctrl+K` ChatWidget toggle. The literal text `⌘K` SHALL NOT be translated.

The footer row SHALL NOT contain a manual refresh control. The Workspace SHALL rely on the existing per-vault watcher for index refresh; a manual refresh button is explicitly excluded as visual noise.

The Settings invocation from the sidebar footer SHALL share state with the Lobby's BottomStrip invocation: a single application-shell `settingsOpen` state and a single `<SettingsModal>` instance SHALL serve both entry points. The Workspace component SHALL receive the open-settings callback via a prop from the application shell; it SHALL NOT mount its own Settings modal instance.

#### Scenario: Sidebar footer renders settings button and shortcut chip without refresh control

- **WHEN** the Workspace sidebar is mounted
- **THEN** the sidebar bottom contains a footer row with a Settings icon button on the left AND a `⌘K` keyboard-shortcut chip on the right AND no refresh button or refresh affordance

#### Scenario: Clicking sidebar Settings button opens the same modal as the Lobby gear

- **GIVEN** the user is in the Workspace
- **WHEN** the user clicks the sidebar footer's Settings icon button
- **THEN** the application-shell `<SettingsModal>` opens centered over a dimmed Workspace background, identical in identity and behavior to the modal opened by the Lobby BottomStrip's gear

#### Scenario: Sidebar Settings button reuses bottomStrip.settings i18n key

- **WHEN** the sidebar footer Settings button renders in any supported locale
- **THEN** its `aria-label` and tooltip text are sourced from the existing `bottomStrip.settings` i18n message key (no new i18n key is introduced for the sidebar Settings button)

---

### Requirement: Workspace Sidebar Section Label Policy

The Workspace sidebar nav region SHALL NOT render any section label above its three tab rows, including but not limited to a `VAULT` label, in any locale. This is a deliberate departure from the v1 design mock's `VAULT` section label and SHALL be preserved across future visual revisions until a multi-group sidebar nav is introduced.

#### Scenario: Sidebar nav has no section label above tabs

- **WHEN** the Workspace sidebar is rendered in any supported locale
- **THEN** the DOM region between the vault display-name / path block and the first nav row contains no `<div>`, `<span>`, or `<SectionLabel>` element rendering the literal text `VAULT` or any other section-label-style heading
