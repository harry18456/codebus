# app-workspace Specification

## Purpose

TBD - created by archiving change 'v3-app-workspace-goal'. Update Purpose after archive.

## Requirements

### Requirement: Workspace Layout and Tab Navigation

The system SHALL replace the Workspace stub with a real Workspace shell composed of a left sidebar with exactly three tabs and a main content area. The three tabs SHALL be `Goals` (default selection), `Wiki`, and `Quiz`. The sidebar SHALL also render the active vault's display name + path block and a `← Back to Lobby` control. Selecting a tab SHALL switch the main content area to the corresponding view; unselected tabs SHALL not retain their internal scroll position across switches (each switch SHALL re-mount the inactive view).

The `Quiz` tab SHALL render the quiz history list and a `+ New quiz` control as defined in the Quiz Tab Flow requirements of this capability. The `Quiz` tab SHALL NOT render the v1 "Coming soon" placeholder.

#### Scenario: Workspace mounts with Goals tab selected

- **WHEN** the user opens a vault from the Lobby
- **THEN** the main view transitions to the Workspace AND the `Goals` tab is the active selection AND the `Goals` main content area is rendered

#### Scenario: Tab switch re-renders main content

- **WHEN** the user clicks the `Wiki` tab while `Goals` is active
- **THEN** the main content area renders the Wiki tab content AND the `Goals` main content area is unmounted

#### Scenario: Quiz tab shows history and new-quiz control

- **WHEN** the user clicks the `Quiz` tab
- **THEN** the main content area renders the quiz history list AND a `+ New quiz` control AND SHALL NOT render the literal text "Coming soon — quiz flow ships in v3-app-quiz"

#### Scenario: Back to Lobby control returns to Lobby

- **WHEN** the user clicks the `← Back to Lobby` control while in the Workspace
- **THEN** the main view returns to the Lobby state AND the Workspace component is unmounted AND any active goal run continues running in the background


<!-- @trace
source: v3-app-quiz
updated: 2026-05-16
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/spike-artifacts/quiz-fixture-vault/manifest.yaml
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/jwt-token-lifecycle.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/index.md
  - docs/spike-artifacts/spike-quiz-7-F5.jsonl
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/config/quiz.rs
  - docs/spike-artifacts/spike-quiz-7-F1.jsonl
  - codebus-app/src-tauri/src/ipc/config.rs
  - docs/2026-05-15-v3-app-quiz-spike-plan.md
  - docs/spike-artifacts/spike-quiz-7-F6.jsonl
  - docs/spike-artifacts/spike-quiz-8-E3.jsonl
  - docs/spike-artifacts/spike-quiz-9-S1.jsonl
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-cli/src/commands/mod.rs
  - codebus-core/src/config/claude_code.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run2.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC1.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/user-store.md
  - docs/spike-artifacts/spike-quiz-10-R1-run1.jsonl
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-core/src/skill_bundle/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/wiki/log.md
  - docs/spike-artifacts/spike-quiz-7-F2.jsonl
  - docs/spike-artifacts/spike-quiz-8-E4.jsonl
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - docs/2026-05-15-v3-app-quiz-discussion.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/session-vs-token.md
  - docs/spike-artifacts/spike-quiz-8-E5.jsonl
  - codebus-cli/src/commands/quiz.rs
  - docs/spike-artifacts/spike-quiz-9-S3.jsonl
  - codebus-core/src/config/mod.rs
  - codebus-core/src/log/events/sink.rs
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - docs/spike-artifacts/spike-quiz-runbook.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/verb/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/CLAUDE.md
  - codebus-core/src/verb/event.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - docs/spike-artifacts/spike-quiz-8-E2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/raw/code/auth.py
  - docs/spike-artifacts/spike-quiz-8-E1.jsonl
  - docs/spike-artifacts/spike-quiz-7-F3.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/auth-middleware.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/processes/login-flow.md
  - docs/spike-artifacts/spike-quiz-9-S2.jsonl
  - codebus-core/src/vault/source_gitignore.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run3.jsonl
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/spike-artifacts/spike-quiz-7-F4.jsonl
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-core/tests/verb_library_surface.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/workspace/QuizAnswering.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/cli_routing.rs
-->

---
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


<!-- @trace
source: run-log-spec-include-quiz
updated: 2026-05-30
code:
  - codebus-cli/src/main.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
-->
<!-- @trace
source: goals-persistent-quickstart-examples
updated: 2026-06-03
code:
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
-->

---
### Requirement: New Goal Modal Flow

The system SHALL render the `New Goal` modal as a centered dialog with a single multi-line textarea (placeholder text "What should codebus document?") and two buttons labeled `Cancel` and `Run`. The modal SHALL be opened by clicking the Goals tab's `[+ New goal]` button or by clicking any pre-fill example row in the empty-state hint. The modal SHALL trap focus while open and SHALL close on `Esc` keypress, on `Cancel` click, or after a successful `Run` invocation.

Clicking `Run` SHALL invoke the `spawn_goal` Tauri command with the textarea text (trimmed) and the active vault path. On success, the modal SHALL close, the Goals overview list SHALL gain a new running-state row (synthesized client-side before the first `goal-stream` event arrives), AND the main content area SHALL switch to the `Running` detail view for the newly spawned run.

The `Run` button SHALL be disabled (visually and functionally) when any of the following hold: textarea contains only whitespace; another goal run is currently active in this vault (the `useGoalsStore.activeRun` state is non-null). When disabled due to an existing active run, the modal SHALL render a hint line below the textarea reading "Wait for current run to finish or cancel it before starting a new one."

#### Scenario: Submit empty modal text rejected

- **WHEN** the user opens the New Goal modal AND clicks `Run` without typing any text
- **THEN** the `Run` button SHALL be disabled AND no `spawn_goal` invocation SHALL occur

#### Scenario: Submit while another goal run is active

- **WHEN** the user opens the New Goal modal AND a previous goal run is still in the running state (`useGoalsStore.activeRun != null`)
- **THEN** the `Run` button SHALL be disabled AND the modal SHALL render the hint "Wait for current run to finish or cancel it before starting a new one." AND no `spawn_goal` invocation SHALL occur

#### Scenario: Successful Run transitions to Running detail

- **WHEN** the user types `"describe the auth flow"` AND clicks `Run` AND `spawn_goal` resolves with a new `RunId`
- **THEN** the modal closes AND the Goals overview list gains a row with outcome icon `⚪` AND goal text `"describe the auth flow"` AND the main content area switches to the `Running` detail view for that run


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
### Requirement: Run Detail Views — Running

The system SHALL render the `Running` detail view when the user navigates to a run whose state is the currently-active goal run (i.e., `useGoalsStore.activeRun.runId` equals the clicked run id and no RunLog row has been written yet for it). The view SHALL include: a header with `← back`, the goal text, an `⏺ Running` badge, AND an `[⏹ Cancel]` button placed inside the header on the right-hand side (immediately to the right of the badge AND to the left of the reserved `pr-[160px]` Windows traffic-light padding); a metadata line with elapsed time (live-updated every second) and accumulated token count from Usage events received so far; AND an `Activity stream` block rendering received events in arrival order. The view SHALL NOT render a separate bottom `<footer>` element for the Cancel button.

The metadata line's token-count slot SHALL NOT render the literal string `0` (nor `0 tokens`, nor any other literal-zero rendering) while the running run has produced no `StreamEvent::Usage` events yet. Instead, the slot SHALL render the localized translation of `workspace.runDetail.tokensRunningPlaceholder` (the placeholder MUST exist in every shipped locale, currently `en` AND `zh`). As soon as the first `StreamEvent::Usage` event is observed for the active run, the slot SHALL switch to rendering the real accumulated token count (input + output tokens summed across all Usage events received so far). The placeholder semantic is "no Usage event has arrived yet" — it is NOT a generic loading affordance and SHALL NOT be shown after a non-zero accumulated sum has been displayed (i.e., the slot SHALL NOT flicker back to the placeholder if a subsequent stream tick re-evaluates the sum mid-run).

The Activity stream SHALL render `StreamEvent::ToolUse { name, input }` events as one-line summaries with an emoji leader matching the CLI convention (`render::stream_event` `ToolUse Write/Edit specialization`):

- `ToolUse { name: "Write" | "Edit" }` SHALL render as `✍️ <file_path>` where `<file_path>` is the value of `input.file_path` normalized to forward slashes (e.g., `wiki/modules/auth.md`). The `input` dict shape SHALL NOT leak — only the path renders.
- `ToolUse { other }` SHALL render as `🛠️ <name>[ · <input-summary>]` where the input summary follows the existing abbreviation rules (file_path → file basename; pattern → quoted string; command → first 80 chars).

`StreamEvent::Thought { text }` events SHALL render inline within the Activity stream timeline (NOT buffered to a separate trailing block). Consecutive Thought events SHALL be folded into a single `🤔 <text>` item — the renderer SHALL maintain a running text buffer that flushes when any non-Thought event is observed AND emits one ThoughtItem per fold boundary. When the folded text contains a single line, the ThoughtItem SHALL render `🤔 <text>` on one line. When the folded text contains multiple lines, the ThoughtItem SHALL render `🤔 <first-line>` followed by a `(<N> more lines ▼)` toggle; clicking the toggle expands the remaining lines (indented) and reveals a `▲ collapse` control.

`StreamEvent::ToolResult` SHALL NOT render in this view (results are an internal flow signal — the GUI is a focused viewer, not a linear log). Deep-debug access to ToolResult bodies SHALL remain available via the Done detail's `Run details` collapsible block (which replays the full events.jsonl).

The `[⏹ Cancel]` button SHALL carry `data-testid="cancel-button"`. The button's wrapper element inside the header SHALL NOT carry the `data-tauri-drag-region` attribute (so window-drag pointer handlers do not swallow the button's click). Clicking `[⏹ Cancel]` SHALL invoke `cancel_goal(run_id)`. The button SHALL transition to a `Cancelling…` disabled state immediately upon click AND SHALL be replaced once the run transitions to a terminal state (cancelled / done / failed).

#### Scenario: Activity stream renders tool_use with emoji leaders

- **WHEN** the Running detail view receives two `goal-stream` events: `StreamEvent::ToolUse { name: "Read", input: { file_path: "raw/code/auth.rs" } }` then `StreamEvent::ToolUse { name: "Glob", input: { pattern: "wiki/**/*.md" } }`
- **THEN** the Activity stream block SHALL contain exactly two rendered rows in arrival order AND the first row contains `🛠️`, `Read`, AND `auth.rs` AND the second row contains `🛠️`, `Glob`, AND `wiki/**/*.md`

#### Scenario: ToolUse Write specialization renders only the file path

- **WHEN** the Running detail view receives `StreamEvent::ToolUse { name: "Write", input: { file_path: "wiki/modules/auth.md" } }`
- **THEN** the rendered row SHALL contain `✍️` AND `wiki/modules/auth.md` AND SHALL NOT contain the substring `Write` (the emoji conveys the tool) AND SHALL NOT contain `input` / `file_path` dict labels

#### Scenario: Thought chunks fold inline into a single timeline item

- **WHEN** the Running detail view receives `ToolUse(Read)`, then three sequential `StreamEvent::Thought` events with texts `"Analyzing "`, `"the auth "`, `"middleware..."`, then `ToolUse(Glob)`
- **THEN** the Activity stream renders three timeline items in order: a ToolUse row for Read, a single ThoughtItem rendering `🤔 Analyzing the auth middleware...` (the concatenation of the three Thought chunks), then a ToolUse row for Glob

#### Scenario: Multi-line Thought renders first line plus collapsible toggle

- **WHEN** the Running detail view receives `StreamEvent::Thought { text: "first line\nsecond\nthird" }`
- **THEN** the rendered ThoughtItem initially shows `🤔 first line` and a `(2 more lines ▼)` toggle AND clicking the toggle reveals the indented remainder `second\nthird` and a `▲ collapse` control

#### Scenario: Cancel button invokes cancel_goal and disables

- **WHEN** the user clicks `[⏹ Cancel]` in the Running detail view for run id `X`
- **THEN** `cancel_goal("X")` SHALL be invoked AND the button SHALL transition to a `Cancelling…` disabled state AND the button SHALL NOT be clickable a second time

#### Scenario: Cancel button renders inside header on the right

- **WHEN** the user navigates to the Running detail view for an active run
- **THEN** the element with `data-testid="cancel-button"` SHALL be a descendant of the view's `<header>` element AND SHALL appear in document order after the element with `data-testid="running-badge"` AND the cancel button's nearest ancestor element with `data-tauri-drag-region` (if any) SHALL be a different element from the cancel button's immediate wrapper (i.e., the cancel button's immediate wrapper SHALL NOT itself carry `data-tauri-drag-region`) AND the Running detail view SHALL NOT contain a `<footer>` descendant that wraps the cancel button

#### Scenario: Token slot renders placeholder before first Usage event

- **WHEN** the user navigates to the Running detail view for an active goal run AND zero `StreamEvent::Usage` events have been observed for that run since the spawn started
- **THEN** the metadata line's token-count slot SHALL render the localized translation of `workspace.runDetail.tokensRunningPlaceholder` AND SHALL NOT contain the substring `0 tokens` AND SHALL NOT contain a bare numeric `0` followed by a token-count label

#### Scenario: Token slot switches to real count after first Usage event

- **WHEN** the Running detail view receives its first `StreamEvent::Usage { input_tokens: 120, output_tokens: 80 }` event for the active run
- **THEN** the metadata line's token-count slot SHALL render an integer rendering of `200` (input + output) AND SHALL NOT continue to render the placeholder string

##### Example: token slot transitions

| Stream history so far | Token slot rendering | Notes |
| --- | --- | --- |
| no Usage events | localized `tokensRunningPlaceholder` (e.g., `—` / `計算中…`) | initial state |
| one Usage event with input=120 output=80 | `200` (rendered with existing token-count formatter) | first real value |
| two Usage events totalling input=240 output=160 | `400` | accumulated sum |
| run ended (transitioned to Done view) | not applicable — Done view uses RunLog summary | not Running view |


<!-- @trace
source: chatwidget-pulse-and-goal-token-display
updated: 2026-05-28
code:
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-app/src/components/workspace/Workspace.tsx
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src/components/workspace/ChatWidget.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
tests:
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
-->

---
### Requirement: Run Detail Views — Done

The system SHALL render the `Done` detail view when the user navigates to a run whose corresponding `RunLog` row has `outcome="succeeded"`. The view SHALL include: a header with `← back`, the goal text, and a `✓ Done` badge; a metadata line with duration (finished_at − started_at), accumulated tokens, and commit sha (first 7 chars of the latest commit on `<vault>/.codebus/`); a `Covered pages` block listing pages affected by the run; a `Lint` line summarizing `lint_error_count` and `lint_warn_count`; an `Activity summary` block summarizing tool-use counts derived from the events file; AND a collapsible `Run details` block (collapsed by default) rendering the full events.jsonl replay using the same `ActivityStreamItem` view as the Running detail (Thought events render inline as ThoughtItems per the Running detail's fold rules — there is no separate "Thinking" block, since the inline timeline already exposes them).

The `Covered pages` list SHALL be derived from the events.jsonl tail-replay by collecting unique `file_path` values from `StreamEvent::ToolUse { name: "Write" | "Edit", input.file_path }` events that resolve under the vault's `wiki/` directory. Each covered page SHALL render as a clickable `[[wikilink]]` row; clicking it SHALL switch the Workspace to the `Wiki` tab with that page loaded.

The `Covered pages` list AND the `Activity summary` block SHALL be grouped by verb phase. A "phase" is bounded by the `VerbLifecycleEvent::SpawnStart { verb }` … `VerbLifecycleEvent::SpawnEnd { verb }` event pair — every `StreamEvent::ToolUse` observed between those markers belongs to that phase. Typical phases for a goal run are `goal` (the goal agent itself) and `fix` (the post-spawn lint-and-fix agent invoked from the goal verb's fix loop). When the same verb runs multiple times in a single goal run (e.g., the fix loop iterates twice), the corresponding tool-use counts SHALL be merged under one `fix` heading (one bucket per `verb`, not per spawn).

The `Activity summary` block SHALL render one labelled sub-section per phase observed in the events file, ordered by first appearance. Each sub-section SHALL contain one line per tool name observed in that phase (e.g., `12 Read`, `8 Write`). Tools that did not fire in a phase SHALL NOT render a row inside that phase's section. When a phase produced zero ToolUse events (e.g., goal agent decided not to ingest), the phase SHALL still render its heading with an em-dash or short hint line indicating no tools were used.

The `Covered pages` block SHALL likewise be grouped by phase. Each covered page row SHALL appear under the phase whose ToolUse Write/Edit produced it. Slug uniqueness is enforced across phases — if the same slug is written by both `goal` and `fix` phases (rare), the later phase wins for display ordering but both phase headings still render the row.

The `Run details` block SHALL be collapsed by default with a `Show run details ▼` / `Hide run details ▲` toggle. When expanded, it SHALL render the events.jsonl replay using the same `ActivityStreamItem` + ThoughtItem fold as the Running detail, in arrival order. This block recovers the "Stream history" surface previously deprecated by the design — collapsed-by-default keeps the Done view minimal for the common "verify outcome" case while still giving deep-debug access to the full timeline (including Thoughts inline at the moment they fired).

#### Scenario: Done detail lists covered pages from events

- **WHEN** the user navigates to a `succeeded` run whose events.jsonl contains `ToolUse { name: "Write", input.file_path: "wiki/modules/auth.md" }` and `ToolUse { name: "Edit", input.file_path: "wiki/index.md" }`
- **THEN** the Done detail's `Covered pages` block lists exactly two rows: `[[auth]]` and `[[index]]` AND each row is clickable

#### Scenario: Done detail covered-page click switches to Wiki tab

- **WHEN** the user clicks a covered-page `[[slug]]` row in the Done detail view
- **THEN** the Workspace switches the active tab to `Wiki` AND `useWikiStore.currentPath` is set to that page's path AND the Milkdown preview renders that page

#### Scenario: Activity summary groups tool counts by verb phase

- **WHEN** the user navigates to a `succeeded` run whose events.jsonl contains: `Lifecycle::SpawnStart { verb: "goal" }`, then 12 `ToolUse { name: "Read" }`, then `Lifecycle::SpawnEnd { verb: "goal" }`, then `Lifecycle::SpawnStart { verb: "fix" }`, then 2 `ToolUse { name: "Bash" }` + 2 `ToolUse { name: "Write" }`, then `Lifecycle::SpawnEnd { verb: "fix" }`
- **THEN** the `Activity summary` block renders two phase sub-sections — `goal` containing one line `12 Read`, AND `fix` containing two lines `2 Bash` AND `2 Write` — AND no `Write` row appears under the `goal` phase even though the fix phase wrote pages

#### Scenario: Activity summary phase with zero tool uses renders empty hint

- **WHEN** the user navigates to a `succeeded` run whose events.jsonl contains `Lifecycle::SpawnStart { verb: "goal" }` immediately followed by `Lifecycle::SpawnEnd { verb: "goal" }` (goal agent ran but invoked no tools — e.g., judged the goal out-of-scope)
- **THEN** the `Activity summary` block still renders the `goal` phase heading AND the body of that phase contains an em-dash or short hint line ("(no tools used)") AND no `ToolUse` row appears under it

#### Scenario: Covered pages groups slugs by writing phase

- **WHEN** the user navigates to a `succeeded` run whose `goal` phase wrote `wiki/modules/auth.md` AND whose `fix` phase wrote `wiki/index.md` AND `wiki/log.md`
- **THEN** the `Covered pages` block renders two phase sub-sections — `goal` containing one `[[auth]]` row AND `fix` containing two rows `[[index]]` AND `[[log]]` — AND each row remains clickable to switch to the Wiki tab

#### Scenario: Run details block is collapsed by default and replays full timeline on expand

- **WHEN** the user navigates to a `succeeded` run AND the `Run details` toggle is in the default collapsed state
- **THEN** the timeline is NOT visible AND clicking `Show run details ▼` SHALL render every event in events.jsonl using the same ActivityStreamItem + ThoughtItem fold as the Running detail, in arrival order (Thought chunks folded inline at their original position)


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
### Requirement: Run Detail Views — Cancelled and Interrupted

The system SHALL render a single `RunDetailInterrupted` component for any terminal run whose `RunLog` outcome is `"cancelled"`, `"failed"`, or `"interrupted"`. The component file SHALL be `codebus-app/src/components/workspace/RunDetailInterrupted.tsx` (renamed from `RunDetailCancelled.tsx`); the previous `RunDetailCancelled` component SHALL NOT exist as a separate export.

The component SHALL implement an explicit two-stage state machine driven by two inputs:

1. **Banner tier** (color and visual language), derived from `RunLog.outcome`:
   - `outcome === "failed"` → banner tier `"red"` (Failed visual language).
   - `outcome === "cancelled"` OR `outcome === "interrupted"` → banner tier `"amber"` (Interrupted visual language).
2. **Reason sub-variant** (banner subtitle copy), derived from `RunLog.interrupt_reason`:
   - When `interrupt_reason === "app-close"` → subtitle key `workspace.runDetail.banner.reason.appClose`.
   - When `interrupt_reason === "user-cancel"` → subtitle key `workspace.runDetail.banner.reason.userCancel`.
   - When `interrupt_reason === "network-drop"` → subtitle key `workspace.runDetail.banner.reason.networkDrop`.
   - When `interrupt_reason` is the `{ other: string }` variant → subtitle key `workspace.runDetail.banner.reason.other`. The free-form `other` string SHALL NOT be rendered into the UI text.
   - When `interrupt_reason` is `undefined` (legacy RunLog) AND `outcome === "cancelled" | "interrupted"` → subtitle key `workspace.runDetail.banner.interruptedSubtitle` (generic amber fallback).
   - When banner tier is `"red"` (`outcome === "failed"`) → title key `workspace.runDetail.banner.failedTitle` AND subtitle key `workspace.runDetail.banner.failedSubtitle` (the failed branch ignores `interrupt_reason`).

The component SHALL include: a header with `← back`, the goal text, and a status badge whose icon and tier follow the banner tier above; a banner block with title + subtitle keyed per the state machine above; a `Partial timeline` section summarizing tool_use events grouped by category (reading / writing / other); and a `[Retry with same goal]` button.

The `[Retry with same goal]` button SHALL extract the goal text from the run's RunLog row (when present) or the events.jsonl first user-prompt event (for virtual interrupted entries with no RunLog row), pre-fill the New Goal modal with that text, and open the modal. The user SHALL still confirm the run by clicking `Run` in the modal — Retry SHALL NOT spawn a new goal directly.

Routing in `codebus-app/src/components/workspace/Workspace.tsx` SHALL dispatch every non-running terminal outcome that is not `"succeeded"` to `RunDetailInterrupted`. The previous outcome switch case for `RunDetailCancelled` SHALL be removed.

The i18n keys `workspace.runDetail.cancelledBadge`, `workspace.runDetail.cancelledWarning`, `workspace.runDetail.interruptedBadge`, `workspace.runDetail.interruptedWarning`, and `workspace.runDetail.retryButton` SHALL remain registered with their existing key names; the system SHALL NOT rename them. New banner keys SHALL be added without removing the legacy keys.

#### Scenario: Cancelled run renders amber banner with userCancel subtitle

- **WHEN** the user navigates to a run with `RunLog.outcome === "cancelled"` AND `RunLog.interrupt_reason === "user-cancel"`
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner uses the amber tier AND the banner subtitle is sourced from the i18n key `workspace.runDetail.banner.reason.userCancel`

#### Scenario: Failed run renders red banner

- **WHEN** the user navigates to a run with `RunLog.outcome === "failed"`
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner uses the red tier AND the banner title is sourced from `workspace.runDetail.banner.failedTitle` AND the banner subtitle is sourced from `workspace.runDetail.banner.failedSubtitle` AND the banner SHALL NOT use any `reason.*` subtitle key

#### Scenario: Interrupted virtual entry with app-close reason renders appClose subtitle

- **WHEN** the vault contains `events-2026-05-13T03-00-00Z.jsonl` AND no corresponding RunLog row exists for `started_at === "2026-05-13T03:00:00Z"` AND the synthesized virtual entry carries `interrupt_reason === "app-close"`
- **THEN** the Goals overview list contains a virtual entry with `⚠` icon AND clicking it navigates to the `RunDetailInterrupted` view AND the banner uses the amber tier AND the banner subtitle is sourced from `workspace.runDetail.banner.reason.appClose`

#### Scenario: Legacy cancelled run without interrupt_reason falls back to generic amber subtitle

- **WHEN** the user navigates to a legacy run with `RunLog.outcome === "cancelled"` AND `RunLog.interrupt_reason === undefined`
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner uses the amber tier AND the banner subtitle is sourced from `workspace.runDetail.banner.interruptedSubtitle` AND no JavaScript error is thrown

#### Scenario: Unknown interrupt_reason maps to generic reason subtitle

- **WHEN** the user navigates to a run whose `RunLog.interrupt_reason` deserializes to the `{ other: "agent-crash" }` variant
- **THEN** the detail view renders `RunDetailInterrupted` AND the banner subtitle is sourced from `workspace.runDetail.banner.reason.other` AND the raw `"agent-crash"` string SHALL NOT be rendered into the UI text

#### Scenario: Retry pre-fills modal without spawning across all three terminal outcomes

- **WHEN** the user clicks `[Retry with same goal]` in `RunDetailInterrupted` for a run with `goal === "describe auth flow"` AND the run's outcome is any of `"cancelled" | "failed" | "interrupted"`
- **THEN** the New Goal modal opens AND the textarea contains exactly the text `"describe auth flow"` AND no `spawn_goal` IPC invocation occurs until the user clicks `Run` in the modal

##### Example: state machine inputs to banner outputs

| outcome     | interrupt_reason            | banner tier | title key              | subtitle key                                  |
| ----------- | --------------------------- | ----------- | ---------------------- | --------------------------------------------- |
| cancelled   | undefined                   | amber       | interruptedTitle       | interruptedSubtitle                           |
| cancelled   | "user-cancel"               | amber       | interruptedTitle       | reason.userCancel                             |
| failed      | undefined                   | red         | failedTitle            | failedSubtitle                                |
| failed      | "user-cancel"               | red         | failedTitle            | failedSubtitle (reason ignored on red tier)   |
| interrupted | "app-close"                 | amber       | interruptedTitle       | reason.appClose                               |
| interrupted | "network-drop"              | amber       | interruptedTitle       | reason.networkDrop                            |
| interrupted | { other: "agent-crash" }    | amber       | interruptedTitle       | reason.other                                  |
| interrupted | undefined                   | amber       | interruptedTitle       | interruptedSubtitle                           |


<!-- @trace
source: interrupted-state-formalize
updated: 2026-05-27
code:
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-core/src/log/sinks/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailInterrupted.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/log/sink.rs
  - codebus-core/src/log/mod.rs
  - codebus-core/src/log/sinks/null_sink.rs
  - codebus-app/design-handoff/AUDIT.md
  - codebus-core/src/verb/chat.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-core/src/verb/query.rs
  - codebus-core/src/log/verb_log.rs
  - codebus-app/src/components/workspace/Workspace.tsx
tests:
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/RunDetailInterrupted.test.tsx
-->

---
### Requirement: Wiki Tab with Collapsible File Tree

The `Wiki` tab main content area SHALL render a Milkdown editor in read-only mode displaying the markdown body of the currently-selected wiki page. A collapsible `Pages` file tree panel SHALL be rendered as a left-side column that is expanded by default; clicking a folder icon button in the Wiki tab top bar SHALL toggle the panel's visibility. When expanded, the panel SHALL list all wiki pages in `useWikiStore.pages` grouped by taxonomy folder (concepts / entities / modules / processes / synthesis) with the file basename as the row label. Clicking a row SHALL set `useWikiStore.currentPath` to that page's path and load its body via `read_wiki_page`.

The Wiki tab top bar SHALL also display the currently-selected page's title (from frontmatter `title` or the file basename if no frontmatter title).

When the vault has zero wiki pages, the main content area SHALL render a centered hint reading "No wiki pages yet — run a goal to start documenting".

#### Scenario: Wiki tab opens with file tree expanded

- **WHEN** the user clicks the `Wiki` tab for the first time after opening a vault
- **THEN** the `Pages` file tree panel IS visible on the left AND lists all pages grouped by taxonomy folder AND the Milkdown preview occupies the remaining width

#### Scenario: File tree toggle collapses the panel

- **WHEN** the user clicks the folder icon button in the Wiki tab top bar while the tree is expanded
- **THEN** the `Pages` file tree panel hides AND the Milkdown preview expands to occupy the full width of the main content area

#### Scenario: Empty vault shows wiki hint

- **WHEN** the user opens the `Wiki` tab in a vault that has no `wiki/**/*.md` files
- **THEN** the main content area renders the centered hint "No wiki pages yet — run a goal to start documenting"


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
### Requirement: Wikilink Resolution and Click Behavior

The Milkdown preview SHALL render `[[page-slug]]` syntax via a custom ProseMirror node provided by a wikilink plugin. The plugin SHALL parse `[[slug]]` in markdown content (paste rule + input rule), look up `slug` in `useWikiStore.pages` (key = filename basename without `.md` extension), and render the link in one of two states:

- Resolvable (slug exists in pages map): rendered as a colored clickable link; clicking SHALL invoke `useWikiStore.loadPage(slug)` and update `currentPath`
- Unresolvable (slug not in pages map): rendered as a dimmed disabled-style span; hover SHALL display a tooltip reading "Page not found"; clicking SHALL be a no-op

Wikilink resolution SHALL be entirely client-side using the page index loaded at Workspace mount time. The system SHALL NOT issue an IPC call per wikilink click — `read_wiki_page` is invoked only when the navigation lands on a resolvable target.

When two or more pages share the same slug (filename collision across taxonomy folders), the last entry inserted into `useWikiStore.pages` SHALL win; this matches the vault's existing slug-uniqueness expectation enforced by the wiki lint `duplicate-slug` rule.

#### Scenario: Resolvable wikilink navigates to page

- **WHEN** the Milkdown preview contains `[[uv-lib]]` AND `useWikiStore.pages["uv-lib"]` exists AND the user clicks the link
- **THEN** `useWikiStore.currentPath` updates to the resolved page path AND the Milkdown preview re-renders with that page's body

#### Scenario: Unresolvable wikilink renders disabled and tooltip

- **WHEN** the Milkdown preview contains `[[nonexistent-page]]` AND `useWikiStore.pages["nonexistent-page"]` does not exist
- **THEN** the link renders with a dimmed visual style AND hovering the link displays a tooltip with the text "Page not found" AND clicking the link is a no-op (no IPC invocation, no navigation)

#### Scenario: Wikilink resolution is client-side only

- **WHEN** the user clicks any wikilink in the Milkdown preview
- **THEN** the resolution SHALL look up the slug in `useWikiStore.pages` in memory AND SHALL NOT issue a `list_wiki_pages` or other page-index IPC call


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
### Requirement: Tauri IPC Commands for Goal Lifecycle and Wiki Read

The system SHALL register Tauri commands beyond the foundation's nine commands, covering goal-mode lifecycle, chat-turn lifecycle, and wiki read paths. The full added set is:

- `spawn_goal(vault_path: String, goal_text: String) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::goal::run_goal` with the given vault and goal text. The function SHALL sample the wall-clock time EXACTLY ONCE via `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` and derive from that single sample BOTH the `RunId` slug (the RFC 3339 string with `:` replaced by `-`) AND the colon-form RFC 3339 string passed down into `run_goal` as its `run_started_at` argument. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the `RunId` slug, return the `RunId` slug to the caller, AND emit each `VerbEvent` produced by the closure to a Tauri event channel named `"goal-stream"` with payload `{ run_id: String, event: VerbEvent }` carrying that same `RunId` slug. The `goal-terminal` payload SHALL carry that same `RunId` slug. Because the events.jsonl filename slug and `RunLog.started_at` written by `run_goal` derive from the SAME single sample, the `RunId` returned to the frontend SHALL (after slugging) be byte-equal to the persisted run's identity, so a completed run's `get_run_detail` lookup by that `RunId` SHALL succeed. On thread completion (success, failure, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_goal(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (run already terminated), return `Ok(())` (idempotent).

- `list_runs(vault_path: String, mode_filter: ModeFilter) -> Result<Vec<RunLogSummary>, AppError>` — read all `runs-*.jsonl` files under `<vault>/.codebus/log/`, parse each row to `RunLogSummary`, apply `mode_filter` (`Goal` keeps only `mode=="goal"`; `All` keeps everything), then scan `events-*.jsonl` files for interrupted detection per the next requirement, merge virtual entries, and return the combined list sorted by `started_at` descending.

- `get_run_detail(vault_path: String, run_id: String) -> Result<RunDetail, AppError>` — find the matching `RunLogSummary` (real or virtual interrupted), open the corresponding `events-*.jsonl`, replay all events into `Vec<RecordedEvent>`, and return `RunDetail { summary, events }`.

- `list_wiki_pages(vault_path: String) -> Result<Vec<WikiPageMeta>, AppError>` — glob `<vault>/.codebus/wiki/**/*.md`, parse each file's frontmatter to extract `title`, derive slug from the filename (without `.md`), and return one `WikiPageMeta { slug, path, title }` per file. Files without parseable frontmatter SHALL still be returned with `title` equal to the slug.

- `read_wiki_page(vault_path: String, page_slug: String) -> Result<String, AppError>` — look up the page by slug among the wiki files, read its raw bytes, strip the leading frontmatter block (delimited by `---\n...\n---\n` at the start), and return the remaining markdown body as a `String`. If the slug does not match any wiki file, return `AppError::Invalid { field: "page_slug", message: "no such page" }`.

The chat-turn lifecycle commands (`spawn_chat_turn`, `cancel_chat_turn`) are defined separately under `Tauri IPC Commands for Chat Turn Lifecycle` and SHALL coexist with the above in `codebus-app/src-tauri/src/ipc/mod.rs` registration.

`ModeFilter` SHALL be a serde-tagged enum with variants `Goal` and `All` (snake_case).

`AppError` SHALL be the same discriminated union used by the foundation's commands — no new variants added by this change.

The goal `RunId` SHALL be sampled EXACTLY ONCE in the IPC layer at `chrono::SecondsFormat::Millis` precision and threaded down into `run_goal` (as `run_started_at`), so that (a) two `spawn_goal` invocations within the same wall-clock second receive distinct `active_runs` keys, AND (b) the `active_runs` key, the `RunId` returned to the frontend, the events.jsonl filename slug, and the `RunLog.started_at` value all originate from that single sample and are therefore byte-identical strings — never two independent `Utc::now()` samples that can drift apart.

#### Scenario: spawn_goal returns run id derived from a single IPC sample

- **WHEN** the frontend calls `invoke("spawn_goal", { vault_path: "/some/vault", goal_text: "X" })` AND the IPC layer samples the wall-clock once at `2026-05-13T14:56:21.123Z`
- **THEN** the IPC call resolves with `"2026-05-13T14-56-21.123Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string AND the colon-form `"2026-05-13T14:56:21.123Z"` is passed to `run_goal` as `run_started_at`

#### Scenario: spawn_goal same-second calls yield distinct RunIds

- **WHEN** the frontend calls `invoke("spawn_goal", ...)` twice in rapid succession AND both calls land within the same wall-clock second but on distinct wall-clock milliseconds
- **THEN** the two IPC calls SHALL resolve with two distinct `RunId` strings differing in the `.fff` fractional component AND `AppState.active_runs` SHALL contain two entries simultaneously, each with its own cancel handle

#### Scenario: spawn_goal RunId resolves via get_run_detail after the run terminates

- **WHEN** the frontend calls `invoke("spawn_goal", ...)` and receives `RunId` `R` AND the spawned `run_goal` runs to a terminal outcome (`succeeded` or `failed`), writing its `events-*.jsonl` file and `RunLog` row to disk
- **THEN** a subsequent `invoke("get_run_detail", { vault_path, run_id: R })` SHALL resolve with the `RunDetail` for that run (the persisted run's slug equals `R`) AND SHALL NOT return `AppError::Invalid { field: "run_id" }`

#### Scenario: cancel_goal idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_goal", { run_id: "nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

#### Scenario: list_runs filters by mode

- **WHEN** the frontend calls `invoke("list_runs", { vault_path: ..., mode_filter: { kind: "goal" } })` AND the vault's `runs-*.jsonl` contain three goal rows, two chat rows, and one fix row
- **THEN** the returned `Vec<RunLogSummary>` length is 3 AND every entry has `mode == "goal"`

#### Scenario: read_wiki_page strips frontmatter

- **WHEN** the frontend calls `invoke("read_wiki_page", { vault_path: ..., page_slug: "uv-lib" })` AND the file at `<vault>/.codebus/wiki/modules/uv-lib.md` contains a frontmatter block followed by markdown body
- **THEN** the IPC returns the markdown body string without the leading `---\n...\n---\n` block


<!-- @trace
source: backend-cleanup-codex-websearch-and-runid-millis, goal-run-id-unify-stuck-rundetail
updated: 2026-06-07
code:
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/goal.rs
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src-tauri/src/ipc/chats.rs
  - codebus-core/src/agent/codex_backend.rs
  - docs/2026-05-28-run-id-collision-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-28-runid-source-of-truth-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/src-tauri/src/ipc/goals.rs
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
  - docs/2026-05-28-cancelling-stuck-todo.md
  - codebus-core/src/verb/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->

---
### Requirement: Interrupted Run Detection

The system SHALL detect interrupted goal runs by comparing `events-*.jsonl` files against `runs-*.jsonl` rows at workspace mount time (via the `list_runs` IPC). For each `events-<started_at_slug>.jsonl` file present under `<vault>/.codebus/log/`, the system SHALL search the `runs-*.jsonl` files for a row whose `started_at` (slugged identically) matches.

When no matching `runs-*.jsonl` row exists AND the events file is identifiable as a goal-mode run (one of its leading events is a `VerbBanner::Goal` event), the system SHALL surface a virtual `RunLogSummary` whose `outcome` field SHALL be determined by whether the run is still alive: if the slug is currently present in the process-wide `active_runs` map (the in-memory cancel-flag registry maintained by `spawn_goal` / cleanup), `outcome` SHALL be `"running"` and `interrupt_reason` SHALL be absent; otherwise `outcome` SHALL be `"interrupted"` and `interrupt_reason` SHALL be `AppClose`. In both cases the synthesized entry SHALL have `started_at` derived from the slug, `goal` extracted from the `VerbBanner::Goal` event, `mode="goal"`, and an empty `finished_at`. This dual projection prevents the GUI from misreading an in-flight goal (whose terminal RunLog has not yet been written) as interrupted just because the user navigated to the Lobby and back.

When an orphan events file is NOT identifiable as a goal-mode run (no `VerbBanner::Goal` event among its leading events), the system SHALL NOT surface any virtual entry for it, and that events file SHALL NOT contribute any row to the `list_runs` response. This prevents in-progress or interrupted `chat` / `query` / `fix` / `quiz` runs — whose `events-*.jsonl` file exists before their terminal `runs-*.jsonl` row is written — from transiently appearing in the Goals list with empty goal text.

The virtual entry SHALL NOT be written back to any `runs-*.jsonl` file — it exists only in the IPC response. Subsequent re-invocations of `list_runs` SHALL re-derive the virtual entry from the same on-disk state plus the current `active_runs` snapshot.

If the same events file later gains a matching RunLog row (e.g., because the original `run_goal` process recovered and wrote its terminal RunLog late), the virtual entry SHALL no longer appear in `list_runs` output — the real row supersedes it.

**NOTE — Single-Source Run Id Invariant:** The `active_runs` map key, the `RunId` returned to the frontend, the `events-<slug>.jsonl` filename slug, AND the `RunLog.started_at` value persisted in `runs-*.jsonl` SHALL all originate from a SINGLE `chrono::Utc::now()` sample taken once in the IPC layer (`spawn_goal` / `spawn_chat_turn`) and threaded down into the verb function (e.g. the `run_started_at` argument of `run_goal`). Deriving the verb-side slug from an INDEPENDENT second `Utc::now()` sample is FORBIDDEN: equal `SecondsFormat` precision does NOT guarantee equal values across two samples taken at different instants, so the orphan-detection join in `list_runs` (which joins these values as strings) would silently miss — a live goal would be mis-labeled `"interrupted"` AND a completed run's detail would be unreachable by the frontend's `RunId` (a permanent loading state). The CLI path, which has no cross-layer id join, MAY let the verb function sample internally (passing `run_started_at = None`). This invariant is enforced by a regression test asserting that the `RunId` returned by `spawn_goal` resolves via `get_run_detail` / `list_runs` after the verb writes its terminal RunLog.

#### Scenario: Orphan goal events file with no active_runs entry produces interrupted virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-13T03-00-00.000Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="describe auth flow"` AND no `runs-*.jsonl` row has `started_at == "2026-05-13T03:00:00.000Z"` AND `active_runs` does NOT contain the slug
- **THEN** the returned list contains a virtual entry with `outcome == "interrupted"` AND `mode == "goal"` AND `goal == "describe auth flow"` AND `started_at == "2026-05-13T03:00:00.000Z"` AND `interrupt_reason == "app_close"` AND no row is appended to any `runs-*.jsonl` file on disk

#### Scenario: Orphan goal events file with live active_runs entry produces running virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-28T07-39-26.123Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="smoke probe goal"` AND no `runs-*.jsonl` row matches its slug AND `active_runs` currently contains an entry keyed by `"2026-05-28T07-39-26.123Z"` (the in-flight spawn from the current app session, whose key is the same single IPC sample as the events file slug)
- **THEN** the returned list contains a virtual entry with `outcome == "running"` AND `mode == "goal"` AND `goal == "smoke probe goal"` AND `started_at == "2026-05-28T07:39:26.123Z"` AND `interrupt_reason` absent AND `finished_at` empty

#### Scenario: Single-source run id keeps active_runs key, events slug, and RunLog.started_at byte-equal

- **WHEN** `spawn_goal` samples the wall-clock once as `2026-05-28T09:50:42.322Z` AND threads the colon form into `run_goal` as `run_started_at` AND the run writes `events-2026-05-28T09-50-42.322Z.jsonl` plus a `RunLog` row with `started_at == "2026-05-28T09:50:42.322Z"`
- **THEN** the `active_runs` key, the `RunId` returned to the frontend, the events-file slug, and the slugged `RunLog.started_at` SHALL all be the byte-identical string `"2026-05-28T09-50-42.322Z"` AND `list_runs` SHALL join them without a miss (the run shows `"running"` while live and is reachable by `get_run_detail` once terminal — never a spurious `"interrupted"` or an unresolvable `RunId`)


<!-- @trace
source: backend-cleanup-codex-websearch-and-runid-millis, goal-run-id-unify-stuck-rundetail
updated: 2026-06-07
code:
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/goal.rs
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src-tauri/src/ipc/chats.rs
  - codebus-core/src/agent/codex_backend.rs
  - docs/2026-05-28-run-id-collision-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-28-runid-source-of-truth-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/src-tauri/src/ipc/goals.rs
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
  - docs/2026-05-28-cancelling-stuck-todo.md
  - codebus-core/src/verb/query.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->

---
### Requirement: One Active Goal Run At A Time

The system SHALL enforce that at most one goal-mode `run_goal` invocation is active per vault per app session. This invariant SHALL be enforced at two layers:

- Frontend (`useGoalsStore`): exposes an `activeRun` field that is non-null when a spawn is in progress; New Goal modal `Run` button is disabled while `activeRun != null` (per the `New Goal Modal Flow` requirement)
- Backend (`AppState.active_runs`): `spawn_goal` SHALL return `AppError::Invalid { field: "active_runs", message: "another goal run is already active" }` when invoked while `active_runs` contains an entry keyed by a goal-mode `RunId` (slug pattern matching the `started_at` form WITHOUT a `chat-` or other-mode prefix). Chat-mode entries (keyed `chat-<slug>`) SHALL NOT block goal spawn; chat-mode and goal-mode runs can coexist in `active_runs` simultaneously because chat is read-only (`CHAT_TOOLSET` excludes `Write`/`Edit`) and cannot conflict with concurrent goal-mode writes.

When the Chat Widget's `[Promote to goal]` flow invokes `spawn_goal` AND a goal-mode entry already exists in `active_runs`, the call SHALL be rejected with the same `AppError::Invalid` as above; the Chat Widget SHALL surface this rejection per the `Promote Suggestion Inline Pill and Spawn-Goal Flow` requirement.

This invariant applies per app session within a single vault; switching vaults (back to lobby then opening a different vault) does not carry the constraint across.

#### Scenario: Second spawn_goal during active run rejected at backend

- **WHEN** a goal run is currently active for vault `V` (an entry exists in `active_runs` keyed by a goal-mode RunId) AND the frontend invokes `spawn_goal` with the same vault
- **THEN** the IPC call rejects with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring "already active"

#### Scenario: Spawn allowed after cancel completes

- **WHEN** a goal run is active AND `cancel_goal` is invoked AND the background thread observes the flag, kills the child, removes the entry from `active_runs`, AND emits a final `goal-stream` event signaling termination
- **THEN** a subsequent `spawn_goal` invocation succeeds AND a new run id is returned

#### Scenario: Chat turn does not block concurrent goal spawn

- **WHEN** a chat turn is currently active for vault `V` (an entry exists in `active_runs` keyed `chat-<slug>`) AND no goal-mode entry exists AND the frontend invokes `spawn_goal` with the same vault
- **THEN** the IPC call resolves with `Ok(<new_run_id>)` AND `active_runs` SHALL contain both the chat-mode entry AND the new goal-mode entry simultaneously

---
### Requirement: Cross-Vault Goal Spawn Permitted

The system SHALL permit a `spawn_goal` invocation against vault `V2` to succeed even when `active_runs` contains a goal-mode entry associated with a different vault `V1`, in accordance with the existing `One Active Goal Run At A Time` requirement statement "switching vaults (back to lobby then opening a different vault) does not carry the constraint across". The "per vault" qualifier in the same requirement SHALL be enforced by associating each `active_runs` entry with the vault path under which it was inserted, and SHALL be queried via a vault-scoped predicate when evaluating the pre-spawn guard for `spawn_goal`. Goal-mode entries inserted under vault `V1` SHALL NOT cause the pre-spawn guard for vault `V2` to reject.

The vault-scoped enforcement SHALL apply symmetrically to the chat and quiz mode pre-spawn guards: a chat turn or quiz run active under vault `V1` SHALL NOT block a chat turn or quiz run, respectively, against a different vault `V2`. Same-vault same-mode mutual exclusion (the existing scenarios under `One Active Goal Run At A Time`, `Chat Turn Lifecycle`, and any quiz double-spawn guard) SHALL remain unchanged.

#### Scenario: Cross-vault goal spawn allowed while another vault has an active goal

- **WHEN** a goal-mode entry exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_goal` with vault path `V2` where `V2 != V1`
- **THEN** the IPC call SHALL resolve with `Ok(<new_run_id>)` AND `active_runs` SHALL contain BOTH the prior goal-mode entry for `V1` AND the newly inserted goal-mode entry for `V2` simultaneously

#### Scenario: Same-vault same-mode exclusion preserved after vault scope landed

- **WHEN** a goal-mode entry already exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_goal` with the same vault path `V1`
- **THEN** the IPC call SHALL reject with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring "already active"

#### Scenario: Cross-vault chat spawn allowed

- **WHEN** a chat-mode entry (keyed with the `chat-` prefix) exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_chat_turn` with a different vault path `V2`
- **THEN** the IPC call SHALL resolve successfully AND `active_runs` SHALL contain BOTH chat-mode entries simultaneously, one under `V1` AND one under `V2`

#### Scenario: Cross-vault quiz spawn allowed

- **WHEN** a quiz-mode entry (keyed with the `quiz-` prefix) exists in `active_runs` associated with vault `V1` AND the frontend invokes `spawn_quiz_plan` or `spawn_quiz_generate` with a different vault path `V2`
- **THEN** the IPC call SHALL resolve successfully AND `active_runs` SHALL contain BOTH quiz-mode entries simultaneously, one under `V1` AND one under `V2`

<!-- @trace
source: vault-switch-goal-regression
updated: 2026-05-28
code:
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src-tauri/src/ipc/chats.rs
  - codebus-app/src-tauri/src/ipc/quiz.rs
tests:
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
-->


<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
code:
  - docs/2026-05-14-github-repo-setup-backlog.md
  - docs/2026-05-15-codebus-fs-watcher-backlog.md
  - codebus-app/src/components/workspace/ChatInput.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - docs/2026-05-14-rag-index-search-backlog.md
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/src/components/workspace/ChatUndoToast.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/2026-05-14-mcp-server-backlog.md
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-14-openai-privacy-filter-backlog.md
  - docs/2026-05-14-mycoder-cli-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - docs/2026-05-14-settings-chat-model-backlog.md
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - docs/2026-05-14-ui-accessibility-backlog.md
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - docs/2026-05-14-app-font-scale-backlog.md
  - docs/BACKLOG.md
  - codebus-app/src/store/chat.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/ChatWidget.tsx
tests:
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatInput.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->


<!-- @trace
source: vault-switch-goal-regression
updated: 2026-05-28
code:
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/store/goals.ts
tests:
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/store/goals.test.ts
-->

---
### Requirement: Chat Widget Layout and Two-State Toggle

The Workspace SHALL render a Chat Widget anchored to the bottom-right corner of the Workspace main content area (for the `bubble` and `floating` modes) or as a centered modal overlay (for the `modal` mode). The widget SHALL have exactly three visual modes, modeled in `useChatStore` by `mode: "bubble" | "floating" | "modal"` (replacing the previous `expanded: boolean` field) and `modalReturnMode: "bubble" | "floating" | null` (recording the mode the user came from when `mode === "modal"`, so Esc / backdrop click can restore it).

1. **Bubble mode**: a 44px × 44px circular bubble pinned to the viewport bottom-right corner. The bubble's right edge SHALL sit 16px from the viewport's right edge AND its bottom edge SHALL sit above the existing `BottomStrip` footer with a 16px gap (i.e., bottom offset equals `BottomStrip height + 16px`; with the current 32px-tall `BottomStrip` that is 48px from the viewport bottom). The bubble SHALL contain a `💬` emoji icon. The bubble SHALL remain visible whenever the Workspace is mounted AND SHALL NOT be obscured by any tab (Goals / Wiki / Quiz) content. When a `VerbLifecycleEvent::PromoteSuggestion` event is emitted while the widget is in bubble mode, the bubble SHALL display a small red dot badge (with `data-testid="chat-widget-promote-badge"`) until the next time the widget transitions to `floating` or `modal`. The bubble SHALL NOT render any indicator tied to `useGoalsStore.activeRun` — the active-goal ambient signal is rendered on the Goals tab sidebar row instead (see the `Workspace Sidebar Nav Row Visual Contract` requirement). The bubble SHALL NOT render any element with `data-testid="chat-widget-active-goal-pulse"` in any of the three modes.

2. **Floating mode**: a fixed-size panel of exactly `360px × 460px` positioned with its bottom-right corner aligned to the same anchor point as the bubble (right: 16px, bottom: `BottomStrip height + 16px`). The panel SHALL contain four vertically stacked regions: a header bar (containing the `💬` emoji, a localized title from `chat.widget.aria.floating.title`, a `⤢` expand-to-modal button with `data-testid="chat-widget-expand-to-modal"` whose `aria-label` resolves to `chat.widget.aria.floating.expandToModal`, AND a `▿` minimize button with `data-testid="chat-widget-minimize"` whose `aria-label` resolves to `chat.widget.aria.floating.minimize`), an undo toast region, a scrollable transcript region (containing past turns and the active turn live events), and an input region (containing a textarea and a send button, or a `⏹ Stop` button while a turn is active). The floating panel SHALL NOT be resizable; no resize handle SHALL be rendered. Pressing `Esc` while in floating mode SHALL NOT close the widget (the floating mode is "sticky"; the user must click the `▿` minimize button to return to bubble mode).

3. **Modal mode**: a centered modal dialog rendered through the project's existing `Dialog` primitive (radix-ui based, located at `codebus-app/src/components/ui/dialog.tsx`). The modal content SHALL have a width of `640px` AND a maximum height of `480px`. The modal SHALL be positioned at `60px` from the viewport top (not vertically centered; the visual weight sits above center). A backdrop SHALL be rendered behind the modal with `55%` black opacity AND a `2px` blur filter (subject to graceful fallback to plain 55% black when the host WebView2 engine cannot render the blur filter performantly). The modal SHALL contain the same four regions as floating mode (header, undo toast, transcript, input) except that the header right side SHALL contain a `⤡` dock-to-floating button with `data-testid="chat-widget-dock-to-floating"` whose `aria-label` resolves to `chat.widget.aria.modal.dockToFloating` AND a `✕` close button with `data-testid="chat-widget-modal-close"` whose `aria-label` resolves to `chat.widget.aria.modal.close`. When the modal opens, the input textarea SHALL receive focus automatically. While the modal is open, the underlying `Dialog` primitive SHALL trap keyboard focus inside the modal (tab and shift-tab cycle within the modal subtree, never reaching focusable elements behind the backdrop). When the modal closes, focus SHALL return to the element that was focused at the moment the modal opened (handled by the radix `Dialog` primitive).

All three modes SHALL share a single chat session via `useChatStore` (`sessionId`, `turns`, `activeTurn`, `tokensTotal`, `promoteSuggestion`, `onboardedVaults`, `lastTranscript`, `lastSessionId`). Switching between modes SHALL NOT reset, clear, or duplicate any session state.

The widget SHALL use logical pixel values for fixed dimensions (`44px`, `360px`, `460px`, `640px`, `480px`, `60px`) AND SHALL NOT expose any user-configurable size preference. Bubble and floating modes anchor to the viewport bottom-right corner; the widget SHALL NOT be draggable to any other position. Mode preference SHALL NOT be persisted: every Workspace mount SHALL initialize with `mode = "bubble"` AND `modalReturnMode = null`.

The bubble mode bubble's `aria-label` SHALL be the localized translation of `chat.widget.aria.openChat` regardless of the value of `useGoalsStore.activeRun`. The previous conditional aria-label that switched to `chat.widget.aria.openChatWithActiveGoalRunning` while a goal was running SHALL be removed — the active-goal signal is no longer announced on the chat bubble (it is announced via the Goals sidebar row's pulse dot aria-label sourced from `workspace.tab.goals.activeRunPulse`). The i18n key `chat.widget.aria.openChatWithActiveGoalRunning` SHALL be removed from every shipped locale bundle. The floating mode panel title SHALL render the localized translation of `chat.widget.aria.floating.title`; the modal mode dialog title SHALL render the localized translation of `chat.widget.aria.modal.title`. Both modal/floating title keys MUST exist in every shipped locale.

`useChatStore` SHALL expose the following actions in place of the removed `toggleExpanded()` AND `setSize(width, height)` actions:

- `openFloating()`: transitions `mode` from `"bubble"` to `"floating"` AND sets `modalReturnMode = null`. SHALL be a no-op when `mode !== "bubble"`.
- `minimizeToBubble()`: transitions `mode` from `"floating"` to `"bubble"` AND sets `modalReturnMode = null`. SHALL be a no-op when `mode !== "floating"`.
- `openModal()`: when `mode === "bubble"` OR `mode === "floating"`, sets `modalReturnMode` to the current mode AND transitions `mode` to `"modal"`. SHALL be a no-op when `mode === "modal"` (does NOT re-snapshot `modalReturnMode`).
- `dockToFloating()`: transitions `mode` from `"modal"` to `"floating"` AND sets `modalReturnMode = null`. SHALL be a no-op when `mode !== "modal"`.
- `closeModalToReturnMode()`: transitions `mode` from `"modal"` to the value of `modalReturnMode` (falling back to `"bubble"` when `modalReturnMode` is null) AND sets `modalReturnMode = null`. Invoked by Esc keypress while modal is open AND by clicking the backdrop. SHALL be a no-op when `mode !== "modal"`.
- `closeModalToBubble()`: transitions `mode` from `"modal"` to `"bubble"` regardless of `modalReturnMode` AND sets `modalReturnMode = null`. Invoked by clicking the `✕` close button. SHALL be a no-op when `mode !== "modal"`.

`useChatStore.resetForVault(vaultPath)` SHALL additionally reset `mode = "bubble"` AND `modalReturnMode = null` so a vault switch always returns the widget to its initial mode.

The root `data-testid="chat-widget"` element's `data-state` attribute SHALL reflect the current `mode` as the literal string `"bubble"`, `"floating"`, or `"modal"` (replacing the previous `"collapsed"` / `"expanded"` values).

#### Scenario: Bubble mode renders as 44px circle in bottom-right corner

- **WHEN** the user opens a vault AND the Workspace component mounts AND `useGoalsStore.activeRun` is null
- **THEN** an element with `data-testid="chat-widget"` AND `data-state="bubble"` SHALL render as a 44px × 44px rounded button positioned `position: fixed` with `right: 16px` AND `bottom: 48px` (== `BottomStrip height (32px) + 16px gap`) so it sits above the `BottomStrip` AND the bubble's `aria-label` SHALL be the localized translation of `chat.widget.aria.openChat` AND the Workspace main content area SHALL NOT have its width or layout altered by the bubble

#### Scenario: Bubble click opens floating mode

- **WHEN** the user clicks the bubble AND `mode === "bubble"`
- **THEN** `useChatStore.mode` SHALL transition to `"floating"` AND the rendered element SHALL have `data-state="floating"` AND the panel's computed width SHALL equal `360px` AND the panel's computed height SHALL equal `460px` AND the panel SHALL be positioned with `right: 16px` AND `bottom: 48px` AND `modalReturnMode` SHALL remain `null`

#### Scenario: Floating mode has no resize handle

- **WHEN** the widget is in floating mode
- **THEN** no element with `data-testid="chat-widget-resize-handle"` SHALL exist anywhere in the rendered DOM AND the floating panel's dimensions SHALL remain exactly `360px × 460px` regardless of viewport size

#### Scenario: Floating minimize returns to bubble

- **WHEN** `mode === "floating"` AND the user clicks the element with `data-testid="chat-widget-minimize"`
- **THEN** `useChatStore.mode` SHALL transition to `"bubble"` AND `modalReturnMode` SHALL be `null`

#### Scenario: Floating Esc is a no-op

- **WHEN** `mode === "floating"` AND no modal-level component (e.g., NewGoalModal) is open AND the user presses `Esc`
- **THEN** `useChatStore.mode` SHALL remain `"floating"` AND the widget SHALL NOT close or minimize

#### Scenario: Floating expand-to-modal records floating as return mode

- **WHEN** `mode === "floating"` AND the user clicks the element with `data-testid="chat-widget-expand-to-modal"`
- **THEN** `useChatStore.mode` SHALL transition to `"modal"` AND `modalReturnMode` SHALL equal `"floating"`

#### Scenario: Modal mode renders centered with backdrop

- **WHEN** `mode` transitions to `"modal"`
- **THEN** an element with `data-state="modal"` SHALL render via the radix `Dialog` portal with `role="dialog"` AND `aria-modal="true"` AND the modal content SHALL have a computed width of `640px` AND a computed maximum height of `480px` AND a computed top offset of `60px` from the viewport top AND a backdrop element SHALL render behind the modal with a black background at approximately 55% opacity

#### Scenario: Modal input is focused on open

- **WHEN** `mode` transitions from `"bubble"` or `"floating"` to `"modal"`
- **THEN** the modal's input textarea SHALL receive keyboard focus within the same animation frame

#### Scenario: Modal traps focus

- **WHEN** `mode === "modal"` AND the user presses Tab or Shift+Tab repeatedly
- **THEN** keyboard focus SHALL cycle only through focusable elements inside the modal (transcript scrollable region, textarea, send button, dock button, close button) AND SHALL NEVER reach any focusable element outside the modal subtree (e.g., sidebar nav, tab buttons, BottomStrip controls)

#### Scenario: Modal Esc returns to recorded mode

- **WHEN** `mode === "modal"` AND `modalReturnMode === "floating"` AND the user presses `Esc`
- **THEN** `useChatStore.mode` SHALL transition to `"floating"` AND `modalReturnMode` SHALL be `null`

##### Example: Esc round-trip preserves origin mode

| Starting mode | Trigger to modal | Esc result |
| ------------- | ---------------- | ---------- |
| `bubble`      | `⌘K`             | `bubble`   |
| `floating`    | `⌘K`             | `floating` |
| `floating`    | `⤢` expand       | `floating` |

#### Scenario: Modal backdrop click returns to recorded mode

- **WHEN** `mode === "modal"` AND `modalReturnMode === "bubble"` AND the user clicks the modal backdrop (the area outside the modal content)
- **THEN** `useChatStore.mode` SHALL transition to `"bubble"` AND `modalReturnMode` SHALL be `null`

#### Scenario: Modal dock button always returns to floating

- **WHEN** `mode === "modal"` AND the user clicks the element with `data-testid="chat-widget-dock-to-floating"`
- **THEN** `useChatStore.mode` SHALL transition to `"floating"` regardless of the value of `modalReturnMode` AND `modalReturnMode` SHALL be `null`

##### Example: dock button ignores return mode

| `modalReturnMode` at dock-click time | Mode after dock |
| ------------------------------------ | --------------- |
| `"bubble"`                           | `floating`      |
| `"floating"`                         | `floating`      |
| `null`                               | `floating`      |

#### Scenario: Modal close button always returns to bubble

- **WHEN** `mode === "modal"` AND the user clicks the element with `data-testid="chat-widget-modal-close"`
- **THEN** `useChatStore.mode` SHALL transition to `"bubble"` regardless of the value of `modalReturnMode` AND `modalReturnMode` SHALL be `null`

#### Scenario: Modal focus returns to trigger element on close

- **WHEN** `mode === "modal"` AND the modal was opened from `mode === "floating"` by clicking the expand-to-modal button AND the user closes the modal via the `✕` close button
- **THEN** keyboard focus SHALL return to a focusable element representing the bubble (since the close path resets mode to `"bubble"`) OR to the document body (when no focusable bubble is yet mounted) AND focus SHALL NOT remain on any element inside the now-removed modal portal

#### Scenario: Mode switch preserves chat session

- **WHEN** the user has an active chat session with `turns.length = 3` AND `tokensTotal.input_tokens = 1500` AND `activeTurn` is non-null AND the user transitions `mode` through any sequence of bubble / floating / modal
- **THEN** `useChatStore.turns.length` SHALL remain `3` AND `useChatStore.tokensTotal.input_tokens` SHALL remain `1500` AND `useChatStore.activeTurn` SHALL remain non-null AND `useChatStore.sessionId` SHALL be unchanged AND `useChatStore.promoteSuggestion` SHALL be unchanged

#### Scenario: Pending promote suggestion shows badge on bubble

- **WHEN** `mode === "bubble"` AND a `VerbLifecycleEvent::PromoteSuggestion` event arrives via the `chat-stream` channel AND the user has not yet acted on the suggestion
- **THEN** the bubble SHALL render a small red dot badge with `data-testid="chat-widget-promote-badge"` AND the badge SHALL disappear the next time `mode` transitions to `"floating"` or `"modal"` or the suggestion is dismissed

#### Scenario: Chat bubble SHALL NOT render the active-goal pulse dot in any mode

- **WHEN** `useGoalsStore.activeRun` is non-null (a goal is running) AND `mode` is any of `"bubble"` / `"floating"` / `"modal"`
- **THEN** no element with `data-testid="chat-widget-active-goal-pulse"` SHALL be rendered anywhere in the chat widget's subtree (the active-goal ambient indicator lives on the Goals sidebar row instead)

#### Scenario: Bubble aria-label SHALL remain `openChat` regardless of active-goal state

- **WHEN** `mode === "bubble"` AND `useGoalsStore.activeRun` transitions from null to non-null
- **THEN** the element with `data-testid="chat-widget"` SHALL keep its `aria-label` attribute equal to the localized translation of `chat.widget.aria.openChat` AND SHALL NOT switch to any other key (the previously-used `chat.widget.aria.openChatWithActiveGoalRunning` key is removed)

#### Scenario: Reduced motion disables modal open animation

- **WHEN** the user agent reports `prefers-reduced-motion: reduce` AND `mode` transitions to `"modal"`
- **THEN** the modal SHALL appear within the same frame with no perceptible fade or scale animation AND the backdrop SHALL appear within the same frame

#### Scenario: Vault switch resets mode to bubble

- **WHEN** `mode === "floating"` OR `mode === "modal"` AND the user navigates back to Lobby AND opens a different vault
- **THEN** `useChatStore.mode` SHALL be reset to `"bubble"` AND `modalReturnMode` SHALL be `null` for the new vault


<!-- @trace
source: chatwidget-pulse-and-goal-token-display
updated: 2026-05-28
code:
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-app/src/components/workspace/Workspace.tsx
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src/components/workspace/ChatWidget.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
tests:
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
-->

---
### Requirement: Chat Widget Toggle Shortcut

The Workspace SHALL register a keyboard shortcut `Cmd+K` (on macOS) AND `Ctrl+K` (on Windows / Linux) that opens the Chat Widget in `modal` mode (replacing the previous bubble-vs-expanded toggle behavior). The shortcut handler SHALL invoke `useChatStore.getState().openModal()`. The shortcut SHALL be active only while the Workspace component is mounted; the shortcut SHALL NOT fire in the Lobby route. The shortcut handler SHALL call `preventDefault()` on the keydown event to prevent the host browser's default Ctrl+K binding from interfering.

When the user presses `Cmd+K` / `Ctrl+K` while `mode === "bubble"`, the widget SHALL transition to `modal` with `modalReturnMode = "bubble"` so closing the modal returns the widget to bubble mode. When pressed while `mode === "floating"`, the widget SHALL transition to `modal` with `modalReturnMode = "floating"` so closing the modal returns to floating. When pressed while `mode === "modal"`, the shortcut SHALL be a no-op (the modal is already open AND `modalReturnMode` SHALL NOT be re-snapshotted).

#### Scenario: Cmd+K from bubble opens modal with bubble return

- **WHEN** the Workspace is mounted AND `useChatStore.mode === "bubble"` AND the user presses `Cmd+K` (or `Ctrl+K`)
- **THEN** `useChatStore.mode` SHALL transition to `"modal"` AND `useChatStore.modalReturnMode` SHALL equal `"bubble"` AND the keydown event's default action SHALL be prevented

#### Scenario: Cmd+K from floating opens modal with floating return

- **WHEN** the Workspace is mounted AND `useChatStore.mode === "floating"` AND the user presses `Cmd+K` (or `Ctrl+K`)
- **THEN** `useChatStore.mode` SHALL transition to `"modal"` AND `useChatStore.modalReturnMode` SHALL equal `"floating"`

#### Scenario: Cmd+K while modal is open is a no-op

- **WHEN** `useChatStore.mode === "modal"` AND `useChatStore.modalReturnMode === "bubble"` AND the user presses `Cmd+K` (or `Ctrl+K`)
- **THEN** `useChatStore.mode` SHALL remain `"modal"` AND `useChatStore.modalReturnMode` SHALL remain `"bubble"` (not be re-snapshotted or overwritten)

#### Scenario: Shortcut inactive in Lobby

- **WHEN** the Lobby is rendered (no vault selected) AND the user presses `Cmd+K`
- **THEN** no chat widget SHALL appear AND no error SHALL occur AND `useChatStore.mode` SHALL remain at its initial `"bubble"` value when a vault is subsequently opened


<!-- @trace
source: chatwidget-three-modes
updated: 2026-05-27
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/scripts/focus-trap-probe.mjs
  - codebus-app/src/components/workspace/ChatWidget.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - codebus-app/src/store/chat.ts
  - codebus-app/design-handoff/AUDIT.md
tests:
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
-->

---
### Requirement: Chat Session Lifecycle and Reset Triggers

The Chat Widget SHALL maintain a single in-memory session per vault, identified by a `sessionId: String | null` field in `useChatStore`. The `sessionId` starts as `null` AND becomes a non-null string after the first successful `spawn_chat_turn` resolves with the claude CLI session id. Subsequent `spawn_chat_turn` calls within the same session SHALL pass this `sessionId` as the `session_id` parameter so the backend issues `--resume <id>` to the claude CLI.

The session state SHALL be reset to its initial (empty transcript, `sessionId = null`) state on the following triggers ONLY:

1. **Vault switch** — when the Workspace component unmounts (because the user returned to Lobby or opened a different vault). This is enforced by calling `useChatStore.resetForVault()` in the Workspace `useEffect` cleanup. `resetForVault()` SHALL additionally reset `mode = "bubble"` AND `modalReturnMode = null`.
2. **`+ New chat` button** — when the user clicks the button in the floating or modal header. Before resetting, the store SHALL copy the current `sessionId` and `turns` into `lastSessionId` and `lastTranscript` fields. A toast SHALL render with text `"Started new chat. [Undo]"` (or the locale-specific translation) for 5 seconds; clicking `[Undo]` within the 5-second window SHALL restore `sessionId` and `turns` from the saved fields. After 5 seconds the saved fields SHALL be garbage-collected and the toast SHALL fade out.

The session state SHALL NOT be persisted to disk; an application reload SHALL discard the session entirely. The session state SHALL NOT be reset by:

- Switching between Workspace tabs (Goals / Wiki / Quiz)
- Switching the widget between bubble, floating, and modal modes
- An active turn finishing (succeeded, cancelled, or failed)

#### Scenario: Vault switch resets the chat session and returns widget to bubble mode

- **WHEN** the user has an active chat session for vault `V1` with `sessionId = "abc-123"` AND `turns.length = 3` AND `mode === "modal"` AND `modalReturnMode === "floating"` AND the user clicks `← Back to Lobby` AND then opens vault `V2`
- **THEN** the `useChatStore` state SHALL have `sessionId = null` AND `turns.length = 0` AND `mode = "bubble"` AND `modalReturnMode = null` for `V2` so the widget opens as a bubble in the fresh vault

#### Scenario: + New chat triggers undo toast

- **WHEN** the user has an active chat session with `sessionId = "abc-123"` AND `turns.length = 3` AND the user clicks `+ New chat` (rendered inside floating or modal mode header)
- **THEN** the `useChatStore.sessionId` SHALL become `null` AND `turns.length` SHALL become `0` AND a toast with `data-testid="chat-undo-toast"` SHALL render with text containing `"Started new chat"` AND an `[Undo]` button AND `useChatStore.mode` SHALL remain at its current floating-or-modal value

#### Scenario: Undo within 5 seconds restores session

- **WHEN** the `chat-undo-toast` is visible (less than 5 seconds since `+ New chat` clicked) AND the user clicks `[Undo]`
- **THEN** the `useChatStore.sessionId` SHALL be restored to its previous value (`"abc-123"`) AND `turns` SHALL be restored to its previous content AND the toast SHALL disappear

#### Scenario: Undo buffer gc'd after 5 seconds

- **WHEN** the `chat-undo-toast` has been visible for 5 seconds AND no `[Undo]` click occurred
- **THEN** the toast SHALL fade out AND the `lastSessionId` / `lastTranscript` fields in `useChatStore` SHALL be set back to `null`

#### Scenario: Tab switch preserves chat session and mode

- **WHEN** the user has an active chat session with `turns.length = 3` AND `mode === "floating"` AND the user switches the Workspace tab from Goals to Wiki AND back to Goals
- **THEN** the `useChatStore.turns.length` SHALL still equal `3` AND `useChatStore.mode` SHALL still equal `"floating"`


<!-- @trace
source: chatwidget-three-modes
updated: 2026-05-27
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/scripts/focus-trap-probe.mjs
  - codebus-app/src/components/workspace/ChatWidget.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - codebus-app/src/store/chat.ts
  - codebus-app/design-handoff/AUDIT.md
tests:
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
-->

---
### Requirement: Tauri IPC Commands for Chat Turn Lifecycle

The system SHALL register two new Tauri commands for chat turn lifecycle, extending the goal lifecycle IPC surface:

- `spawn_chat_turn(vault_path: String, text: String, session_id: Option<String>) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::chat::run_chat_turn` with `ChatTurnOptions { text, session_id }`. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the new `RunId` (where `RunId` = `chat-<started_at_slug>` and the slug is derived from `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` with `:` replaced by `-`), and emit each `VerbEvent` produced by the closure to a Tauri event channel named `"chat-stream"` with payload `{ run_id: String, event: VerbEvent }`. The chat-stream channel SHALL be separate from the existing `goal-stream` channel. On thread completion (success, failure, cancel, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_chat_turn(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (turn already terminated), return `Ok(())` (idempotent).

`spawn_chat_turn` SHALL return `AppError::Invalid { field: "active_runs", message: "another chat turn is already active in this session" }` when invoked while `active_runs` already contains a `chat-*` keyed entry for the same vault. Goal-mode entries SHALL NOT block chat spawn AND vice versa (see `One Active Goal Run At A Time` modification).

The chat `RunId` SHALL be derived using `chrono::SecondsFormat::Millis` precision so that two `spawn_chat_turn` invocations occurring within the same wall-clock second (necessarily across distinct vault paths, since per-vault concurrency is bounded to one) receive distinct `active_runs` keys and the second invocation does not overwrite the first invocation's cancel handle.

#### Scenario: spawn_chat_turn returns chat run id

- **WHEN** the frontend calls `invoke("spawn_chat_turn", { vault_path: "/some/vault", text: "X", session_id: null })` AND the spawned `run_chat_turn` invocation's first stream event timestamps the turn at `2026-05-14T10:20:30.456Z`
- **THEN** the IPC call resolves with `"chat-2026-05-14T10-20-30.456Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string

#### Scenario: spawn_chat_turn same-second calls across vaults yield distinct RunIds

- **WHEN** the frontend calls `invoke("spawn_chat_turn", { vault_path: "V1", ... })` AND then `invoke("spawn_chat_turn", { vault_path: "V2", ... })` in rapid succession AND both calls land within the same wall-clock second but on distinct wall-clock milliseconds
- **THEN** the two IPC calls SHALL resolve with two distinct `chat-<slug>` `RunId` strings differing in the `.fff` fractional component AND `AppState.active_runs` SHALL contain two entries simultaneously, one for each vault, each with its own cancel handle

#### Scenario: spawn_chat_turn rejects when chat turn already active

- **WHEN** a chat turn is currently active for vault `V` (an entry exists in `active_runs` with key starting `chat-`) AND the frontend invokes `spawn_chat_turn` with the same vault
- **THEN** the IPC call rejects with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring `"chat turn is already active"`

#### Scenario: chat-stream events forwarded with run_id payload

- **WHEN** `spawn_chat_turn` is invoked AND the backend emits a `VerbEvent::Stream { ... }` event
- **THEN** the Tauri event channel `chat-stream` SHALL receive a payload `{ run_id: "chat-<slug>", event: <VerbEvent JSON> }`

#### Scenario: cancel_chat_turn idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_chat_turn", { run_id: "chat-nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error


<!-- @trace
source: backend-cleanup-codex-websearch-and-runid-millis
updated: 2026-05-28
code:
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/goal.rs
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src-tauri/src/ipc/chats.rs
  - codebus-core/src/agent/codex_backend.rs
  - docs/2026-05-28-run-id-collision-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-28-runid-source-of-truth-todo.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/src-tauri/src/ipc/goals.rs
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
  - docs/2026-05-28-cancelling-stuck-todo.md
  - codebus-core/src/verb/query.rs
-->

---
### Requirement: Promote Suggestion Inline Pill and Spawn-Goal Flow

When the chat-stream channel delivers a `VerbLifecycleEvent::PromoteSuggestion { reason }` event during a turn, the Chat Widget SHALL render an inline pill at the end of the assistant message produced in that same turn. The pill SHALL contain the visible text `[Promote to goal: <reason>]` (where `<reason>` is the event's `reason` field) AND a separate `[Dismiss]` button. The pill SHALL persist on the assistant message after the turn completes so the user can act on it later in the conversation.

When the user clicks the `[Promote to goal]` portion of the pill, the Chat Widget SHALL:

1. Construct a transcript dump string from the current session's turns using the format defined in the design's `Transcript Dump Format for Promote` section (user/assistant labels per turn, terminating with `Write: <reason>`).
2. Invoke the existing `spawn_goal(vault_path, transcript)` IPC command.
3. On success: transition the Chat Widget to `collapsed` AND set the Workspace active tab to `goals` AND select the newly returned `run_id` so the Workspace router lands on `RunDetailRunning` for the new goal.
4. On failure (e.g., another goal is already active): render an inline error message adjacent to the pill containing the substring `"Another goal is running"` (or the locale-specific translation) AND leave the pill clickable so the user can retry after the active goal finishes.

When the user clicks the `[Dismiss]` button, the pill SHALL be removed from the assistant message AND `useChatStore.promoteSuggestion` SHALL be set to `null`. Dismissed suggestions SHALL NOT be re-emitted on the same assistant message; the user must explicitly request promotion in a later turn for the agent to emit a new suggestion.

#### Scenario: Promote pill renders on assistant message

- **WHEN** the chat-stream channel emits a `VerbLifecycleEvent::PromoteSuggestion { reason: "auth + JWT 適合寫成 wiki" }` during turn 3 AND turn 3's assistant message is rendered
- **THEN** the assistant message SHALL contain an inline pill element with `data-testid="promote-pill"` whose text contains `"Promote to goal: auth + JWT 適合寫成 wiki"` AND a sibling `[Dismiss]` button

#### Scenario: Click Promote spawns goal with transcript

- **WHEN** the user has 3 completed turns AND a promote pill is visible AND the user clicks `[Promote to goal]`
- **THEN** the frontend SHALL invoke `spawn_goal(vault_path, transcript)` where `transcript` is a string starting with `"Based on this conversation:"` followed by alternating `<user>:` / `<assistant>:` blocks for all 3 turns AND ending with `"Write: <reason>"` (where `<reason>` is the pill's reason field)

#### Scenario: Successful promote collapses widget and routes to RunDetailRunning

- **WHEN** the `[Promote to goal]` click results in `spawn_goal` resolving with `run_id = "2026-05-14T10-20-30Z"`
- **THEN** the Chat Widget state SHALL become `collapsed` AND the Workspace active tab SHALL become `goals` AND the Workspace's `selectedRunId` SHALL equal `"2026-05-14T10-20-30Z"` AND the rendered detail view SHALL be `RunDetailRunning`

#### Scenario: Promote fails when goal already active

- **WHEN** another goal is currently active AND the user clicks `[Promote to goal]`
- **THEN** `spawn_goal` SHALL reject with `AppError::Invalid` AND the pill area SHALL render an inline error message containing the substring `"Another goal is running"` AND the pill SHALL remain clickable for retry

#### Scenario: Dismiss removes pill and prevents re-emit on same message

- **WHEN** a promote pill is visible AND the user clicks `[Dismiss]`
- **THEN** the pill SHALL be removed from the DOM AND the same assistant message SHALL NOT show the pill again, even if the page re-renders

<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
-->


<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
code:
  - docs/2026-05-14-github-repo-setup-backlog.md
  - docs/2026-05-15-codebus-fs-watcher-backlog.md
  - codebus-app/src/components/workspace/ChatInput.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - docs/2026-05-14-rag-index-search-backlog.md
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/src/components/workspace/ChatUndoToast.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/2026-05-14-mcp-server-backlog.md
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-14-openai-privacy-filter-backlog.md
  - docs/2026-05-14-mycoder-cli-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - docs/2026-05-14-settings-chat-model-backlog.md
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - docs/2026-05-14-ui-accessibility-backlog.md
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - docs/2026-05-14-app-font-scale-backlog.md
  - docs/BACKLOG.md
  - codebus-app/src/store/chat.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/ChatWidget.tsx
tests:
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatInput.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Chat Onboarding Hint and Placeholder

Whenever the Chat Widget's transcript region is empty (no completed turns AND no active streaming turn), the transcript region SHALL render an onboarding hint containing exactly the following two pieces of information:

1. A statement that the user can ask anything about the vault.
2. A statement covering both promote paths — that the AI SHALL surface a Promote suggestion when it judges the discussion worth documenting AND that the user can also explicitly request the AI to promote a discussion in plain language.

The hint text SHALL match the locale message keys `chat.onboarding.hintEn` / `chat.onboarding.hintTw` (and equivalents for other supported locales). The English version SHALL contain the substring `"AI will suggest"` AND the substring `"ask AI to promote"`. The Traditional Chinese version SHALL contain the substring `"主動建議"` AND the substring `"主動跟 AI 講"`.

The hint SHALL be hidden as soon as the transcript has at least one completed turn OR an active streaming turn; the input placeholder takes over from there. The placeholder text SHALL match locale keys `chat.placeholder.en` / `chat.placeholder.tw` and SHALL contain `"Ask anything"` (en) or `"輸入訊息"` (tw).

The hint SHALL re-appear every time the transcript returns to the empty state — including after the `+ New chat` button clears the session AND after a vault round-trip via the Lobby. The hint MUST NOT be gated by any per-vault `localStorage` flag; manual UX verification confirmed the user expects promote-suggestion mechanics to be reaffirmed at the start of every fresh conversation, not just the very first time per vault.

#### Scenario: Empty transcript shows onboarding hint

- **WHEN** the Chat Widget is rendered AND `useChatStore.turns.length === 0` AND `useChatStore.activeTurn === null`
- **THEN** the transcript region SHALL render an element with `data-testid="chat-onboarding-hint"` containing the substring `"AI will suggest"` (en locale) or `"主動建議"` (tw locale)

#### Scenario: + New chat returns the transcript to the empty state and the hint re-appears

- **WHEN** the user has completed at least one turn AND clicks `+ New chat` AND the store clears `sessionId` / `turns`
- **THEN** the transcript region SHALL again render `data-testid="chat-onboarding-hint"`

#### Scenario: Hint hides as soon as an active or completed turn exists

- **WHEN** the Chat Widget is rendered AND either `useChatStore.turns.length > 0` OR `useChatStore.activeTurn !== null`
- **THEN** the transcript region SHALL NOT render `chat-onboarding-hint` AND the input placeholder SHALL be the localized "Ask anything" string

<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
-->


<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
code:
  - docs/2026-05-14-github-repo-setup-backlog.md
  - docs/2026-05-15-codebus-fs-watcher-backlog.md
  - codebus-app/src/components/workspace/ChatInput.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - docs/2026-05-14-rag-index-search-backlog.md
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/src/components/workspace/ChatUndoToast.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/2026-05-14-mcp-server-backlog.md
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-14-openai-privacy-filter-backlog.md
  - docs/2026-05-14-mycoder-cli-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - docs/2026-05-14-settings-chat-model-backlog.md
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - docs/2026-05-14-ui-accessibility-backlog.md
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - docs/2026-05-14-app-font-scale-backlog.md
  - docs/BACKLOG.md
  - codebus-app/src/store/chat.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/ChatWidget.tsx
tests:
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatInput.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Chat Token Usage Display

The Chat Widget header SHALL display a token usage indicator on the right side showing the cumulative tokens used in the current session. The display format SHALL be `<N>k ↑` where `<N>` is the formatted total in thousands (one decimal place when below 10k, integer otherwise), summing `input_tokens` + `output_tokens` across every turn's `Usage` stream event in the session. Hovering the indicator (mouse pointer OR keyboard focus) SHALL reveal a tooltip showing the four sub-values: `cache_read_input_tokens`, `cache_creation_input_tokens`, `input_tokens`, AND `output_tokens` with bilingual labels. The indicator SHALL NOT display a USD cost estimate.

When `useChatStore.turns.length === 0` AND no active turn has emitted a usage event, the indicator SHALL render `0 ↑` (not hidden — keeps layout stable across session lifetime).

#### Scenario: Token total renders in widget header

- **WHEN** the session has accumulated `input_tokens = 1200` AND `output_tokens = 2221` across all turns
- **THEN** an element with `data-testid="chat-token-display"` SHALL render in the widget header with visible text containing `"3.4k ↑"`

#### Scenario: Tooltip reveals token breakdown on hover

- **WHEN** the user hovers the `chat-token-display` element
- **THEN** a tooltip SHALL appear containing four labeled values (`input`, `output`, `cache read`, `cache create`) corresponding to the summed stream-event fields

#### Scenario: Zero state renders 0k

- **WHEN** the widget is just expanded for a fresh session with no turns
- **THEN** the `chat-token-display` SHALL render visible text containing `"0 ↑"` (zero with the up-arrow glyph)

<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
-->


<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
code:
  - docs/2026-05-14-github-repo-setup-backlog.md
  - docs/2026-05-15-codebus-fs-watcher-backlog.md
  - codebus-app/src/components/workspace/ChatInput.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - docs/2026-05-14-rag-index-search-backlog.md
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/src/components/workspace/ChatUndoToast.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/2026-05-14-mcp-server-backlog.md
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-14-openai-privacy-filter-backlog.md
  - docs/2026-05-14-mycoder-cli-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - docs/2026-05-14-settings-chat-model-backlog.md
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - docs/2026-05-14-ui-accessibility-backlog.md
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - docs/2026-05-14-app-font-scale-backlog.md
  - docs/BACKLOG.md
  - codebus-app/src/store/chat.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/ChatWidget.tsx
tests:
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatInput.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Chat Activity Stream Reuse with Per-Turn Grouping

The Chat Widget transcript region SHALL render each completed turn as a vertical block containing: (1) the user's prompt text on top, then (2) the assistant's events rendered via the existing `ActivityStreamItem`, `ThoughtItem`, and `foldTimeline` exports from `app-workspace` so that `tool_use` one-liners, thought blocks, and assistant text appear with the same visual treatment as the Goal `RunDetailRunning` activity stream. Consecutive turn blocks SHALL be separated by a horizontal divider OR clear vertical spacing so users can distinguish turn boundaries.

The active (in-progress) turn SHALL be rendered at the bottom of the transcript region with the user's prompt at the top and the events buffer driven by the live `chat-stream` channel below.

The transcript region SHALL be a standard scrollable container (CSS `overflow: auto`) and SHALL NOT perform any automatic scroll on event arrival. Manual UX verification on the first GUI build showed the sticky-to-bottom flag did not pin reliably in the real DOM and the auto-scroll requirement was dropped from v1 scope; users scroll manually to follow the live stream. A future change is expected to reintroduce auto-scroll with a verified implementation.

#### Scenario: Completed turn renders user + assistant + tool one-liners

- **WHEN** a turn completes with the user prompt `"auth 怎麼運作"` AND the assistant emitted one tool_use (Read `wiki/modules/auth.md`) AND one assistant text chunk `"JWT-based..."`
- **THEN** the transcript SHALL contain a block with the user prompt at the top, then the tool one-liner (matching the `ActivityStreamItem` Read pattern e.g. `🛠 Read · auth.md`), then the assistant text `"JWT-based..."` rendered via the markdown renderer

#### Scenario: Two turns separated by divider

- **WHEN** the user has completed turn 1 (`"auth 怎麼運作"`) AND turn 2 (`"JWT 也講"`)
- **THEN** the transcript SHALL render two distinct blocks separated by a horizontal divider or vertical spacing visible to a screen reader as a turn boundary

#### Scenario: Transcript region uses standard browser scroll only

- **WHEN** the active turn emits new events AND the resulting content exceeds the transcript region's viewport
- **THEN** the transcript region SHALL continue rendering new events at the bottom AND `scrollTop` SHALL NOT be programmatically modified; users SHALL be able to scroll manually with mouse wheel / touch / keyboard to follow the live stream

<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
-->


<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
code:
  - docs/2026-05-14-github-repo-setup-backlog.md
  - docs/2026-05-15-codebus-fs-watcher-backlog.md
  - codebus-app/src/components/workspace/ChatInput.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - docs/2026-05-14-rag-index-search-backlog.md
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/src/components/workspace/ChatUndoToast.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/2026-05-14-mcp-server-backlog.md
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-14-openai-privacy-filter-backlog.md
  - docs/2026-05-14-mycoder-cli-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - docs/2026-05-14-settings-chat-model-backlog.md
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - docs/2026-05-14-ui-accessibility-backlog.md
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - docs/2026-05-14-app-font-scale-backlog.md
  - docs/BACKLOG.md
  - codebus-app/src/store/chat.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/ChatWidget.tsx
tests:
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatInput.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Chat Assistant Message Markdown Rendering and Wiki Citation Links

The Chat Widget SHALL render each assistant message's text content through a Markdown renderer (`react-markdown`) rather than as plain text. The renderer SHALL be configured with the `remark-gfm` plugin so GitHub-flavored Markdown tables, strikethrough, AND task lists render as their corresponding HTML elements (`<table>` / `<del>` / task-list items) instead of leaking through as raw markdown syntax.

Before passing assistant text to `react-markdown`, the renderer SHALL pre-process the text by replacing every `[[slug]]` occurrence with a standard markdown link of the form `[<slug>](codebus://wiki/<percent-encoded-slug>)` (reusing the existing `transformBodyWikilinks` helper shared with the wiki preview surface). The renderer SHALL pass an `urlTransform` to `react-markdown` that returns each URL unchanged so the synthetic `codebus://wiki/...` scheme survives the renderer's default safelist (which would otherwise strip non-http(s)/mailto schemes).

The custom `a` element override SHALL classify each rendered link by `href` shape AND route the click accordingly:

- **Wikilink (codebus scheme)**: when `href` starts with `codebus://wiki/`, the renderer SHALL extract the slug by stripping that prefix AND percent-decoding the remainder. The renderer SHALL consult `useWikiStore.pages` (the client-side page index loaded at Workspace mount time) to classify the slug:
  - **Resolvable** (slug exists in `pages`): rendered as a `<button>`-like clickable element whose visible text is `pages[slug].title` when present, otherwise the raw slug. Clicking SHALL invoke `onWikiLinkClick(slug)` (passing the **decoded slug**, NOT the raw href) AND SHALL transition the Chat Widget to `collapsed` via `useChatStore.toggleExpanded()` (the existing collapse helper already short-circuits when already collapsed).
  - **Unresolvable** (slug missing from `pages`): rendered as a dimmed `<span>` with a `title` tooltip reading "Page not found". Clicking SHALL be a no-op (no `onWikiLinkClick` invocation, no widget transition).
- **Legacy wiki markdown link**: when `href` matches the regex `^wiki\/(.+)\.md$` (used by older agent outputs that embedded markdown links of the form `[label](wiki/<path>.md)`), the renderer SHALL extract the slug from the capture group (the path between `wiki/` AND the trailing `.md`) AND route through the SAME resolvable / unresolvable flow as the codebus-scheme branch. The capture group's value SHALL be the slug passed to `onWikiLinkClick`; the raw href SHALL NOT be passed.
- **External link**: when `href` starts with `http://` or `https://`, the renderer SHALL invoke the existing Tauri opener plugin with the URL. The Workspace active tab SHALL NOT change AND the Chat Widget SHALL remain in its current state.
- **Other**: any other `href` shape (for example source code paths like `src/auth/jwt.rs`) SHALL render as an inert `<span>` with no click handler AND no `<a>` tag carrying a non-empty href.

The `onWikiLinkClick` callback on `ChatTranscript` AND its descendants SHALL accept a **slug string**, NOT a raw href. Callers (notably `Workspace.onSelectPage(slug)`) SHALL receive the post-extraction slug regardless of whether the source markdown used `[[slug]]` syntax or the legacy `[label](wiki/<path>.md)` form. This contract change corrects a prior type-lie where the callback was documented AND typed as receiving an href but the only production consumer (`Workspace.onSelectPage`) treated the argument as a slug AND fed it to `useWikiStore.loadPage(vault, slug)` — leading to a `wiki/wiki/<path>.md.md` lookup miss if the chat had ever actually emitted a clickable wiki markdown link.

Plain-text mentions of wiki paths within an assistant message (for example `"see wiki/modules/auth.md"` without markdown link syntax AND without `[[...]]` syntax) SHALL NOT be auto-detected or made clickable; only markdown link syntax OR `[[slug]]` syntax SHALL produce clickable elements.

#### Scenario: GFM table renders as table element

- **WHEN** an assistant message contains the GFM markdown text below (column separators, header divider, two rows of data)

  ```
  | Tool | Replaces |
  |------|----------|
  | uv   | pip      |
  | ruff | flake8   |
  ```

- **THEN** the rendered DOM SHALL contain a `<table>` element with at least one `<th>` element bearing the text `Tool` AND at least one `<td>` element bearing the text `uv` AND SHALL NOT contain raw `|---|` text in the rendered prose

#### Scenario: Wikilink markdown syntax renders as clickable resolvable link

- **WHEN** an assistant message contains the plain text `[[modules/auth]]` AND `useWikiStore.pages["modules/auth"]` exists AND the user clicks the rendered link
- **THEN** the rendered link's visible text SHALL be `pages["modules/auth"].title` (falling back to `modules/auth` when the title is empty) AND the click SHALL invoke `onWikiLinkClick("modules/auth")` (the decoded slug, NOT a raw href) AND the Chat Widget SHALL transition to `collapsed`

#### Scenario: Wikilink to nonexistent page renders dimmed and is inert

- **WHEN** an assistant message contains `[[nonexistent-page]]` AND `useWikiStore.pages["nonexistent-page"]` does NOT exist AND the user clicks the rendered text
- **THEN** the rendered element SHALL be a `<span>` (NOT a `<button>` or `<a>` with click handler) AND its `title` attribute SHALL equal "Page not found" AND `onWikiLinkClick` SHALL NOT be invoked AND the Chat Widget SHALL NOT transition

#### Scenario: Legacy wiki markdown link click passes slug not href

- **WHEN** an assistant message contains the markdown text `[auth](wiki/modules/auth.md)` AND `useWikiStore.pages["modules/auth"]` exists AND the user clicks the rendered link
- **THEN** the Workspace active tab SHALL become `wiki` AND `onWikiLinkClick` SHALL be invoked with the slug `"modules/auth"` (the regex capture group between `wiki/` AND `.md`, NOT the raw href `"wiki/modules/auth.md"`) AND the Chat Widget SHALL transition to `collapsed`

#### Scenario: External https link opens in browser

- **WHEN** an assistant message contains `[docs](https://example.com/foo)` AND the user clicks the link
- **THEN** the Tauri opener plugin SHALL be invoked with the URL `https://example.com/foo` AND the Workspace active tab SHALL NOT change AND the Chat Widget SHALL remain in its current state

#### Scenario: Source code path renders as inert text

- **WHEN** an assistant message contains the markdown text `[jwt.rs](src/auth/jwt.rs)` AND the user clicks the rendered text
- **THEN** no navigation or IPC call SHALL occur AND the rendered element SHALL NOT have an `<a>` tag with a non-empty href OR equivalent click handler

#### Scenario: Plain text wiki mention without markdown or wikilink syntax is not clickable

- **WHEN** an assistant message contains the plain text `"see wiki/modules/auth.md for details"` (no markdown link syntax, no `[[...]]` wrapping)
- **THEN** the rendered text `"wiki/modules/auth.md"` SHALL NOT have a click handler attached AND SHALL render as inert prose


<!-- @trace
source: chat-display-polish-app
updated: 2026-05-23
code:
  - codebus-app/src/components/workspace/ChatTranscript.tsx
tests:
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Chat Widget Mount at Workspace Level

The Chat Widget element SHALL be rendered by the `Workspace` component (not by any individual tab component such as `GoalsTab`, `WikiTab`, or `QuizTab`) so that the widget remains mounted with consistent state across tab switches within the same vault. The widget SHALL be positioned via fixed/absolute CSS such that it overlays the entire Workspace main area regardless of which tab is currently displayed underneath.

When the user changes tabs (Goals → Wiki → Quiz or any other transition), the Chat Widget's state (`expanded`, `width`, `height`, transcript content, session id, active turn) SHALL be preserved without re-mounting the component.

#### Scenario: Chat persists across tab switches

- **WHEN** the user expands the Chat Widget on the Goals tab AND types one turn AND switches to the Wiki tab AND back to Goals
- **THEN** the Chat Widget SHALL still be expanded AND the typed turn SHALL still appear in the transcript AND `useChatStore.sessionId` SHALL still equal the value from before the tab switch

<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
-->

<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

<!-- @trace
source: v3-app-chat-cmdk
updated: 2026-05-15
code:
  - docs/2026-05-14-github-repo-setup-backlog.md
  - docs/2026-05-15-codebus-fs-watcher-backlog.md
  - codebus-app/src/components/workspace/ChatInput.tsx
  - codebus-app/src/hooks/useChatShortcut.ts
  - docs/2026-05-14-rag-index-search-backlog.md
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/src/components/workspace/ChatUndoToast.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/2026-05-14-mcp-server-backlog.md
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
  - codebus-app/src-tauri/src/ipc/chats.rs
  - docs/2026-05-14-openai-privacy-filter-backlog.md
  - docs/2026-05-14-mycoder-cli-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - docs/2026-05-14-settings-chat-model-backlog.md
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - docs/2026-05-14-ui-accessibility-backlog.md
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - docs/2026-05-14-app-font-scale-backlog.md
  - docs/BACKLOG.md
  - codebus-app/src/store/chat.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/ChatWidget.tsx
tests:
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/ChatInput.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src/components/workspace/ChatUndoToast.test.tsx
  - codebus-app/src/hooks/useChatShortcut.test.tsx
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/components/workspace/ChatTokenDisplay.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
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


<!-- @trace
source: quiz-fullscreen-wizard-view
updated: 2026-05-27
code:
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/components/workspace/QuizWizardScopeConfirm.tsx
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/workspace/QuizWizardGenerating.tsx
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/ui/TabContentHeader.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->

---
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


<!-- @trace
source: quiz-attempt-progress
updated: 2026-05-19
code:
  - codebus-core/src/verb/quiz.rs
-->

---
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


<!-- @trace
source: quiz-attempt-progress
updated: 2026-05-19
code:
  - codebus-core/src/verb/quiz.rs
-->

---
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

<!-- @trace
source: quiz-attempt-progress
updated: 2026-05-19
code:
  - codebus-core/src/verb/quiz.rs
-->

---
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

<!-- @trace
source: quiz-content-verify
updated: 2026-05-19
code:
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/config/quiz.rs
  - codebus-cli/src/commands/quiz.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src-tauri/src/ipc/quiz.rs
tests:
  - codebus-cli/tests/quiz_flow.rs
  - codebus-core/tests/verb_library_surface.rs
  - codebus-cli/tests/bins/mock_claude.rs
-->

---
### Requirement: Goal Content Verify GUI Wiring

The GUI goal-spawn Tauri IPC command SHALL participate in the optional content verification stage defined by the `verb-library` capability's `Goal Content Verification and Repair` requirement, with behavior parity to the CLI and without adding a new IPC command or a content-review UI element.

The command SHALL resolve `goal.content_verify` from the shared `goal.*` configuration using the same core loader the CLI uses (default `false`; a config load error SHALL fall back to `false` rather than silently enabling extra spawns). It SHALL pass the originating goal text into `run_goal` so the off-goal defect check can run. When `goal.content_verify` is `false`, the GUI goal flow SHALL be unchanged and no content-review status SHALL be produced. When `true`, the GUI-driven `run_goal` SHALL run the same verify→repair stage the CLI does (events stream over the existing goal channel); `auto_commit` and the run outcome SHALL be unaffected by content verification beyond the content-review status.

#### Scenario: GUI resolves config and threads goal text

- **WHEN** the GUI goal-spawn IPC runs with `goal.content_verify` set to `true`
- **THEN** `run_goal` SHALL receive `content_verify = true` and the originating goal text AND the verify→repair stage SHALL run with events on the goal stream channel

#### Scenario: GUI default-off leaves the flow unchanged

- **WHEN** the GUI goal-spawn IPC runs and `goal.content_verify` is absent or `false`
- **THEN** no verify spawn SHALL run AND no content-review status SHALL be produced AND no new IPC command or content-review UI element SHALL be introduced

#### Scenario: GUI config load error is conservative

- **WHEN** the shared goal config cannot be loaded
- **THEN** the GUI SHALL treat `content_verify` as `false` (do not silently enable extra spawns)

<!-- @trace
source: goal-content-verify
updated: 2026-05-19
code:
  - codebus-core/src/config/mod.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/mod.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/verb/content_verify.rs
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/git/mod.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/goal.rs
tests:
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/goal_content_verify_cli.rs
  - codebus-cli/tests/goal_flow.rs
-->

---
### Requirement: Wiki Tab Subscribes To Watcher Events

The Wiki tab SHALL subscribe to the `wiki-list-changed` and `wiki-page-changed` Tauri events defined by the `fs-watcher` capability. On `wiki-list-changed` the Wiki tab SHALL invoke `useWikiStore.listPages()` to refresh the tree. On `wiki-page-changed` the WikiPreview component SHALL compare the event payload `path` against its currently rendered page; if they match, the preview SHALL re-fetch and re-render that page's content. If they do not match, the preview SHALL ignore the event.

#### Scenario: External edit refreshes the wiki tree

- **GIVEN** the Wiki tab is mounted AND a vault watcher is active
- **WHEN** an external editor saves a new file `<V>/.codebus/wiki/concepts/new.md`
- **THEN** the Wiki tree SHALL show `new.md` within 400 ms without a manual tab switch

#### Scenario: External edit of the open page refreshes the preview

- **GIVEN** WikiPreview is rendering `<V>/.codebus/wiki/concepts/foo.md`
- **WHEN** an external editor modifies that same file
- **THEN** WikiPreview SHALL re-fetch and re-render the file's new content within 400 ms

#### Scenario: External edit of a non-open page does not refresh the preview

- **GIVEN** WikiPreview is rendering `<V>/.codebus/wiki/concepts/foo.md`
- **WHEN** an external editor modifies `<V>/.codebus/wiki/concepts/other.md`
- **THEN** WikiPreview SHALL NOT re-fetch foo.md AND its rendered content SHALL remain unchanged


<!-- @trace
source: codebus-fs-watcher
updated: 2026-05-20
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src-tauri/src/watcher.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - Cargo.toml
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/hooks/useWatcherEvent.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/lib.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/src/components/workspace/WatcherStatusBanner.tsx
  - codebus-app/src/store/vault-watcher-status.ts
  - codebus-app/src-tauri/src/ipc/mod.rs
tests:
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/hooks/useWatcherEvent.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/vault-watcher-status.test.ts
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
-->

---
### Requirement: Goals Tab Subscribes To Watcher Events

The Goals tab SHALL subscribe to the `goals-changed` and `goal-run-changed` Tauri events. On `goals-changed` the tab SHALL invoke `useGoalsStore.refreshRuns()`. On `goal-run-changed` any currently mounted RunDetailRunning or RunDetailDone component SHALL compare the event payload `run_id` against its currently displayed run; if they match, the component SHALL re-fetch the run's events and RunLog summary.

#### Scenario: Terminal-spawned goal becomes visible in Goals list

- **GIVEN** the Goals tab is mounted AND no GUI goal run is in flight
- **WHEN** a terminal session writes a new `events-*.jsonl` and `runs-*.jsonl` for a goal run
- **THEN** the Goals list SHALL include the new run within 400 ms

#### Scenario: Live append to currently viewed run is reflected

- **GIVEN** RunDetailRunning is displaying run `R` that was spawned externally
- **WHEN** the corresponding `events-<R>.jsonl` receives appended lines
- **THEN** RunDetailRunning SHALL re-fetch the events and render the new lines within 400 ms

#### Scenario: Append to a different run does not refetch the open run

- **GIVEN** RunDetailDone is displaying run `R1`
- **WHEN** the `events-<R2>.jsonl` file for a different run is appended
- **THEN** RunDetailDone SHALL NOT re-fetch `R1`'s events


<!-- @trace
source: codebus-fs-watcher
updated: 2026-05-20
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src-tauri/src/watcher.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - Cargo.toml
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/hooks/useWatcherEvent.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/lib.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/src/components/workspace/WatcherStatusBanner.tsx
  - codebus-app/src/store/vault-watcher-status.ts
  - codebus-app/src-tauri/src/ipc/mod.rs
tests:
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/hooks/useWatcherEvent.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/vault-watcher-status.test.ts
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
-->

---
### Requirement: Quiz Tab Subscribes To Watcher Events

The Quiz tab SHALL subscribe to the `quiz-changed` and `quiz-attempt-changed` Tauri events. On `quiz-changed` the tab SHALL rescan `<vault>/.codebus/quiz/` and update its history view. On `quiz-attempt-changed` any currently mounted QuizAnswering or QuizReview component SHALL compare the event payload `{ slug, id }` against its currently displayed attempt; if they match, the component SHALL re-fetch the attempt's markdown and progress sidecar.

#### Scenario: Terminal-spawned quiz becomes visible in history

- **GIVEN** the Quiz tab is mounted
- **WHEN** a terminal session writes a new `<V>/.codebus/quiz/<slug>/<id>.md`
- **THEN** the Quiz history view SHALL include the new attempt within 400 ms

#### Scenario: External progress edit refreshes open attempt

- **GIVEN** QuizAnswering is displaying attempt `(slug=jwt-basics, id=2026-05-20T08-30-00Z)`
- **WHEN** an external process modifies that attempt's `.progress.json` sidecar
- **THEN** QuizAnswering SHALL re-fetch the sidecar and update its rendered progress within 400 ms

#### Scenario: Edit of a different attempt does not refetch

- **GIVEN** QuizReview is displaying attempt `A1`
- **WHEN** a different attempt `A2` is modified externally
- **THEN** QuizReview SHALL NOT re-fetch `A1`'s files


<!-- @trace
source: codebus-fs-watcher
updated: 2026-05-20
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src-tauri/src/watcher.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - Cargo.toml
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/hooks/useWatcherEvent.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/lib.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/src/components/workspace/WatcherStatusBanner.tsx
  - codebus-app/src/store/vault-watcher-status.ts
  - codebus-app/src-tauri/src/ipc/mod.rs
tests:
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/hooks/useWatcherEvent.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/vault-watcher-status.test.ts
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
-->

---
### Requirement: Watcher Error Surfaces Auto-Refresh-Disabled State

The Workspace SHALL subscribe to the `vault-watcher-error` Tauri event and SHALL display a persistent inline indicator on every affected tab (Wiki, Goals, Quiz) when the event fires for the open vault. The indicator SHALL state that auto-refresh is disabled and SHALL include the failure reason. The indicator SHALL remain visible for the rest of the Workspace session for that vault; the frontend SHALL NOT attempt to restart the watcher automatically.

#### Scenario: Auto-refresh-disabled indicator appears on all tabs after watcher failure

- **GIVEN** the Workspace is mounted for vault V
- **WHEN** the backend emits `vault-watcher-error { vault_path: V, reason: "..." }`
- **THEN** each of the Wiki, Goals, and Quiz tabs SHALL render an indicator stating "auto-refresh disabled" together with the failure reason

#### Scenario: No automatic retry

- **GIVEN** the auto-refresh-disabled indicator is showing for vault V
- **WHEN** any time passes while V's Workspace remains mounted
- **THEN** the frontend SHALL NOT invoke `start_vault_watcher(V)` again until the user manually leaves and re-enters the Workspace

<!-- @trace
source: codebus-fs-watcher
updated: 2026-05-20
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src-tauri/src/watcher.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - Cargo.toml
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/hooks/useWatcherEvent.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/lib.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/src/components/workspace/WatcherStatusBanner.tsx
  - codebus-app/src/store/vault-watcher-status.ts
  - codebus-app/src-tauri/src/ipc/mod.rs
tests:
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/hooks/useWatcherEvent.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/vault-watcher-status.test.ts
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
-->

---
### Requirement: Open Wiki Page In Obsidian

The system SHALL let the user open the currently-previewed wiki page in Obsidian directly from the codebus-app Wiki tab. Because codebus-app creates and binds vaults WITHOUT performing Obsidian registration (the app's init path passes `no_obsidian_register: true` and the app has no equivalent of the CLI's init-time registration), the app SHALL ensure registration at vault view time via the `get_obsidian_vault_id` probe described below, rather than assuming `codebus init` already registered the vault. This requirement defines two Tauri IPC commands (a register-and-resolve probe and an open action) and the WikiPreview button that drives them. These commands are defined separately from the `Tauri IPC Commands for Goal Lifecycle and Wiki Read` requirement, following the same precedent by which the chat-turn lifecycle commands live in their own requirement.

#### IPC command: get_obsidian_vault_id

`get_obsidian_vault_id(vault_path: String) -> Result<Option<String>, AppError>` SHALL, in order:

1. Attempt to register `<vault_path>/.codebus/wiki` into the user-level `obsidian.json` by calling `codebus_core::vault::obsidian_register::register_vault`. This step SHALL be idempotent (re-registering an already-present vault updates the existing entry's timestamp and SHALL NOT create a duplicate entry) and fail-soft (a `RegisterOutcome::ObsidianNotInstalled` or `RegisterOutcome::IoError` SHALL NOT abort the command, SHALL NOT surface an error, and SHALL leave step 2 to report the resulting state). When Obsidian is not installed (config dir absent) no file SHALL be written.
2. Resolve the Obsidian vault id for the wiki directory by calling `codebus_core::vault::obsidian_register::lookup_vault_id(<vault_path>/.codebus/wiki)`.

The result mapping (from step 2) SHALL be:

- `Ok(Some(id))` from the core helper → `Ok(Some(id))` (the 16-char SHA-256 prefix Obsidian uses as the vault key). After step 1 succeeds for an installed Obsidian, a previously-unregistered vault SHALL resolve to `Some(id)` on this same call.
- `Ok(None)` (no `obsidian.json`, Obsidian config dir absent, or no entry matches the wiki path) → `Ok(None)`.
- `Err(io_error)` (the `obsidian.json` exists but cannot be read or parsed) → `Err(AppError)` — a fail-soft signal the frontend treats identically to `None` (button hidden), never a hard crash.

The registration in step 1 is the universal touchpoint that makes the button work for BOTH newly-created and pre-existing vaults: the frontend wiki store calls this command whenever a vault's wiki is loaded, so any vault the user views is registered (or refreshed) at that moment without requiring a re-init.

#### IPC command: open_wiki_in_obsidian

`open_wiki_in_obsidian(vault_path: String, slug: String) -> Result<(), AppError>` SHALL perform the following steps in order:

1. Resolve the vault id via `lookup_vault_id`. When it resolves to `None`, the command SHALL return `AppError::Invalid { field: "obsidian", message: <vault-not-registered message> }` and SHALL NOT attempt to open anything.
2. Locate the wiki file whose filename stem (without `.md`) equals `slug`, by scanning `<vault_path>/.codebus/wiki/**/*.md`. When no file matches, the command SHALL return `AppError::Invalid { field: "slug", message: <no-such-page message> }`.
3. Compute the file path relative to `<vault_path>/.codebus/wiki/`, normalize path separators to forward slashes, and percent-encode each path segment.
4. Construct the URL `obsidian://open?vault=<id>&file=<rel>` where `<id>` is the resolved vault id and `<rel>` is the encoded relative path including the `.md` extension.
5. Open the URL via the tauri-plugin-opener Rust API. When the opener call fails, the command SHALL return `AppError`.

The relative-path + URL construction SHALL be a pure, separately-unit-testable function so the URL string can be asserted without spawning Obsidian. The command SHALL re-resolve the vault id on every invocation rather than accepting a caller-supplied id, so a vault that becomes unregistered while the app is open is detected at action time.

#### WikiPreview button

The Wiki preview footer action area (the same area that hosts `[Quiz me on this]`) SHALL render an `[Open in Obsidian]` button when, and only when, the wiki store's cached Obsidian vault id is non-null. The store SHALL fetch the vault id once via `get_obsidian_vault_id` when a vault's wiki is loaded and clear it on reset. The button SHALL render for both content pages and nav pages (`index.md` / `log.md`) — unlike `[Quiz me on this]` which renders only for content pages. Clicking the button SHALL invoke `open_wiki_in_obsidian(vault_path, current_slug)` exactly once with the currently-previewed page's slug.

When the cached vault id is null (vault not registered, or the probe returned an error), the button SHALL NOT be present in the DOM at all (hidden, not disabled).

#### Scenario: get_obsidian_vault_id registers an unregistered vault then returns Some

- **WHEN** Obsidian is installed (config dir present) AND the vault's wiki path is NOT yet in `obsidian.json` AND the frontend calls `invoke("get_obsidian_vault_id", { vault_path })`
- **THEN** the command SHALL register the wiki path into `obsidian.json` AND return `Ok(Some(<id>))` where `<id>` is the 16-char vault key for that path

#### Scenario: get_obsidian_vault_id registration is idempotent

- **WHEN** the frontend calls `invoke("get_obsidian_vault_id", { vault_path })` twice for the same vault while Obsidian is installed
- **THEN** both calls SHALL return `Ok(Some(<id>))` with the same `<id>` AND `obsidian.json` SHALL contain exactly one entry for that wiki path (the second call updates the timestamp, not a duplicate)

#### Scenario: get_obsidian_vault_id returns Some for a registered vault

- **WHEN** the frontend calls `invoke("get_obsidian_vault_id", { vault_path })` AND the user's `obsidian.json` contains an entry whose path matches `<vault_path>/.codebus/wiki`
- **THEN** the command SHALL return `Ok(Some(<id>))` where `<id>` is the 16-char vault key

#### Scenario: get_obsidian_vault_id returns None and writes nothing when Obsidian not installed

- **WHEN** the Obsidian config dir is absent AND the frontend calls `invoke("get_obsidian_vault_id", { vault_path })`
- **THEN** the command SHALL return `Ok(None)` AND SHALL NOT create or write any `obsidian.json` file (the button stays hidden, no regression for users without Obsidian)

#### Scenario: get_obsidian_vault_id maps a parse failure to AppError (fail-soft)

- **WHEN** `obsidian.json` exists but cannot be parsed as JSON AND the frontend calls `invoke("get_obsidian_vault_id", { vault_path })`
- **THEN** the command SHALL return `Err(AppError)` AND the frontend SHALL treat this identically to `None` (the Open in Obsidian button SHALL NOT render)

#### Scenario: open_wiki_in_obsidian builds the id-based URL for a sub-folder page

- **WHEN** the frontend calls `invoke("open_wiki_in_obsidian", { vault_path, slug: "uv-lib" })` AND the page lives at `<vault_path>/.codebus/wiki/modules/uv-lib.md` AND the vault id resolves to `abc123def456abcd`
- **THEN** the command SHALL open the URL `obsidian://open?vault=abc123def456abcd&file=modules/uv-lib.md`

##### Example: relative path + encoding cases

| slug | abs wiki path (under `<vault>/.codebus/wiki/`) | `file=` value |
| --- | --- | --- |
| `uv-lib` | `modules/uv-lib.md` | `modules/uv-lib.md` |
| `project-purpose` | `concepts/project-purpose.md` | `concepts/project-purpose.md` |
| `index` | `index.md` | `index.md` |
| `授權流程` | `processes/授權流程.md` | `processes/%E6%8E%88%E6%AC%8A%E6%B5%81%E7%A8%8B.md` |

#### Scenario: open_wiki_in_obsidian rejects an unregistered vault

- **WHEN** the frontend calls `invoke("open_wiki_in_obsidian", { vault_path, slug })` AND `lookup_vault_id` resolves to `None`
- **THEN** the command SHALL return `AppError::Invalid { field: "obsidian", .. }` AND SHALL NOT attempt to open any URL

#### Scenario: open_wiki_in_obsidian rejects an unknown slug

- **WHEN** the frontend calls `invoke("open_wiki_in_obsidian", { vault_path, slug: "no-such-page" })` AND no wiki file has that filename stem
- **THEN** the command SHALL return `AppError::Invalid { field: "slug", .. }`

#### Scenario: Button renders for both content and nav pages when vault id is present

- **WHEN** the wiki store's cached Obsidian vault id is non-null AND the preview shows a content page OR a nav page (`index.md` / `log.md`)
- **THEN** the `[Open in Obsidian]` button SHALL be present in the footer action area in all of those cases (whereas `[Quiz me on this]` renders only on content pages)

#### Scenario: Button hidden when vault id is null

- **WHEN** the wiki store's cached Obsidian vault id is null (vault not registered after the probe, or the probe returned an error)
- **THEN** the `[Open in Obsidian]` button SHALL NOT be present in the DOM


<!-- @trace
source: app-obsidian-register-on-open
updated: 2026-05-22
code:
  - codebus-app/src-tauri/src/ipc/wiki.rs
-->

---
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

<!-- @trace
source: quiz-double-spawn-guard
updated: 2026-05-21
code:
  - docs/2026-05-21-chat-display-polish-backlog.md
  - docs/BACKLOG.md
  - docs/2026-05-21-cli-wikilink-link-target-backlog.md
-->

---
### Requirement: Activity Stream Shell Command Wrapper Extraction

The codebus-app activity stream renderer SHALL display Shell tool invocations using the inner command the user authored, not the OS-specific wrapper the agent runtime wraps it in. When the raw `command` field of a Shell tool invocation matches a recognized wrapper shape, the renderer SHALL extract and display the inner command verbatim; the wrapper prefix and any surrounding quotes SHALL NOT count against the display character budget.

The three recognized wrapper shapes SHALL be:

1. **PowerShell wrapper** — a path ending in `powershell.exe` (case-insensitive, with or without surrounding quotes), optionally followed by zero or more leading PowerShell switch flags (each shaped `-<word>`, e.g. `-NoProfile`, `-NoLogo`, `-NonInteractive`), then `-Command` (case-insensitive), then the inner command (optionally enclosed in single or double quotes). The path MAY be a Windows absolute path containing spaces (e.g., `C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`). Real-world Codex sandbox invocations have been observed using both the bare `-Command` and the `-NoProfile -Command` forms; both SHALL be stripped.
2. **POSIX shell -c wrapper** — `sh` or `bash` (with or without a leading absolute path such as `/bin/`), followed by `-c`, followed by the inner command (optionally enclosed in single or double quotes).
3. **No wrapper recognized** — the raw command is passed through unchanged.

After extraction, the renderer SHALL truncate the displayed inner command to a maximum of 80 visible characters (matching the existing `summarizeToolInput` truncation cap), appending an ellipsis when truncation occurs. The truncation cap SHALL be applied to the extracted inner command, not to the raw wrapped command.

The extraction SHALL NOT mutate the underlying tool-use event payload (the raw wrapped command remains available in the per-run events.jsonl and in any debug / verbose surface).

#### Scenario: PowerShell wrapper is stripped before truncation

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "Get-Content package.json | Select-Object -First 50"`
- **THEN** the displayed command SHALL begin with `Get-Content package.json` AND SHALL NOT contain `powershell.exe` or `-Command` AND SHALL NOT have been truncated within the wrapper prefix

#### Scenario: PowerShell wrapper with leading switch flags is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-ChildItem -Recurse -File wiki"`
- **THEN** the displayed command SHALL begin with `Get-ChildItem -Recurse -File wiki` AND SHALL NOT contain `powershell.exe` or `-NoProfile` or `-Command`

#### Scenario: PowerShell wrapper with multiple leading switch flags is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `powershell.exe -NoLogo -NonInteractive -NoProfile -Command "Get-Date"`
- **THEN** the displayed command SHALL begin with `Get-Date` AND SHALL NOT contain any of `powershell.exe`, `-NoLogo`, `-NonInteractive`, `-NoProfile`, or `-Command`

#### Scenario: PowerShell wrapper around a multi-line here-string inner command is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is a PowerShell wrapper whose inner command is a PowerShell here-string (begins with `@'`, ends with `'@`, contains newlines), e.g. `"…\powershell.exe" -Command "@'<NL>line 1<NL>line 2<NL>'@"`
- **THEN** the inner command SHALL still be extracted (the renderer SHALL tolerate newlines inside the inner command, not stop at the first line break)

#### Scenario: POSIX sh -c wrapper is stripped before truncation

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `/bin/sh -c "git log --oneline -n 20"`
- **THEN** the displayed command SHALL begin with `git log --oneline -n 20` AND SHALL NOT contain `/bin/sh` or `-c`

#### Scenario: bash -c wrapper is stripped

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `bash -c 'grep -r "AppShell" src/'`
- **THEN** the displayed command SHALL begin with `grep -r` AND SHALL NOT contain `bash -c`

#### Scenario: Unrecognized command passes through unchanged

- **WHEN** the renderer receives a Shell tool invocation whose `command` is `git status --short` (no wrapper)
- **THEN** the displayed command SHALL be `git status --short`

#### Scenario: Inner command exceeding 80 chars is truncated after extraction

- **WHEN** the renderer receives a Shell tool invocation whose `command` wraps a 200-character inner command with a PowerShell wrapper
- **THEN** the displayed command SHALL contain the first 80 characters of the extracted inner command followed by an ellipsis AND SHALL NOT contain any portion of the wrapper prefix

##### Example: wrapper-detection table

| Raw `command` | Displayed (post-extraction, pre-truncation) |
|---|---|
| `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "Get-Date"` | `Get-Date` |
| `"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-ChildItem"` | `Get-ChildItem` |
| `powershell.exe -NoLogo -NonInteractive -NoProfile -Command "Get-Date"` | `Get-Date` |
| `powershell.exe -Command "ls D:\"` | `ls D:\` |
| `/bin/sh -c "echo hi"` | `echo hi` |
| `bash -c 'cat foo.txt'` | `cat foo.txt` |
| `sh -c "ls -la"` | `ls -la` |
| `git status` | `git status` |


<!-- @trace
source: critical-bugs-ql1-x1-qgen1
updated: 2026-05-26
code:
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
-->

---
### Requirement: Activity Stream Internal Sentinel Marker Filter

The codebus-app activity stream renderer SHALL NOT render internal `[CODEBUS_*]` sentinel markers as raw user-facing text. These markers are an agent ↔ codebus-core wire protocol (e.g., `[CODEBUS_QUIZ_SCOPE]`, `[CODEBUS_QUIZ_NO_MATCH]`, `[CODEBUS_QUIZ_NO_VALIDATE]`, `[CODEBUS_QUIZ_VIOLATION]`) and exposing them raw produces text that reads as a defect to end users.

When a thought block's text begins with a `[CODEBUS_*]` token (an opening `[`, the literal `CODEBUS_`, an uppercase ASCII / underscore identifier, a closing `]`), the renderer SHALL apply the following display rules:

1. When the marker has a registered user-facing translation (sourced from `codebus-app/src/i18n/messages.ts` under a key namespaced by the marker name), the renderer SHALL render the translated text in the active locale (zh-tw / en) in place of the raw marker-prefixed line. The remainder of the marker's payload MAY be appended after the translation when it carries information meaningful to the user (e.g., a reason string).
2. When the marker is `[CODEBUS_*]` but has no registered translation, the renderer SHALL suppress the marker-prefixed line entirely (render nothing for that thought block). The renderer SHALL NOT render the literal `[CODEBUS_…]` substring as user-facing text under any fallback path.

The first registered translation SHALL be `[CODEBUS_QUIZ_NO_VALIDATE]` with zh-tw value `codex 沙箱無法跑 quiz 結構驗證，跳過此步` and a matching en value. Future markers MAY be added to the same registry without renderer changes.

The filter SHALL apply only when the marker begins the thought block's text (after optional leading whitespace). A marker appearing mid-sentence inside a longer thought block SHALL NOT trigger suppression (such occurrences are out of scope for this requirement; they have not been observed in practice and conservative non-suppression preserves user-visible content).

The filter SHALL NOT mutate the underlying stream event payload (the raw marker text remains available in the per-run events.jsonl).

#### Scenario: Known marker is replaced by translated user-facing text

- **GIVEN** the active locale is zh-tw
- **WHEN** the renderer receives a thought block whose text is `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation`
- **THEN** the rendered output SHALL contain `codex 沙箱無法跑 quiz 結構驗證，跳過此步` AND SHALL NOT contain the literal substring `[CODEBUS_QUIZ_NO_VALIDATE]`

#### Scenario: Unknown marker is suppressed entirely

- **WHEN** the renderer receives a thought block whose text is `[CODEBUS_FUTURE_MARKER] some payload codebus-app has never seen`
- **THEN** the rendered output for this thought block SHALL be empty AND SHALL NOT contain the literal substring `[CODEBUS_FUTURE_MARKER]`

#### Scenario: Thought block without a leading marker is unaffected

- **WHEN** the renderer receives a thought block whose text is `I will start by reading README.md to understand the project structure.`
- **THEN** the rendered output SHALL be the thought text verbatim AND the filter SHALL NOT alter it

#### Scenario: Mid-sentence marker is not suppressed

- **WHEN** the renderer receives a thought block whose text is `The agent emitted [CODEBUS_QUIZ_SCOPE] wiki/a.md as its first line.`
- **THEN** the rendered output SHALL contain the thought verbatim including the literal `[CODEBUS_QUIZ_SCOPE]` substring (the filter only triggers when the marker begins the block)

##### Example: marker-handling table

| Locale | Raw thought text | Rendered output |
|---|---|---|
| zh-tw | `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation` | `codex 沙箱無法跑 quiz 結構驗證，跳過此步` |
| en | `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation` | (en translation registered under the same i18n key) |
| zh-tw | `[CODEBUS_UNKNOWN_MARKER] payload` | (empty — suppressed) |
| zh-tw | `Reading README.md first.` | `Reading README.md first.` |

<!-- @trace
source: critical-bugs-ql1-x1-qgen1
updated: 2026-05-26
code:
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
-->

---
### Requirement: Workspace Sidebar Nav Row Visual Contract

The Workspace sidebar SHALL render each of its three tab navigation rows (`Goals`, `Wiki`, `Quiz`) as a single horizontal row composed, in order, of:

1. an inline emoji prefix rendered inside `<span aria-hidden="true">` (🚏 for `Goals`, 📂 for `Wiki`, 🎓 for `Quiz`),
2. the localized tab label,
3. a right-aligned mono-numeric count rendered in a tabular-nums monospace style with tertiary foreground color, and
4. an ambient `active-pulse` dot element rendered immediately after the count, anchored to the row's right edge.

The emoji prefix SHALL be encoded directly in the component source, SHALL NOT be sourced from an i18n message value, and SHALL be visually separated from the label by a fixed gap (not by a literal whitespace character inside the label string).

The mono count SHALL display `0` as a literal numeric `0` when the underlying store is empty or still loading; it SHALL NOT be hidden, suppressed, or replaced with a placeholder character in that case.

The count source for each row SHALL be read from a global store via a selector. Specifically: `Goals` count SHALL be `useGoalsStore().runs.length`, `Wiki` count SHALL be `useWikiStore().pages.length`, and `Quiz` count SHALL be derived from a dedicated quiz-history store whose `attempts` collection is loaded and reset on Workspace mount / unmount and is kept in sync via the `quiz-changed` watcher event (the same channel `QuizTab` already subscribes to). Counts SHALL NOT be passed into the sidebar via component props (no prop drilling).

The currently active nav row SHALL display a 2px-wide vertical accent-color bar at its left edge as the primary "you are here" indicator. Non-active rows SHALL NOT render this bar (zero-opacity placeholders are not permitted). The active row's prior whole-row accent-tint fill (`bg-accent/20 text-accent`) SHALL be removed or weakened so the left bar is the dominant active-state signal; any residual active-label emphasis (color, weight, or tint) SHALL remain subtle enough that it does not compete with the left bar.

Each nav row SHALL render an `active-pulse` element with `data-testid="workspace-tab-<id>-active-pulse"` (e.g., `workspace-tab-goals-active-pulse`). The element SHALL be a 7px round accent-coloured dot, always mounted (so its 200ms opacity transition can play in both directions), and SHALL toggle between `opacity-100` (active) and `opacity-0` (inactive) via a CSS class change. The element SHALL carry the Tailwind classes `transition-opacity duration-200 motion-reduce:transition-none` so reduced-motion users see an instant transition rather than a fade.

The `Goals` row's `active-pulse` SHALL be opacity-100 (visible) if and only if `useGoalsStore.activeRun != null`. The `Wiki` row's `active-pulse` AND the `Quiz` row's `active-pulse` SHALL remain opacity-0 at all times in the current spec — those rows do not yet have a cross-tab activity signal wired (they SHALL exist in the DOM purely as a layout-stable placeholder so future activity signals can be added without re-architecting the row). The `Wiki` AND `Quiz` pulse dots SHALL carry `aria-hidden="true"` while inactive AND SHALL NOT carry an `aria-label` attribute.

When a row's `active-pulse` is visible (opacity-100), the dot SHALL carry `role="status"` AND an `aria-label` resolving to a localized message tied to that row's activity. For the `Goals` row this label SHALL resolve to the `workspace.tab.goals.activeRunPulse` i18n key (which MUST exist in every shipped locale, currently `en` AND `zh`). When the row's `active-pulse` is hidden (opacity-0), the dot SHALL carry `aria-hidden="true"` AND SHALL NOT carry an `aria-label` so screen readers do not announce a non-existent activity.

The `Goals` row's `active-pulse` SHALL be the relocated home for the ODI-4 active-goal ambient indicator, which previously lived on the Chat Widget's bubble surface. The previous chat-bubble pulse dot (`data-testid="chat-widget-active-goal-pulse"`) SHALL no longer be rendered in any chat widget mode — the chat surface is reserved for chat-state signals (promote badge, transcript content) so users do not misread a chat-located indicator as a chat-state signal.

Keyboard focus rings, hover affordances, and the existing `data-testid="workspace-tab-<id>"` and `data-active` attributes on each row SHALL be preserved.

#### Scenario: Each nav row renders emoji prefix, label, right-aligned count, and active-pulse placeholder

- **WHEN** the user opens a vault and the Workspace sidebar renders
- **THEN** each of the three nav rows displays its emoji prefix (🚏 / 📂 / 🎓) inside an `aria-hidden` span, followed by the localized label, followed by a right-aligned mono-numeric count whose value matches the corresponding store length, followed by an `active-pulse` dot element whose `data-testid` is `workspace-tab-<id>-active-pulse`

##### Example: row composition

| Tab id | Emoji | Label (en) | Count source | Active-pulse source |
| ------ | ----- | ---------- | ------------ | ------------------- |
| `goals` | 🚏 | `Goals` | `useGoalsStore().runs.length` | `useGoalsStore().activeRun != null` |
| `wiki` | 📂 | `Wiki` | `useWikiStore().pages.length` | always opacity-0 (placeholder for future signal) |
| `quiz` | 🎓 | `Quiz` | `useQuizHistoryStore().attempts.length` | always opacity-0 (placeholder for future signal) |

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

#### Scenario: Goals row active-pulse appears while a goal run is in flight

- **WHEN** `useGoalsStore.activeRun` transitions from null to a non-null value
- **THEN** the element with `data-testid="workspace-tab-goals-active-pulse"` SHALL carry the Tailwind class `opacity-100` AND `bg-accent` AND its `aria-label` SHALL resolve to the localized translation of `workspace.tab.goals.activeRunPulse` AND its `role` SHALL be `"status"`

#### Scenario: Goals row active-pulse disappears when the run ends

- **WHEN** `useGoalsStore.activeRun` transitions from a non-null value back to null
- **THEN** the element with `data-testid="workspace-tab-goals-active-pulse"` SHALL keep its place in the DOM (always-mounted contract) AND SHALL carry the Tailwind class `opacity-0` AND SHALL carry `aria-hidden="true"` AND SHALL NOT carry an `aria-label`

#### Scenario: Active-pulse fade uses motion-reduce variant

- **WHEN** the user agent reports `prefers-reduced-motion: reduce` AND `useGoalsStore.activeRun` transitions between null and non-null
- **THEN** the `workspace-tab-goals-active-pulse` element SHALL reach its target opacity within the same frame (no perceptible CSS transition) because the rendered class list includes `motion-reduce:transition-none`

#### Scenario: Wiki and Quiz rows keep active-pulse as a hidden placeholder

- **WHEN** the Workspace sidebar renders AND any goal run is active
- **THEN** the elements with `data-testid="workspace-tab-wiki-active-pulse"` AND `data-testid="workspace-tab-quiz-active-pulse"` SHALL exist in the DOM AND SHALL both carry the Tailwind class `opacity-0` AND SHALL both carry `aria-hidden="true"` AND SHALL NOT carry an `aria-label`


<!-- @trace
source: chatwidget-pulse-and-goal-token-display
updated: 2026-05-28
code:
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - codebus-app/src/components/workspace/Workspace.tsx
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/src/components/workspace/ChatWidget.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
tests:
  - codebus-app/src/components/workspace/ChatWidget.test.tsx
  - codebus-app/src/i18n/chat.test.ts
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
-->

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


<!-- @trace
source: workspace-sidebar-rework
updated: 2026-05-27
code:
  - codebus-app/src/store/quiz-history.ts
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/App.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/store/quiz-history.test.ts
-->

---
### Requirement: Workspace Sidebar Section Label Policy

The Workspace sidebar nav region SHALL NOT render any section label above its three tab rows, including but not limited to a `VAULT` label, in any locale. This is a deliberate departure from the v1 design mock's `VAULT` section label and SHALL be preserved across future visual revisions until a multi-group sidebar nav is introduced.

#### Scenario: Sidebar nav has no section label above tabs

- **WHEN** the Workspace sidebar is rendered in any supported locale
- **THEN** the DOM region between the vault display-name / path block and the first nav row contains no `<div>`, `<span>`, or `<SectionLabel>` element rendering the literal text `VAULT` or any other section-label-style heading

<!-- @trace
source: workspace-sidebar-rework
updated: 2026-05-27
code:
  - codebus-app/src/store/quiz-history.ts
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/App.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/store/quiz-history.test.ts
-->

---
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

<!-- @trace
source: workspace-content-header-row
updated: 2026-05-27
code:
  - codebus-app/src/components/ui/TabContentHeader.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/hooks/useNewGoalShortcut.ts
tests:
  - codebus-app/src/hooks/useNewGoalShortcut.test.tsx
  - codebus-app/src/i18n/quiz.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/ui/TabContentHeader.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
-->

---
### Requirement: Goals List Running Row Stream Tail

The system SHALL render a single-line "stream tail" inside each `RunListItem` row whose `run.outcome` equals `"running"`. The tail SHALL display a one-line summary of the most recent non-thought `VerbEvent` received for that run on the `goal-stream` Tauri channel, derived via the shared `summarizeVerbEvent` helper. The tail SHALL render to the left of the existing relative-timestamp column AND to the right of the truncated goal-text column. The tail element SHALL carry `data-testid="run-row-tail"` AND the CSS class set `font-mono text-meta text-fg-secondary tabular-nums truncate`. When no non-thought event has yet been received for the run (i.e., the `tailByRunId` slot is undefined for that run id), the tail SHALL render the i18n string for key `workspace.goals.runningTailPending`. When `run.outcome` is NOT `"running"`, the row SHALL NOT render the tail element at all (the `data-testid="run-row-tail"` element SHALL be absent from the DOM for non-running rows).

#### Scenario: Running row shows tail derived from latest tool_use event

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `useGoalsStore.tailByRunId[run.run_id]` holds a `VerbEvent` with shape `{ kind: "stream", data: { kind: "tool_use", name: "Read", input: { file_path: "raw/code/auth.rs" } } }`
- **THEN** the row's `data-testid="run-row-tail"` element SHALL be present AND its text content SHALL contain the string `Read` AND the string `auth.rs`

#### Scenario: Running row shows tail derived from Write event

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `tailByRunId[run.run_id]` holds a `VerbEvent` with shape `{ kind: "stream", data: { kind: "tool_use", name: "Write", input: { file_path: "wiki/modules/auth.md" } } }`
- **THEN** the row's tail element text content SHALL contain the string `wiki/modules/auth.md`

#### Scenario: Running row shows tail derived from banner event

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `tailByRunId[run.run_id]` holds a `VerbEvent` with shape `{ kind: "banner", data: { kind: "sync_start" } }`
- **THEN** the row's tail element text content SHALL equal the i18n value for key `workspace.activity.banner.syncStart`

#### Scenario: Running row shows placeholder when tail slot is empty

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"running"` AND `useGoalsStore.tailByRunId[run.run_id]` is undefined
- **THEN** the row's `data-testid="run-row-tail"` element SHALL be present AND its text content SHALL equal the i18n value for key `workspace.goals.runningTailPending`

#### Scenario: Non-running row omits tail element entirely

- **WHEN** the Goals list contains a row with `run.outcome` equal to `"succeeded"` AND `useGoalsStore.tailByRunId[run.run_id]` holds a `VerbEvent` value (tail slot retained after the run terminated)
- **THEN** the row SHALL NOT contain a `data-testid="run-row-tail"` element


<!-- @trace
source: goals-running-row-stream-tail
updated: 2026-05-27
code:
  - codebus-app/src/lib/streamEventSummary.ts
  - codebus-app/src/hooks/useLatestStreamEvent.ts
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/store/goals.ts
tests:
  - codebus-app/src/hooks/useLatestStreamEvent.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/lib/streamEventSummary.test.ts
  - codebus-app/src/store/goals.test.ts
-->

---
### Requirement: useGoalsStore Tracks Latest Stream Event Per Run

The system SHALL maintain `useGoalsStore.tailByRunId` typed as `Record<string, VerbEvent>` recording the most recently received non-thought `VerbEvent` per `run_id`. The store's existing `_onStreamEvent` handler SHALL, in addition to its existing `activeRun.events` append behavior, write the incoming `VerbEvent` to `tailByRunId[payload.run_id]` whenever the event is NOT a thought event (i.e., NOT `{ kind: "stream", data: { kind: "thought" } }`). The store SHALL accept stream events for any `run_id`, including run ids not present in `activeRun` (terminal-spawned goals whose `_onStreamEvent` arrives without a matching `activeRun.runId`). The store's `_onTerminal` handler SHALL NOT clear `tailByRunId[payload.run_id]` — the entry SHALL remain in the map after the run terminates. The store's `reset()` method SHALL clear `tailByRunId` to an empty object alongside its existing `activeRun` and `runs` clearing.

#### Scenario: Stream event for active run writes to tailByRunId

- **GIVEN** `tailByRunId` is initially empty AND `activeRun.runId` equals `"run-A"`
- **WHEN** `_onStreamEvent` receives payload with `run_id` equal to `"run-A"` AND event `{ kind: "stream", data: { kind: "tool_use", name: "Read", input: { file_path: "x" } } }`
- **THEN** after the call `tailByRunId["run-A"]` SHALL equal that `VerbEvent` AND `activeRun.events` SHALL also contain the event (unchanged from prior behavior)

#### Scenario: Stream event for terminal-spawned goal writes to tailByRunId even when activeRun is null

- **GIVEN** `activeRun` is `null` AND `tailByRunId` is empty
- **WHEN** `_onStreamEvent` receives payload with `run_id` equal to `"run-B"` AND event `{ kind: "banner", data: { kind: "start", repo_path: "/v" } }`
- **THEN** `tailByRunId["run-B"]` SHALL equal that `VerbEvent` AND `activeRun` SHALL remain `null`

#### Scenario: Thought event does not write to tailByRunId

- **GIVEN** `tailByRunId["run-A"]` holds a prior tool_use event `e_prev`
- **WHEN** `_onStreamEvent` receives payload with `run_id` equal to `"run-A"` AND event `{ kind: "stream", data: { kind: "thought", text: "..." } }`
- **THEN** `tailByRunId["run-A"]` SHALL remain `e_prev` (unchanged)

#### Scenario: Terminal event preserves tail slot

- **GIVEN** `tailByRunId["run-A"]` holds a `VerbEvent` `e` AND `activeRun.runId` equals `"run-A"`
- **WHEN** `_onTerminal` receives payload with `run_id` equal to `"run-A"`
- **THEN** `activeRun` SHALL be set to `null` (unchanged from prior behavior) AND `tailByRunId["run-A"]` SHALL still equal `e` (NOT cleared)

#### Scenario: reset clears tail map

- **GIVEN** `tailByRunId` contains entries for `"run-A"` AND `"run-B"`
- **WHEN** `reset()` is called
- **THEN** `tailByRunId` SHALL equal the empty object AND `activeRun` SHALL be `null` AND `runs` SHALL be the empty array


<!-- @trace
source: goals-running-row-stream-tail
updated: 2026-05-27
code:
  - codebus-app/src/lib/streamEventSummary.ts
  - codebus-app/src/hooks/useLatestStreamEvent.ts
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/store/goals.ts
tests:
  - codebus-app/src/hooks/useLatestStreamEvent.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/lib/streamEventSummary.test.ts
  - codebus-app/src/store/goals.test.ts
-->

---
### Requirement: useLatestStreamEvent Hook Provides Per-Run Tail Access

The system SHALL expose a React hook `useLatestStreamEvent(runId: string): VerbEvent | null` in `codebus-app/src/hooks/useLatestStreamEvent.ts`. The hook SHALL subscribe to `useGoalsStore` AND return `tailByRunId[runId]` if present, else `null`. The hook SHALL use a Zustand selector that depends only on the slot for the supplied `runId` (NOT the entire `tailByRunId` record), so that stream events for OTHER run ids do NOT trigger a re-render of components consuming this hook for an unrelated `runId`.

#### Scenario: Hook returns tail value for known run id

- **GIVEN** `useGoalsStore.tailByRunId["run-A"]` holds a `VerbEvent` value `e`
- **WHEN** a component calls `useLatestStreamEvent("run-A")`
- **THEN** the hook SHALL return `e`

#### Scenario: Hook returns null for unknown run id

- **GIVEN** `useGoalsStore.tailByRunId` does NOT contain key `"run-Z"`
- **WHEN** a component calls `useLatestStreamEvent("run-Z")`
- **THEN** the hook SHALL return `null`

#### Scenario: Unrelated stream event does not re-render hook consumer

- **GIVEN** a component mounted with `useLatestStreamEvent("run-A")` AND `tailByRunId["run-A"]` holding a stable value
- **WHEN** `_onStreamEvent` writes a NEW event to `tailByRunId["run-B"]` AND `tailByRunId["run-A"]` is unchanged
- **THEN** the component's render count SHALL NOT increase


<!-- @trace
source: goals-running-row-stream-tail
updated: 2026-05-27
code:
  - codebus-app/src/lib/streamEventSummary.ts
  - codebus-app/src/hooks/useLatestStreamEvent.ts
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/store/goals.ts
tests:
  - codebus-app/src/hooks/useLatestStreamEvent.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/lib/streamEventSummary.test.ts
  - codebus-app/src/store/goals.test.ts
-->

---
### Requirement: Stream Event Summary Helper Module

The system SHALL expose a shared helper module at `codebus-app/src/lib/streamEventSummary.ts` exporting the following pure functions: `summarizeVerbEvent(event: VerbEvent, t: TFunction): string | null`, `bannerLabel(banner: VerbBanner, t: TFunction): string`, `summarizeToolInput(input: unknown): string`, `writeEditPath(input: unknown): string`, AND `extractInnerCommand(raw: string): string`. The `summarizeVerbEvent` facade SHALL return a single-line string for `VerbEvent` whose `kind` is `"banner"` AND for events whose `kind` is `"stream"` AND inner `data.kind` is `"tool_use"`; it SHALL return `null` for events whose `kind` is `"stream"` AND inner `data.kind` is `"thought"`, AND for any other event shape with no one-line summary. The `ActivityStreamItem` component SHALL consume these helpers from the new module rather than from local file-scope definitions; its rendered output SHALL be functionally equivalent to its pre-extraction behavior, verified by the existing `ActivityStreamItem.test.tsx` suite passing without modification.

#### Scenario: summarizeVerbEvent renders Write event

- **WHEN** `summarizeVerbEvent` is called with event `{ kind: "stream", data: { kind: "tool_use", name: "Write", input: { file_path: "wiki/x.md" } } }`
- **THEN** the result SHALL be a non-null string containing the substring `wiki/x.md`

#### Scenario: summarizeVerbEvent renders generic tool_use event

- **WHEN** `summarizeVerbEvent` is called with event `{ kind: "stream", data: { kind: "tool_use", name: "Read", input: { file_path: "raw/code/auth.rs" } } }`
- **THEN** the result SHALL be a non-null string containing the substring `Read` AND the substring `auth.rs`

#### Scenario: summarizeVerbEvent returns null for thought event

- **WHEN** `summarizeVerbEvent` is called with event `{ kind: "stream", data: { kind: "thought", text: "..." } }`
- **THEN** the result SHALL be `null`

#### Scenario: summarizeToolInput truncates long shell command to 80 chars

- **GIVEN** an 800-character shell command string `cmd`
- **WHEN** `summarizeToolInput` is called with `{ command: cmd }`
- **THEN** the result SHALL have length 80 AND SHALL end with the character `…` (U+2026)

##### Example: shell command extraction and truncation

| Input.command                              | Expected output | Notes                          |
| ------------------------------------------ | --------------- | ------------------------------ |
| `bash -c "echo hi"`                        | `echo hi`       | sh -c wrapper stripped         |
| `powershell.exe -NoProfile -Command "ls"`  | `ls`            | PowerShell wrapper stripped    |
| 800-char raw command                       | 79 chars + `…`  | length 80 with trailing ellipsis |
| `git status`                               | `git status`    | no wrapper, passthrough        |


<!-- @trace
source: goals-running-row-stream-tail
updated: 2026-05-27
code:
  - codebus-app/src/lib/streamEventSummary.ts
  - codebus-app/src/hooks/useLatestStreamEvent.ts
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/store/goals.ts
tests:
  - codebus-app/src/hooks/useLatestStreamEvent.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/lib/streamEventSummary.test.ts
  - codebus-app/src/store/goals.test.ts
-->

---
### Requirement: i18n Key for Running Tail Pending Placeholder

The system SHALL define a new i18n key `workspace.goals.runningTailPending` in BOTH the `messages.en` AND `messages.zh` bundles in `codebus-app/src/i18n/messages.ts`. The value SHALL be the single Unicode horizontal ellipsis character `…` (U+2026) in BOTH bundles. The key SHALL be enumerated in the workspace i18n bundle test (`codebus-app/src/i18n/workspace.test.ts`) so the existing bundle-parity check catches any missing translation.

#### Scenario: Both bundles define the key with the ellipsis value

- **WHEN** the test inspects `messages.en["workspace.goals.runningTailPending"]` AND `messages.zh["workspace.goals.runningTailPending"]`
- **THEN** both values SHALL equal the string `"…"` (single character, U+2026)

<!-- @trace
source: goals-running-row-stream-tail
updated: 2026-05-27
code:
  - codebus-app/src/lib/streamEventSummary.ts
  - codebus-app/src/hooks/useLatestStreamEvent.ts
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/store/goals.ts
tests:
  - codebus-app/src/hooks/useLatestStreamEvent.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/lib/streamEventSummary.test.ts
  - codebus-app/src/store/goals.test.ts
-->

---
### Requirement: Activity Stream Two-Phase Cluster Rendering

The Activity stream block in the Run Detail Views — Running, Done, Interrupted, and Failed states SHALL wrap consecutive tool-use events into semantic phase clusters according to the design v1.5 specification. Two cluster phases SHALL exist:

- **READING CODEBASE** — tool calls that intake information without mutating state. The phase SHALL contain `Read`, `Glob`, `Grep`, `ToolUse` events whose `tool_kind` is `read` or `inspect`, AND any future tool whose `tool_kind` is `other_read`.
- **WRITING WIKI** — tool calls that mutate state. The phase SHALL contain `Write`, `Edit`, `ToolUse` events whose `tool_kind` is `mutation`, AND any future tool whose `tool_kind` is `other_write`.

Each cluster SHALL render as a single collapsible row containing a heading, a count of contained tool-use events, and (when expanded) the child rows in arrival order using the existing per-event renderer.

Cluster boundary rules SHALL be:

- Cluster boundary opens when the first qualifying tool-use event of a phase arrives after either the start of the stream or a non-tool-use event (banner or thought).
- Cluster boundary closes when any of the following arrives: a banner event, a thought block, OR a tool-use event whose phase classification differs from the open cluster.
- Clusters SHALL be allowed to repeat across the timeline (e.g. `thought → reading → thought → reading → writing` is a legal sequence where two distinct READING CODEBASE clusters are rendered with a thought block between them).
- The cluster count SHALL only include rendered tool-use rows; thought blocks and banners SHALL NOT be counted.
- Tool-use events whose `tool_kind` is missing (`None` / `undefined`) SHALL be classified as `Inspect` for cluster purposes and grouped under READING CODEBASE.

Cluster collapsible default state SHALL track the run status: a cluster SHALL render expanded by default when the surrounding run is still `running`, AND SHALL render collapsed by default once the run reaches any terminal state (`done`, `interrupted`, `failed`). The user SHALL be able to toggle a cluster open or closed by activating its heading; the heading SHALL be a `<button>` carrying `aria-expanded` and `aria-controls` attributes.

Cluster heading icon prefixes SHALL use mono ASCII / single glyph form (NOT brand emoji) per design v1.5 lock:

| Tool / kind | Icon |
| --- | --- |
| `Read` | `📄` |
| `Glob` | `🗂` |
| `Grep` | `🔍` |
| `Shell` with `tool_kind: read` | `$_` |
| `Shell` with `tool_kind: inspect` | `$?` |
| `Write` / `Edit` | `✎` |
| `Shell` with `tool_kind: mutation` | `$!` |

Cluster heading label SHALL be localized:

- READING CODEBASE heading (en): `Reading codebase`; (zh): `讀檔案`
- WRITING WIKI heading (en): `Writing wiki`; (zh): `寫 wiki`

Cluster collapsed summary, rendered only after the run reaches a terminal state, SHALL include counts in the localized form:

- READING (en): `Reading codebase · {reads} reads · {shell} shell · {elapsedSeconds}s`
- READING (zh): `讀檔案 {reads} 次 · shell {shell} 次 · {elapsedSeconds} 秒`
- WRITING (en): `Writing wiki · {new} new · {updated} updated · {elapsedSeconds}s`
- WRITING (zh): `新增 {new} · 更新 {updated} · {elapsedSeconds} 秒`

Cluster wrapping SHALL be implemented as a pure timeline projection function (`projectClusters`) consumed by a dedicated `ActivityCluster` React component. The function SHALL NOT mutate input arrays AND SHALL be unit-testable independently of React.

#### Scenario: Consecutive read tools fold into one READING CODEBASE cluster

- **WHEN** the Activity stream receives, in order, `ToolUse(Read, file_path=a.md)`, `ToolUse(Glob, pattern=wiki/**.md)`, `ToolUse(Grep, pattern=foo)`
- **THEN** the stream SHALL render exactly one `ActivityCluster` element with phase `reading_codebase`, count `3`, AND its three child rows in arrival order

##### Example: cluster count excludes thought

- **GIVEN** event sequence `Read`, `Thought("checking ...")`, `Read`, `Read`
- **WHEN** projected through `projectClusters`
- **THEN** the result SHALL contain two `ActivityCluster` entries with counts `1` and `2` and a `thought_block` between them

#### Scenario: Phase change breaks cluster and opens a new one

- **WHEN** the Activity stream receives, in order, `ToolUse(Read)`, `ToolUse(Write, file_path=wiki/a.md)`, `ToolUse(Edit, file_path=wiki/b.md)`
- **THEN** the stream SHALL render exactly one READING CODEBASE cluster (count `1`) followed by exactly one WRITING WIKI cluster (count `2`)

#### Scenario: Banner inside cluster sequence ends the cluster

- **WHEN** the Activity stream receives, in order, `ToolUse(Read)`, `VerbBanner::commit_done { sha7 }`, `ToolUse(Read)`
- **THEN** the stream SHALL render in order: a READING CODEBASE cluster with count `1`, a `stream-banner` row for `commit_done`, then a second READING CODEBASE cluster with count `1`

#### Scenario: Cluster default open during running, collapsed when terminal

- **WHEN** the user views the Run Detail Views — Running for an in-flight run AND a READING CODEBASE cluster of three Read events is present
- **THEN** the cluster's heading SHALL carry `aria-expanded="true"` AND the three child rows SHALL be visible in the DOM
- **AND WHEN** the run transitions to the `done` terminal state and the user re-mounts the cluster element
- **THEN** the cluster's heading SHALL carry `aria-expanded="false"` AND the three child rows SHALL NOT be visible

#### Scenario: User toggles cluster open and closed

- **WHEN** the user activates the cluster heading button while it carries `aria-expanded="false"`
- **THEN** the heading attribute SHALL flip to `aria-expanded="true"` AND the child rows SHALL become visible
- **AND WHEN** the user activates the same button a second time
- **THEN** the attribute SHALL flip back to `aria-expanded="false"` AND the child rows SHALL no longer be visible

#### Scenario: Cluster heading uses mono icon and localized label

- **WHEN** a READING CODEBASE cluster is rendered in the `en` locale
- **THEN** the heading SHALL contain the string `Reading codebase` AND SHALL contain at least one of the icons `📄`, `🗂`, `🔍`, `$_`, or `$?` AND SHALL NOT contain the emoji `🛠️`

##### Example: cluster terminal summary string

| Locale | Phase | Counts | Expected summary fragment |
| --- | --- | --- | --- |
| `en` | reading | `reads=12`, `shell=195`, `elapsedSeconds=6.2` | `Reading codebase · 12 reads · 195 shell · 6.2s` |
| `zh` | reading | `reads=12`, `shell=195`, `elapsedSeconds=6.2` | `讀檔案 12 次 · shell 195 次 · 6.2 秒` |
| `en` | writing | `new=3`, `updated=2`, `elapsedSeconds=4.5` | `Writing wiki · 3 new · 2 updated · 4.5s` |
| `zh` | writing | `new=3`, `updated=2`, `elapsedSeconds=4.5` | `新增 3 · 更新 2 · 4.5 秒` |

#### Scenario: Missing tool_kind defaults to Inspect

- **WHEN** the Activity stream receives a `ToolUse(Bash, command="git status")` event whose `tool_kind` field is `undefined`
- **THEN** the event SHALL be grouped under a READING CODEBASE cluster (treated as `Inspect`) AND the frontend SHALL NOT log a console warning or error

<!-- @trace
source: activity-stream-2-phase-cluster
updated: 2026-05-27
code:
  - codebus-core/src/stream/codex_parser.rs
  - codebus-core/src/render/stream_event.rs
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/lib/clusterTimeline.ts
  - codebus-cli/src/commands/chat.rs
  - codebus-app/src/components/workspace/ActivityCluster.tsx
  - codebus-core/src/stream/parser.rs
  - codebus-app/src/lib/streamEventSummary.ts
tests:
  - codebus-app/src/components/workspace/ActivityCluster.test.tsx
  - codebus-app/src/lib/clusterTimeline.test.ts
  - codebus-app/src/lib/streamEventSummary.test.ts
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
-->

---
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


<!-- @trace
source: quiz-fullscreen-wizard-view
updated: 2026-05-27
code:
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/components/workspace/QuizWizardScopeConfirm.tsx
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/workspace/QuizWizardGenerating.tsx
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/ui/TabContentHeader.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->

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


<!-- @trace
source: quiz-fullscreen-wizard-view
updated: 2026-05-27
code:
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/components/workspace/QuizWizardScopeConfirm.tsx
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/workspace/QuizWizardGenerating.tsx
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/ui/TabContentHeader.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->

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


<!-- @trace
source: quiz-fullscreen-wizard-view
updated: 2026-05-27
code:
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src/components/workspace/QuizWizardScopeConfirm.tsx
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/workspace/QuizWizardGenerating.tsx
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/ui/TabContentHeader.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->

---
### Requirement: Wiki Page Metadata Bar

The Wiki page reader SHALL render a single-line metadata bar at the top of the preview (immediately above the markdown body) on every wiki page view. The bar SHALL be composed of up to three segments rendered in this order, separated by a middle-dot `·`:

1. **Last authoring goal** — Text `Last updated by <goal>` where `<goal>` is the last element of `PageFrontmatter.goals`. When `goals` is empty, this segment SHALL NOT be rendered. The `<goal>` value SHALL be rendered as a clickable element that, when activated, navigates the user to the Goal Detail view for that goal.
2. **Time since update** — A relative time-ago string derived from `PageFrontmatter.updated`. The time-ago value SHALL reuse the existing `common.minutesAgo` / `common.hoursAgo` / `common.daysAgo` localized strings. When `PageFrontmatter.updated` cannot be parsed into a valid date, this segment SHALL NOT be rendered.
3. **Wikilink count** — Text `<N> sources` (or localized equivalent) where `<N>` is the count of `[[wikilink]]` occurrences in the page body, computed via the same wikilink-extraction routine the renderer uses. When `<N>` is less than 1, this segment SHALL NOT be rendered.

The metadata bar SHALL NOT render tags, word counts, view counts, author lists, or any other field beyond the three segments specified above. When all three segments are suppressed, the metadata bar component SHALL render nothing (no empty bar).

#### Scenario: All three metadata segments render

- **WHEN** a wiki page with `goals: [g1, g2]`, valid `updated: 2026-05-27T12:00:00Z`, and a body containing three `[[wikilink]]` references is rendered
- **THEN** the metadata bar renders three segments separated by `·`: `Last updated by g2`, a localized time-ago string, and `3 sources`

#### Scenario: Authoring goal name is clickable

- **WHEN** the user clicks the `<goal>` token in the metadata bar
- **THEN** the workspace navigates to the Goal Detail view for that goal AND no IPC call is required to compute the click target

#### Scenario: Empty goals list suppresses first segment

- **WHEN** a wiki page with `goals: []` is rendered AND the body has two wikilinks AND `updated` is valid
- **THEN** the metadata bar renders only `<time-ago> · 2 sources` AND the `Last updated by` segment is omitted

#### Scenario: Zero wikilinks suppresses sources segment

- **WHEN** a wiki page body contains no `[[wikilink]]` references
- **THEN** the metadata bar omits the `<N> sources` segment entirely (including the leading `·` separator)

#### Scenario: All segments suppressed renders nothing

- **WHEN** a wiki page has `goals: []` AND `updated` is unparseable AND the body has no wikilinks
- **THEN** the metadata bar component renders no DOM output

##### Example: segment suppression matrix

| goals          | updated parses | wikilink count | Rendered segments                                  |
| -------------- | -------------- | -------------- | -------------------------------------------------- |
| `[g1, g2]`     | yes            | 3              | `Last updated by g2 · <time-ago> · 3 sources`      |
| `[]`           | yes            | 2              | `<time-ago> · 2 sources`                           |
| `[g1]`         | no             | 1              | `Last updated by g1 · 1 sources`                   |
| `[]`           | no             | 0              | (no DOM output)                                    |
| `[g1]`         | yes            | 0              | `Last updated by g1 · <time-ago>`                  |


<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Wiki Page Edit Hint Footer

The Wiki page reader SHALL render an edit-hint footer at the bottom of every wiki page view (rendered after the markdown body and after the existing bottom action button row). The hint SHALL be a single line of text styled in a tertiary foreground color and SHALL contain a clickable element labelled `Run a goal` (or its localized equivalent). The full hint text SHALL communicate that the user can edit the page by starting a new goal that asks codebus to change it.

Activating the clickable element SHALL open the existing New Goal Modal with a pre-filled goal description of the form `修改 wiki/<page-relative-path> — ` (Chinese form) or `Edit wiki/<page-relative-path> — ` (English form), where `<page-relative-path>` is the wiki page's path relative to the vault `.codebus/wiki/` directory including the type-folder prefix and the `.md` extension. The pre-fill SHALL end with an em-dash and a trailing space so the user can append their own description.

The edit hint footer SHALL NOT be rendered when no wiki page is currently selected.

#### Scenario: Edit hint footer renders for content pages

- **WHEN** the user opens a wiki content page (any page that is not `index.md` or `log.md`)
- **THEN** the edit hint footer is rendered below the markdown body AND below the existing action button row

#### Scenario: Activating Run a goal opens prefilled modal

- **WHEN** the user clicks the `Run a goal` element in the edit hint footer for a page whose relative path is `modules/auth-middleware.md`
- **THEN** the existing New Goal Modal opens AND its goal description is pre-filled with `修改 wiki/modules/auth-middleware.md — ` (in Chinese locale) or the English equivalent prefix, including the trailing em-dash and space


<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Wiki Page Reader Quiz Button Visual Emphasis

The `Quiz me on this` button rendered at the bottom of a wiki content page SHALL use the amber primary button variant. The `Open in Obsidian` button (when present) SHALL retain the secondary button variant. The localized label of the Quiz button SHALL match the standardized wording for the action (`Quiz this page` in English, `Quiz 這頁` in Traditional Chinese; the `Quiz` jargon term SHALL be preserved verbatim in the Chinese label).

#### Scenario: Quiz button uses amber primary variant

- **WHEN** a wiki content page renders its bottom action row with both buttons present
- **THEN** the `Quiz me on this` button uses the amber primary variant AND the `Open in Obsidian` button uses the secondary variant

#### Scenario: Chinese label preserves Quiz jargon

- **WHEN** the locale is `zh-tw`
- **THEN** the Quiz button label is `Quiz 這頁` AND the substring `Quiz` is rendered verbatim without translation


<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Wikilink Plain and Citation Style Variants

The wikilink renderer SHALL apply two distinct visual variants depending on rendering context:

- **Plain wikilink variant** — Used for resolvable wikilinks inside a wiki body. The element SHALL carry the literal CSS class name `plain-wikilink` and SHALL render with the default foreground color, an underline whose decoration color matches the strong-border token, and a 3px underline offset. On hover the element SHALL transition the text color and underline color to the accent token. Under `prefers-reduced-motion: reduce`, the hover color change SHALL be applied instantly without a CSS transition.
- **Citation wikilink variant** — Used inside citation blocks (such as quiz citation blockquotes and chat bubble citations). The element SHALL carry the literal CSS class name `cite-link` and SHALL render in a monospace font with the accent foreground color and a dashed underline at a 3px offset.

Unresolvable wikilinks (slugs that are not present in the wiki page index) SHALL continue to render as a dimmed `cursor-not-allowed` element with the `Page not found` tooltip; no new state SHALL be introduced beyond the existing resolvable / unresolvable distinction. The wikilink renderer SHALL NOT track or visualize a `visited` state.

#### Scenario: Resolvable body wikilink uses plain-wikilink class

- **WHEN** a resolvable `[[slug]]` is rendered inside a wiki body
- **THEN** the rendered anchor element has the literal CSS class `plain-wikilink` in its class list AND the anchor uses the foreground / strong-border / accent token colors specified above

#### Scenario: Citation wikilink uses cite-link class

- **WHEN** a wikilink is rendered inside a quiz citation block or a chat-bubble citation block
- **THEN** the rendered anchor element has the literal CSS class `cite-link` in its class list AND uses the monospace / accent / dashed-underline styling

#### Scenario: Reduced motion suppresses hover transition

- **WHEN** the user agent advertises `prefers-reduced-motion: reduce`
- **THEN** hovering a `plain-wikilink` element changes the color and underline color instantly without a CSS transition

#### Scenario: No visited state is rendered

- **WHEN** the user navigates to a wikilink target and later returns to a page that links to that target
- **THEN** the wikilink renders with the same `plain-wikilink` (or `cite-link`) styling as a never-clicked link AND no visited-state styling is applied


<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Wiki Tree Travel Log Footer Slot

The Wiki Tree component SHALL render a footer slot immediately below the last bucket. The footer slot SHALL contain exactly one entry labelled `Travel log` (or its localized equivalent) representing the system-generated `log.md` page. The footer slot entry SHALL be visually separated from the buckets above by a hairline top border and an 18px gap above the border. The entry SHALL render in a tertiary foreground color to distinguish it from the bucket entries.

Activating the footer slot entry SHALL invoke the same page-selection callback that bucket entries use, with the slug `log`.

When the Wiki Tree is rendered, the Wiki Index entry (slug `index`) SHALL appear at the top of the tree as the first entry, above any bucket. The previous catch-all `OTHER` bucket SHALL no longer be rendered as a bucket; the `log.md` system page that previously appeared inside `OTHER` SHALL appear only in the footer slot. Pages whose `PageFrontmatter.page_type` does not match any of the five known buckets SHALL still be reachable through the page index but SHALL NOT cause an `OTHER` bucket header to appear.

#### Scenario: Travel log footer renders below buckets

- **WHEN** the Wiki Tree mounts with at least one wiki page in the vault
- **THEN** the bottom of the tree renders an entry labelled `Travel log` (or the active locale equivalent) AND the entry is separated from the bucket list above by a hairline top border with an 18px gap

#### Scenario: Travel log entry selects log page

- **WHEN** the user clicks the `Travel log` entry in the footer slot
- **THEN** the page-selection callback fires with slug `log` AND the Wiki page reader loads the `log.md` system page

#### Scenario: Wiki Index appears at the top of the tree

- **WHEN** the Wiki Tree renders
- **THEN** the first entry in the tree is the Wiki Index (slug `index`) AND it appears above any bucket entry

#### Scenario: OTHER bucket is no longer rendered

- **WHEN** the Wiki Tree renders with pages of all five known `page_type` values
- **THEN** the tree renders the five known bucket headers AND no bucket header labelled `OTHER` is rendered


<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Wiki Page Reader Unselected Hint Card

When the Wiki tab is mounted with at least one wiki page in the vault but no page is currently selected, the Wiki page reader region SHALL render a hint card containing exactly three elements: a 36px folder emoji icon (`📂`) in a quaternary foreground color, a primary line of localized text inviting the user to pick a page, and a secondary line of localized text directing the user to the Travel log entry in the tree footer. The hint card SHALL NOT render the metadata bar, the markdown body, the action button row, or the edit hint footer.

#### Scenario: Unselected hint card renders when no page is selected

- **WHEN** the Wiki tab is mounted AND the vault contains at least one wiki page AND no page is currently selected
- **THEN** the Wiki page reader region renders a hint card with the `📂` icon, the `Pick a page to start reading.` primary line (or active locale equivalent), and the `Or click the travel log below to see what codebus has been up to.` secondary line (or active locale equivalent)

#### Scenario: Hint card does not render reader chrome

- **WHEN** the unselected hint card is showing
- **THEN** no metadata bar is rendered AND no markdown body is rendered AND no edit hint footer is rendered AND no action button row is rendered


<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Wiki Tab Empty Hero When Vault Has No Pages

When the Wiki tab is mounted and the vault contains zero wiki pages, the Wiki tab content area SHALL render a single empty hero. The empty hero SHALL contain exactly four elements stacked vertically and centered: a 56px lucide `Folder` icon, a localized hero title (`No wiki pages yet` in English or its locale equivalent), a localized subtitle inviting the user to run a goal so codebus can build the wiki, and a single amber primary call-to-action button (`→ Run a goal to start` in English or its locale equivalent). Activating the call-to-action button SHALL switch the active Workspace tab to the Goals tab AND SHALL open the existing New Goal Modal (without pre-filling the goal description).

The empty hero SHALL replace the previous single-line empty hint entirely; the previous `workspace.wiki.empty` localized hint SHALL NOT be rendered alongside the hero.

#### Scenario: Empty hero renders when vault has no wiki pages

- **WHEN** the Wiki tab mounts AND the vault contains zero wiki pages
- **THEN** the Wiki tab content area renders the 56px Folder icon AND the localized hero title AND the localized subtitle AND the amber primary call-to-action button AND no single-line hint is rendered alongside

#### Scenario: CTA switches to Goals tab and opens New Goal Modal

- **WHEN** the user clicks the empty hero call-to-action button
- **THEN** the Workspace active tab switches to `Goals` AND the existing New Goal Modal opens AND the modal's goal description field is empty (no pre-fill)

<!-- @trace
source: wiki-page-reader-v1-1
updated: 2026-05-28
code:
  - codebus-app/src/styles/globals.css
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src/components/workspace/ExplanationText.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiTab.tsx
tests:
  - codebus-app/src/components/workspace/WikiPageMetadataBar.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/workspace/ChatTranscript.test.tsx
-->

---
### Requirement: Cancel Completes Within Bounded Latency

The system SHALL ensure that a `cancel_goal` invocation against an active goal run reaches a terminal state within a bounded latency window, even when the underlying child process has stopped emitting stdout (for example, the LLM is hung on a network call, the child is waiting on a stalled tool result, or the child is otherwise blocked on I/O that does not surface through stdout). "Terminal state" SHALL mean ALL of the following:

- The entry keyed by the cancelled run's `RunId` SHALL be removed from `AppState.active_runs`.
- A final `goal-stream` event signalling termination SHALL be emitted to the frontend.
- A `spawn_end` event SHALL be appended to the run's events file.
- The runner thread driving `agent::invoke` for that run SHALL return and SHALL NOT remain blocked.

The bounded latency SHALL be at most 1 second in typical operation, measured from the instant the frontend's `cancel_goal` IPC invocation returns to the instant the frontend receives the terminal `goal-stream` event. This invariant SHALL hold regardless of whether the child was actively streaming stdout at the moment of cancellation.

This requirement does NOT introduce a force-timeout: long-running goals SHALL NOT be killed by the system on a timer. The bounded latency applies only to the cancel pathway after the user (or another `cancel_goal` caller) has explicitly requested cancellation.

This requirement strengthens the existing `One Active Goal Run At A Time` requirement's `Spawn allowed after cancel completes` scenario by guaranteeing that "cancel completes" is reachable under all child-state conditions, not only when the child happens to be emitting stdout.

#### Scenario: Cancel reaches terminal state when child is silent

- **WHEN** a goal run is active for vault `V` AND the child process has stopped emitting stdout (LLM hung, waiting on tool result, or network-blocked) AND the frontend invokes `cancel_goal`
- **THEN** within 1 second of the `cancel_goal` IPC invocation returning, the frontend SHALL receive a terminal `goal-stream` event AND `active_runs` SHALL no longer contain the cancelled run's entry AND the run's events file SHALL contain a `spawn_end` event

#### Scenario: Cancel reaches terminal state when child is streaming

- **WHEN** a goal run is active for vault `V` AND the child process is actively streaming stdout AND the frontend invokes `cancel_goal`
- **THEN** within 1 second of the `cancel_goal` IPC invocation returning, the frontend SHALL receive a terminal `goal-stream` event AND `active_runs` SHALL no longer contain the cancelled run's entry AND the run's events file SHALL contain a `spawn_end` event

#### Scenario: Subsequent spawn succeeds after silent-child cancel

- **WHEN** a goal run is cancelled per the `Cancel reaches terminal state when child is silent` scenario AND the frontend subsequently invokes `spawn_goal` for the same vault `V`
- **THEN** the new `spawn_goal` invocation SHALL succeed AND a new run id SHALL be returned

#### Scenario: No force-timeout on long-running goals

- **WHEN** a goal run has been active for any duration without cancellation AND the user has not invoked `cancel_goal`
- **THEN** the system SHALL NOT kill the child process on a timer AND the run SHALL continue until the child exits naturally or the user explicitly cancels

<!-- @trace
source: cancelling-stuck-fix
updated: 2026-05-28
code:
  - codebus-core/src/agent/process_kill.rs
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-zh-clean.png
  - codebus-core/Cargo.toml
  - codebus-core/src/agent/mod.rs
  - docs/2026-05-28-cancelling-stuck-todo.md
  - codebus-core/src/agent/claude_cli.rs
  - codebus-app/scripts/.v11-acceptance/01-lobby-bus-motion-frame.png
  - docs/2026-05-28-four-bugs-backlog.md
  - codebus-app/scripts/.v11-acceptance/01-loading-overlay/error-mode-en.png
  - docs/2026-05-28-goal-token-display-streaming-todo.md
  - docs/2026-05-28-claude-trace-prompt-analysis-todo.md
-->

---
### Requirement: Run Detail Load Failure Surfacing

The system SHALL surface a load failure when fetching a selected run's `RunDetail` (via `get_run_detail`) rejects, instead of remaining indefinitely in the loading state. When the user has selected a non-active run whose `RunDetail` has not yet loaded, the Goals content area SHALL show the loading affordance (`workspace.runDetail.loading`); when the `get_run_detail` call for that run rejects, the Goals content area SHALL render an error state that names the failure AND offers a retry action AND a path back to the Goals list. The frontend SHALL NOT silently discard the `get_run_detail` rejection (no empty catch handler) and SHALL NOT leave the user on the loading affordance after a rejection.

#### Scenario: get_run_detail rejection shows retriable error state

- **WHEN** the user is viewing a selected non-active run AND the `get_run_detail` IPC for that run rejects with an error
- **THEN** the Goals content area SHALL render an error state (not the `workspace.runDetail.loading` affordance) AND the error state SHALL expose a retry control AND a control to return to the Goals list

#### Scenario: successful load after terminal transition shows terminal detail

- **WHEN** the user is sitting in the `Running` detail of a goal AND the goal reaches a terminal outcome (so `activeRun` clears) AND `get_run_detail` for that run resolves
- **THEN** the Goals content area SHALL transition to the matching terminal view (`Done` for `succeeded`, or the interrupted/failed view) AND SHALL NOT remain on the `workspace.runDetail.loading` affordance

<!-- @trace
source: goal-run-id-unify-stuck-rundetail
updated: 2026-06-07
code:
  - codebus-core/src/verb/goal.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/i18n/messages.ts
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
-->