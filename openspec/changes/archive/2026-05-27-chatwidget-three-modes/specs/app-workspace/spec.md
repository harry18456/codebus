## MODIFIED Requirements

### Requirement: Chat Widget Layout and Two-State Toggle

The Workspace SHALL render a Chat Widget anchored to the bottom-right corner of the Workspace main content area (for the `bubble` and `floating` modes) or as a centered modal overlay (for the `modal` mode). The widget SHALL have exactly three visual modes, modeled in `useChatStore` by `mode: "bubble" | "floating" | "modal"` (replacing the previous `expanded: boolean` field) and `modalReturnMode: "bubble" | "floating" | null` (recording the mode the user came from when `mode === "modal"`, so Esc / backdrop click can restore it).

1. **Bubble mode**: a 44px × 44px circular bubble pinned to the viewport bottom-right corner. The bubble's right edge SHALL sit 16px from the viewport's right edge AND its bottom edge SHALL sit above the existing `BottomStrip` footer with a 16px gap (i.e., bottom offset equals `BottomStrip height + 16px`; with the current 32px-tall `BottomStrip` that is 48px from the viewport bottom). The bubble SHALL contain a `💬` emoji icon. The bubble SHALL remain visible whenever the Workspace is mounted AND SHALL NOT be obscured by any tab (Goals / Wiki / Quiz) content. When a `VerbLifecycleEvent::PromoteSuggestion` event is emitted while the widget is in bubble mode, the bubble SHALL display a small red dot badge (with `data-testid="chat-widget-promote-badge"`) until the next time the widget transitions to `floating` or `modal`. When `useGoalsStore.activeRun` is non-null (i.e., the current vault has an in-flight goal run), the bubble SHALL also display an `accent`-coloured pulse dot indicator (with `data-testid="chat-widget-active-goal-pulse"`) positioned in the bubble's top-right corner. The pulse dot SHALL be visually distinct from the promote badge in both colour (accent versus error) and position (further into the bubble's outer corner) so both indicators can be rendered simultaneously without visual overlap. The pulse dot SHALL fade in over approximately 200ms when `activeRun` transitions from null to non-null AND SHALL fade out over approximately 200ms when `activeRun` transitions back to null. When `prefers-reduced-motion: reduce` is active, the pulse dot SHALL appear AND disappear instantly with no transition. The pulse dot SHALL NOT be rendered while the widget is in `floating` or `modal` mode.

2. **Floating mode**: a fixed-size panel of exactly `360px × 460px` positioned with its bottom-right corner aligned to the same anchor point as the bubble (right: 16px, bottom: `BottomStrip height + 16px`). The panel SHALL contain four vertically stacked regions: a header bar (containing the `💬` emoji, a localized title from `chat.widget.aria.floating.title`, a `⤢` expand-to-modal button with `data-testid="chat-widget-expand-to-modal"` whose `aria-label` resolves to `chat.widget.aria.floating.expandToModal`, AND a `▿` minimize button with `data-testid="chat-widget-minimize"` whose `aria-label` resolves to `chat.widget.aria.floating.minimize`), an undo toast region, a scrollable transcript region (containing past turns and the active turn live events), and an input region (containing a textarea and a send button, or a `⏹ Stop` button while a turn is active). The floating panel SHALL NOT be resizable; no resize handle SHALL be rendered. Pressing `Esc` while in floating mode SHALL NOT close the widget (the floating mode is "sticky"; the user must click the `▿` minimize button to return to bubble mode).

3. **Modal mode**: a centered modal dialog rendered through the project's existing `Dialog` primitive (radix-ui based, located at `codebus-app/src/components/ui/dialog.tsx`). The modal content SHALL have a width of `640px` AND a maximum height of `480px`. The modal SHALL be positioned at `60px` from the viewport top (not vertically centered; the visual weight sits above center). A backdrop SHALL be rendered behind the modal with `55%` black opacity AND a `2px` blur filter (subject to graceful fallback to plain 55% black when the host WebView2 engine cannot render the blur filter performantly). The modal SHALL contain the same four regions as floating mode (header, undo toast, transcript, input) except that the header right side SHALL contain a `⤡` dock-to-floating button with `data-testid="chat-widget-dock-to-floating"` whose `aria-label` resolves to `chat.widget.aria.modal.dockToFloating` AND a `✕` close button with `data-testid="chat-widget-modal-close"` whose `aria-label` resolves to `chat.widget.aria.modal.close`. When the modal opens, the input textarea SHALL receive focus automatically. While the modal is open, the underlying `Dialog` primitive SHALL trap keyboard focus inside the modal (tab and shift-tab cycle within the modal subtree, never reaching focusable elements behind the backdrop). When the modal closes, focus SHALL return to the element that was focused at the moment the modal opened (handled by the radix `Dialog` primitive).

All three modes SHALL share a single chat session via `useChatStore` (`sessionId`, `turns`, `activeTurn`, `tokensTotal`, `promoteSuggestion`, `onboardedVaults`, `lastTranscript`, `lastSessionId`). Switching between modes SHALL NOT reset, clear, or duplicate any session state.

The widget SHALL use logical pixel values for fixed dimensions (`44px`, `360px`, `460px`, `640px`, `480px`, `60px`) AND SHALL NOT expose any user-configurable size preference. Bubble and floating modes anchor to the viewport bottom-right corner; the widget SHALL NOT be draggable to any other position. Mode preference SHALL NOT be persisted: every Workspace mount SHALL initialize with `mode = "bubble"` AND `modalReturnMode = null`.

The bubble mode bubble's `aria-label` SHALL be the localized translation of `chat.widget.aria.openChat` when `useGoalsStore.activeRun` is null. When `useGoalsStore.activeRun` is non-null, the bubble's `aria-label` SHALL instead be the localized translation of `chat.widget.aria.openChatWithActiveGoalRunning`. Both keys MUST exist in every shipped locale (currently `en` AND `zh`). The floating mode panel title SHALL render the localized translation of `chat.widget.aria.floating.title`; the modal mode dialog title SHALL render the localized translation of `chat.widget.aria.modal.title`. Both new keys MUST exist in every shipped locale.

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

#### Scenario: Active goal pulse dot appears on bubble

- **WHEN** `mode === "bubble"` AND `useGoalsStore.activeRun` transitions from null to a non-null value
- **THEN** an element with `data-testid="chat-widget-active-goal-pulse"` SHALL be rendered as a descendant of the bubble AND the dot SHALL be positioned in the bubble's top-right corner AND the dot's background colour SHALL resolve to the `--color-accent` token value AND the dot SHALL reach full opacity within approximately 200ms

#### Scenario: Active goal pulse dot disappears when run ends

- **WHEN** the bubble is rendering the active-goal pulse dot AND `useGoalsStore.activeRun` transitions to null
- **THEN** the element with `data-testid="chat-widget-active-goal-pulse"` SHALL fade to opacity 0 within approximately 200ms AND SHALL either be unmounted OR remain mounted but visually hidden such that it does NOT capture pointer events

#### Scenario: Pulse dot and promote badge render simultaneously without overlap

- **WHEN** `mode === "bubble"` AND `useGoalsStore.activeRun` is non-null AND `useChatStore.promoteSuggestion` is non-null
- **THEN** both `data-testid="chat-widget-active-goal-pulse"` AND `data-testid="chat-widget-promote-badge"` SHALL be rendered as descendants of the bubble AND their rendered bounding boxes SHALL NOT overlap

#### Scenario: Floating mode does not render pulse dot

- **WHEN** `mode === "floating"` AND `useGoalsStore.activeRun` is non-null
- **THEN** no element with `data-testid="chat-widget-active-goal-pulse"` SHALL be rendered inside the widget subtree

#### Scenario: Modal mode does not render pulse dot

- **WHEN** `mode === "modal"` AND `useGoalsStore.activeRun` is non-null
- **THEN** no element with `data-testid="chat-widget-active-goal-pulse"` SHALL be rendered inside the widget or modal portal subtree

#### Scenario: Active goal aria-label switches bubble announcement

- **WHEN** `mode === "bubble"` AND `useGoalsStore.activeRun` transitions from null to non-null
- **THEN** the element with `data-testid="chat-widget"` SHALL have its `aria-label` attribute equal to the localized translation of `chat.widget.aria.openChatWithActiveGoalRunning` AND when `activeRun` transitions back to null the `aria-label` SHALL revert to the localized translation of `chat.widget.aria.openChat`

#### Scenario: Reduced motion disables pulse dot fade transition

- **WHEN** the user agent reports `prefers-reduced-motion: reduce` AND `useGoalsStore.activeRun` transitions from null to non-null
- **THEN** the element with `data-testid="chat-widget-active-goal-pulse"` SHALL reach its visible opacity within the same frame (i.e., with no perceptible CSS transition) AND SHALL NOT animate via any keyframe loop

#### Scenario: Reduced motion disables modal open animation

- **WHEN** the user agent reports `prefers-reduced-motion: reduce` AND `mode` transitions to `"modal"`
- **THEN** the modal SHALL appear within the same frame with no perceptible fade or scale animation AND the backdrop SHALL appear within the same frame

#### Scenario: Vault switch resets mode to bubble

- **WHEN** `mode === "floating"` OR `mode === "modal"` AND the user navigates back to Lobby AND opens a different vault
- **THEN** `useChatStore.mode` SHALL be reset to `"bubble"` AND `modalReturnMode` SHALL be `null` for the new vault

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

## REMOVED Requirements

### Requirement: Chat Widget Resize Affordance

**Reason**: AUDIT R7-modes (2026-05-26) locked floating mode to a fixed `360px × 460px` size with no user-resize affordance. The resize handle, the rem-based clamped range (`[18, 40]rem × [24, 60]rem`), the viewport-cap (`50% × 80%`) auto-clamp, and the `setSize` store action are all removed.

**Migration**: Any caller of `useChatStore.setSize(width, height)` SHALL be deleted; the `width` and `height` fields on `useChatStore` SHALL be removed; the `chat-widget-resize-handle` testid SHALL no longer exist in the rendered DOM. The floating panel's dimensions are now hard-coded constants in the ChatWidget renderer (`360px × 460px`). Replacement scenarios for fixed-size rendering AND the absence of the resize handle are covered in the modified `Chat Widget Layout and Two-State Toggle` requirement (Scenario "Floating mode has no resize handle").

#### Scenario: Resize handle no longer renders

- **WHEN** the widget is rendered in any mode (`bubble`, `floating`, or `modal`)
- **THEN** no element with `data-testid="chat-widget-resize-handle"` SHALL exist in the rendered DOM AND no `useChatStore.setSize(width, height)` action SHALL be exported from the chat store module
