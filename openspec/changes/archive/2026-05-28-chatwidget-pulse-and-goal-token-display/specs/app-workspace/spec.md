## MODIFIED Requirements

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
