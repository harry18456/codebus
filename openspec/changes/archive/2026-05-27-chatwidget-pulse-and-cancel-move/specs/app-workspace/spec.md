## MODIFIED Requirements

### Requirement: Run Detail Views — Running

The system SHALL render the `Running` detail view when the user navigates to a run whose state is the currently-active goal run (i.e., `useGoalsStore.activeRun.runId` equals the clicked run id and no RunLog row has been written yet for it). The view SHALL include: a header with `← back`, the goal text, an `⏺ Running` badge, AND an `[⏹ Cancel]` button placed inside the header on the right-hand side (immediately to the right of the badge AND to the left of the reserved `pr-[160px]` Windows traffic-light padding); a metadata line with elapsed time (live-updated every second) and accumulated token count from Usage events received so far; AND an `Activity stream` block rendering received events in arrival order. The view SHALL NOT render a separate bottom `<footer>` element for the Cancel button.

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

### Requirement: Chat Widget Layout and Two-State Toggle

The Workspace SHALL render a Chat Widget overlay anchored to the bottom-right corner of the Workspace main content area. The widget's right edge SHALL sit 16px from the viewport's right edge AND its bottom edge SHALL sit above the existing `BottomStrip` footer with a 16px gap (i.e., bottom offset equals `BottomStrip height + 16px`; with the current 32px-tall `BottomStrip` that is 48px from the viewport bottom) so neither the version label nor the settings gear is ever occluded by the widget. The widget SHALL have exactly two visual states:

1. **Collapsed**: a 3rem × 3rem circular bubble containing a `💬` icon. The bubble SHALL remain visible whenever the Workspace is mounted AND SHALL NOT be obscured by any tab (Goals / Wiki / Quiz) content. When a `VerbLifecycleEvent::PromoteSuggestion` event is emitted while the widget is collapsed, the bubble SHALL display a small red dot badge (with `data-testid="chat-widget-promote-badge"`) until the next time the widget is expanded. When `useGoalsStore.activeRun` is non-null (i.e., the current vault has an in-flight goal run), the bubble SHALL also display an `accent`-coloured pulse dot indicator (with `data-testid="chat-widget-active-goal-pulse"`) positioned in the bubble's top-right corner. The pulse dot SHALL be visually distinct from the promote badge in both colour (accent versus error) and position (further into the bubble's outer corner) so both indicators can be rendered simultaneously without visual overlap. The pulse dot SHALL fade in over approximately 200ms when `activeRun` transitions from null to non-null AND SHALL fade out over approximately 200ms when `activeRun` transitions back to null. When `prefers-reduced-motion: reduce` is active, the pulse dot SHALL appear AND disappear instantly with no transition. The pulse dot SHALL NOT be rendered while the widget is in the `expanded` state.

2. **Expanded**: a `width × height` rem-sized panel positioned with its bottom-right corner aligned to the same anchor point as the collapsed bubble. The default size SHALL be `22rem × 32rem`. The panel SHALL contain three vertically stacked regions: a header bar (containing the `+ New chat` button, the token usage display, AND a `−` minimize button with `data-testid="chat-widget-minimize"` that toggles the widget back to `collapsed` when clicked), a scrollable transcript region (containing past turns and the active turn live events), and an input region (containing a textarea and a send button, or a `⏹ Stop` button while a turn is active). The top-left resize grip SHALL render a small visual affordance (e.g., a diagonal-stroke SVG icon) so the user can locate the drag handle without relying on the `nwse-resize` cursor hint alone.

The widget SHALL NOT be draggable to any other corner or position. The widget SHALL be resizable via a single grip handle on the top-left corner of the expanded panel; the user SHALL be able to drag this handle to change `width` and `height` subject to the clamped range `width ∈ [18, 40]rem` AND `height ∈ [24, 60]rem`. The clamped range SHALL further be bounded by `width <= 50% of viewport width` AND `height <= 80% of viewport height`; when the viewport shrinks below the current size, the widget SHALL auto-clamp to the new max.

The widget SHALL use `rem` units for `width`, `height`, and all internal fixed dimensions so a future global font-scale setting can affect the widget proportionally without rework.

The collapsed bubble's `aria-label` SHALL be the localized translation of `chat.widget.aria.openChat` when `useGoalsStore.activeRun` is null. When `useGoalsStore.activeRun` is non-null, the collapsed bubble's `aria-label` SHALL instead be the localized translation of `chat.widget.aria.openChatWithActiveGoalRunning`. Both keys MUST exist in every shipped locale (currently `en` AND `zh`).

#### Scenario: Collapsed widget renders as bubble in bottom-right corner

- **WHEN** the user opens a vault AND the Workspace component mounts AND `useGoalsStore.activeRun` is null
- **THEN** an element with `data-testid="chat-widget"` AND `data-state="collapsed"` SHALL render as a 3rem × 3rem rounded button positioned `position: fixed` with `right: 16px` AND `bottom: 48px` (== `BottomStrip height (32px) + 16px gap`) so it sits above the `BottomStrip` AND the bubble's `aria-label` SHALL be the localized translation of `chat.widget.aria.openChat` AND the Workspace main content area SHALL NOT have its width or layout altered by the bubble

#### Scenario: Toggle expands widget to default-size panel

- **WHEN** the user clicks the bubble AND the widget's current state is `collapsed`
- **THEN** the widget SHALL transition to state `expanded` AND the rendered element SHALL have `data-state="expanded"` AND the panel's computed width SHALL equal `22rem` AND the panel's computed height SHALL equal `32rem` AND the panel SHALL be positioned with `right: 16px` AND `bottom: 48px` (== `BottomStrip height (32px) + 16px gap`) so the panel sits above the BottomStrip

#### Scenario: Resize handle clamps to bounds

- **WHEN** the widget is expanded AND the user drags the top-left resize handle to a position that would result in `width = 50rem` AND `height = 70rem`
- **THEN** the widget's resulting `width` SHALL equal `40rem` (clamped to max) AND the `height` SHALL equal `60rem` (clamped to max)

#### Scenario: Viewport shrink auto-clamps widget

- **WHEN** the widget is expanded at `width = 30rem, height = 40rem` AND the user resizes the application window such that `viewport width × 50% = 25rem`
- **THEN** the widget's `width` SHALL auto-clamp to `25rem` AND remain visible within the new viewport

#### Scenario: Minimize button collapses the panel

- **WHEN** the widget is in state `expanded` AND the user clicks the element with `data-testid="chat-widget-minimize"` in the header
- **THEN** the widget SHALL transition to state `collapsed`

#### Scenario: Resize handle has a visible affordance

- **WHEN** the widget is in state `expanded`
- **THEN** the element with `data-testid="chat-widget-resize-handle"` SHALL render at least one SVG or icon child so the drag grip is visible without hovering, AND SHALL keep the `cursor: nwse-resize` style for the additional pointer hint

#### Scenario: Pending promote suggestion shows badge on collapsed bubble

- **WHEN** the widget state is `collapsed` AND a `VerbLifecycleEvent::PromoteSuggestion` event arrives via the `chat-stream` channel AND the user has not yet acted on the suggestion
- **THEN** the bubble SHALL render a small red dot badge with `data-testid="chat-widget-promote-badge"` AND the badge SHALL disappear the next time the widget expands or the suggestion is dismissed

#### Scenario: Active goal pulse dot appears on collapsed bubble

- **WHEN** the widget state is `collapsed` AND `useGoalsStore.activeRun` transitions from null to a non-null value
- **THEN** an element with `data-testid="chat-widget-active-goal-pulse"` SHALL be rendered as a descendant of the collapsed bubble AND the dot SHALL be positioned in the bubble's top-right corner AND the dot's background colour SHALL resolve to the `--color-accent` token value AND the dot SHALL reach full opacity within approximately 200ms

#### Scenario: Active goal pulse dot disappears when run ends

- **WHEN** the collapsed bubble is rendering the active-goal pulse dot AND `useGoalsStore.activeRun` transitions to null
- **THEN** the element with `data-testid="chat-widget-active-goal-pulse"` SHALL fade to opacity 0 within approximately 200ms AND SHALL either be unmounted OR remain mounted but visually hidden such that it does NOT capture pointer events

#### Scenario: Pulse dot and promote badge render simultaneously without overlap

- **WHEN** the widget state is `collapsed` AND `useGoalsStore.activeRun` is non-null AND `useChatStore.promoteSuggestion` is non-null
- **THEN** both `data-testid="chat-widget-active-goal-pulse"` AND `data-testid="chat-widget-promote-badge"` SHALL be rendered as descendants of the collapsed bubble AND their rendered bounding boxes SHALL NOT overlap

#### Scenario: Expanded widget does not render pulse dot

- **WHEN** the widget state is `expanded` AND `useGoalsStore.activeRun` is non-null
- **THEN** no element with `data-testid="chat-widget-active-goal-pulse"` SHALL be rendered inside the widget subtree

#### Scenario: Active goal aria-label switches collapsed bubble announcement

- **WHEN** the widget state is `collapsed` AND `useGoalsStore.activeRun` transitions from null to non-null
- **THEN** the element with `data-testid="chat-widget"` SHALL have its `aria-label` attribute equal to the localized translation of `chat.widget.aria.openChatWithActiveGoalRunning` AND when `activeRun` transitions back to null the `aria-label` SHALL revert to the localized translation of `chat.widget.aria.openChat`

#### Scenario: Reduced motion disables pulse dot fade transition

- **WHEN** the user agent reports `prefers-reduced-motion: reduce` AND `useGoalsStore.activeRun` transitions from null to non-null
- **THEN** the element with `data-testid="chat-widget-active-goal-pulse"` SHALL reach its visible opacity within the same frame (i.e., with no perceptible CSS transition) AND SHALL NOT animate via any keyframe loop
