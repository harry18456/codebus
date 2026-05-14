## ADDED Requirements

### Requirement: Chat Widget Layout and Two-State Toggle

The Workspace SHALL render a Chat Widget overlay anchored to the bottom-right corner of the Workspace main content area. The widget's right edge SHALL sit 16px from the viewport's right edge AND its bottom edge SHALL sit above the existing `BottomStrip` footer with a 16px gap (i.e., bottom offset equals `BottomStrip height + 16px`; with the current 32px-tall `BottomStrip` that is 48px from the viewport bottom) so neither the version label nor the settings gear is ever occluded by the widget. The widget SHALL have exactly two visual states:

1. **Collapsed**: a 3rem × 3rem circular bubble containing a `💬` icon. The bubble SHALL remain visible whenever the Workspace is mounted and SHALL NOT be obscured by any tab (Goals / Wiki / Quiz) content. When a `VerbLifecycleEvent::PromoteSuggestion` event is emitted while the widget is collapsed, the bubble SHALL display a small red dot badge until the next time the widget is expanded.

2. **Expanded**: a `width × height` rem-sized panel positioned with its bottom-right corner aligned to the same anchor point as the collapsed bubble. The default size SHALL be `22rem × 32rem`. The panel SHALL contain three vertically stacked regions: a header bar (containing the `+ New chat` button, the token usage display, AND a `−` minimize button with `data-testid="chat-widget-minimize"` that toggles the widget back to `collapsed` when clicked), a scrollable transcript region (containing past turns and the active turn live events), and an input region (containing a textarea and a send button, or a `⏹ Stop` button while a turn is active). The top-left resize grip SHALL render a small visual affordance (e.g., a diagonal-stroke SVG icon) so the user can locate the drag handle without relying on the `nwse-resize` cursor hint alone.

The widget SHALL NOT be draggable to any other corner or position. The widget SHALL be resizable via a single grip handle on the top-left corner of the expanded panel; the user SHALL be able to drag this handle to change `width` and `height` subject to the clamped range `width ∈ [18, 40]rem` AND `height ∈ [24, 60]rem`. The clamped range SHALL further be bounded by `width <= 50% of viewport width` AND `height <= 80% of viewport height`; when the viewport shrinks below the current size, the widget SHALL auto-clamp to the new max.

The widget SHALL use `rem` units for `width`, `height`, and all internal fixed dimensions so a future global font-scale setting can affect the widget proportionally without rework.

#### Scenario: Collapsed widget renders as bubble in bottom-right corner

- **WHEN** the user opens a vault AND the Workspace component mounts
- **THEN** an element with `data-testid="chat-widget"` AND `data-state="collapsed"` SHALL render as a 3rem × 3rem rounded button positioned `position: fixed` with `right: 16px` AND `bottom: 48px` (== `BottomStrip height (32px) + 16px gap`) so it sits above the `BottomStrip` AND the bubble's `aria-label` SHALL contain the text `"Open chat"` (or the locale-specific translation) AND the Workspace main content area SHALL NOT have its width or layout altered by the bubble

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
- **THEN** the bubble SHALL render a small red dot badge AND the badge SHALL disappear the next time the widget expands or the suggestion is dismissed

---

### Requirement: Chat Widget Toggle Shortcut

The Workspace SHALL register a keyboard shortcut `Cmd+K` (on macOS) AND `Ctrl+K` (on Windows / Linux) that toggles the Chat Widget between `collapsed` and `expanded` states. The shortcut SHALL be active only while the Workspace component is mounted; the shortcut SHALL NOT fire in the Lobby route. The shortcut handler SHALL call `preventDefault()` on the keydown event to prevent the host browser's default Ctrl+K binding from interfering.

#### Scenario: Cmd+K toggles widget while in Workspace

- **WHEN** the Workspace is mounted AND the widget's current state is `collapsed` AND the user presses `Cmd+K` (or `Ctrl+K`)
- **THEN** the widget SHALL transition to `expanded` AND a subsequent `Cmd+K` (or `Ctrl+K`) SHALL transition it back to `collapsed`

#### Scenario: Shortcut inactive in Lobby

- **WHEN** the Lobby is rendered (no vault selected) AND the user presses `Cmd+K`
- **THEN** no chat widget SHALL appear AND no error SHALL occur AND the keydown SHALL propagate to the browser default (effectively no-op since there is no chat widget to toggle)

---

### Requirement: Chat Session Lifecycle and Reset Triggers

The Chat Widget SHALL maintain a single in-memory session per vault, identified by a `sessionId: String | null` field in `useChatStore`. The `sessionId` starts as `null` AND becomes a non-null string after the first successful `spawn_chat_turn` resolves with the claude CLI session id. Subsequent `spawn_chat_turn` calls within the same session SHALL pass this `sessionId` as the `session_id` parameter so the backend issues `--resume <id>` to the claude CLI.

The session state SHALL be reset to its initial (empty transcript, `sessionId = null`) state on the following triggers ONLY:

1. **Vault switch** — when the Workspace component unmounts (because the user returned to Lobby or opened a different vault). This is enforced by calling `useChatStore.resetForVault()` in the Workspace `useEffect` cleanup.
2. **`+ New chat` button** — when the user clicks the button in the widget header. Before resetting, the store SHALL copy the current `sessionId` and `turns` into `lastSessionId` and `lastTranscript` fields. A toast SHALL render with text `"Started new chat. [Undo]"` (or the locale-specific translation) for 5 seconds; clicking `[Undo]` within the 5-second window SHALL restore `sessionId` and `turns` from the saved fields. After 5 seconds the saved fields SHALL be garbage-collected and the toast SHALL fade out.

The session state SHALL NOT be persisted to disk; an application reload SHALL discard the session entirely. The session state SHALL NOT be reset by:

- Switching between Workspace tabs (Goals / Wiki / Quiz)
- Toggling the widget between collapsed and expanded
- Resizing the widget
- An active turn finishing (succeeded, cancelled, or failed)

#### Scenario: Vault switch resets the chat session and collapses the widget

- **WHEN** the user has an active chat session for vault `V1` with `sessionId = "abc-123"` AND `turns.length = 3` AND `expanded = true` AND the user clicks `← Back to Lobby` AND then opens vault `V2`
- **THEN** the `useChatStore` state SHALL have `sessionId = null` AND `turns.length = 0` AND `expanded = false` for `V2` so the widget opens as a bubble in the fresh vault. User-resize preferences (`width`, `height`) SHALL survive the round-trip.

#### Scenario: + New chat triggers undo toast

- **WHEN** the user has an active chat session with `sessionId = "abc-123"` AND `turns.length = 3` AND the user clicks `+ New chat`
- **THEN** the `useChatStore.sessionId` SHALL become `null` AND `turns.length` SHALL become `0` AND a toast with `data-testid="chat-undo-toast"` SHALL render with text containing `"Started new chat"` AND an `[Undo]` button

#### Scenario: Undo within 5 seconds restores session

- **WHEN** the `chat-undo-toast` is visible (less than 5 seconds since `+ New chat` clicked) AND the user clicks `[Undo]`
- **THEN** the `useChatStore.sessionId` SHALL be restored to its previous value (`"abc-123"`) AND `turns` SHALL be restored to its previous content AND the toast SHALL disappear

#### Scenario: Undo buffer gc'd after 5 seconds

- **WHEN** the `chat-undo-toast` has been visible for 5 seconds AND no `[Undo]` click occurred
- **THEN** the toast SHALL fade out AND the `lastSessionId` / `lastTranscript` fields in `useChatStore` SHALL be set back to `null`

#### Scenario: Tab switch preserves chat session

- **WHEN** the user has an active chat session with `turns.length = 3` AND the user switches the Workspace tab from Goals to Wiki AND back to Goals
- **THEN** the `useChatStore.turns.length` SHALL still equal `3` AND the widget's expanded/collapsed state SHALL be preserved

---

### Requirement: Tauri IPC Commands for Chat Turn Lifecycle

The system SHALL register two new Tauri commands for chat turn lifecycle, extending the goal lifecycle IPC surface:

- `spawn_chat_turn(vault_path: String, text: String, session_id: Option<String>) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::chat::run_chat_turn` with `ChatTurnOptions { text, session_id }`. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the new `RunId` (where `RunId` = `chat-<started_at_slug>`), and emit each `VerbEvent` produced by the closure to a Tauri event channel named `"chat-stream"` with payload `{ run_id: String, event: VerbEvent }`. The chat-stream channel SHALL be separate from the existing `goal-stream` channel. On thread completion (success, failure, cancel, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_chat_turn(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (turn already terminated), return `Ok(())` (idempotent).

`spawn_chat_turn` SHALL return `AppError::Invalid { field: "active_runs", message: "another chat turn is already active in this session" }` when invoked while `active_runs` already contains a `chat-*` keyed entry for the same vault. Goal-mode entries SHALL NOT block chat spawn AND vice versa (see `One Active Goal Run At A Time` modification).

#### Scenario: spawn_chat_turn returns chat run id

- **WHEN** the frontend calls `invoke("spawn_chat_turn", { vault_path: "/some/vault", text: "X", session_id: null })` AND the spawned `run_chat_turn` invocation's first stream event timestamps the turn at `2026-05-14T10:20:30Z`
- **THEN** the IPC call resolves with `"chat-2026-05-14T10-20-30Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string

#### Scenario: spawn_chat_turn rejects when chat turn already active

- **WHEN** a chat turn is currently active for vault `V` (an entry exists in `active_runs` with key starting `chat-`) AND the frontend invokes `spawn_chat_turn` with the same vault
- **THEN** the IPC call rejects with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring `"chat turn is already active"`

#### Scenario: chat-stream events forwarded with run_id payload

- **WHEN** `spawn_chat_turn` is invoked AND the backend emits a `VerbEvent::Stream { ... }` event
- **THEN** the Tauri event channel `chat-stream` SHALL receive a payload `{ run_id: "chat-<slug>", event: <VerbEvent JSON> }`

#### Scenario: cancel_chat_turn idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_chat_turn", { run_id: "chat-nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

#### Scenario: Cancelled chat turn preserves session id for next turn

- **WHEN** a chat turn with `sessionId = "abc-123"` is active AND the user clicks `⏹ Stop` AND the cancel flag flips AND `run_chat_turn` returns `Err(VerbError::Cancelled)`
- **THEN** the `useChatStore.sessionId` SHALL still equal `"abc-123"` AND a subsequent `spawn_chat_turn` call SHALL pass `session_id: "abc-123"` so the backend issues `--resume abc-123`

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

---

### Requirement: Chat Assistant Message Markdown Rendering and Wiki Citation Links

The Chat Widget SHALL render each assistant message's text content through a Markdown renderer (e.g., `react-markdown`) rather than as plain text. The renderer SHALL produce clickable elements for markdown links of the form `[label](href)` where `href` matches the regex `^wiki\/.+\.md$`; clicking such a link SHALL invoke the Wiki tab page-load pathway (the same one used by `WikiTab` to render a chosen page) with the link's `href` path as the page slug, AND SHALL set the Workspace active tab to `wiki`, AND SHALL transition the Chat Widget to `collapsed`. Links whose `href` starts with `http://` or `https://` SHALL open in the user's default browser via the existing Tauri opener plugin and SHALL NOT transition the widget or change the active tab. Links with other `href` patterns (e.g., source code paths like `src/auth/jwt.rs`) SHALL be rendered as inert text in v1 — no click handler attached.

Plain-text mentions of wiki paths within an assistant message (e.g., `"see wiki/modules/auth.md"` without markdown link syntax) SHALL NOT be auto-detected or made clickable in v1; only markdown-link syntax SHALL produce clickable elements.

#### Scenario: Wiki markdown link click switches Wiki tab

- **WHEN** an assistant message contains the markdown text `[auth.md](wiki/modules/auth.md)` AND the user clicks the rendered link
- **THEN** the Workspace active tab SHALL become `wiki` AND the Wiki tab SHALL invoke its page-load pathway with `wiki/modules/auth.md` (or the equivalent slug `modules/auth`) AND the Chat Widget SHALL transition to `collapsed`

#### Scenario: External https link opens in browser

- **WHEN** an assistant message contains `[docs](https://example.com/foo)` AND the user clicks the link
- **THEN** the Tauri opener plugin SHALL be invoked with the URL `https://example.com/foo` AND the Workspace active tab SHALL NOT change AND the Chat Widget SHALL remain in its current state

#### Scenario: Source code path renders as inert text

- **WHEN** an assistant message contains the markdown text `[jwt.rs](src/auth/jwt.rs)` AND the user clicks the rendered text
- **THEN** no navigation or IPC call SHALL occur AND the rendered element SHALL NOT have an `<a>` tag with a non-empty href OR equivalent click handler

#### Scenario: Plain text wiki mention is not clickable

- **WHEN** an assistant message contains the plain text `"see wiki/modules/auth.md for details"` (no markdown link syntax)
- **THEN** the rendered text `"wiki/modules/auth.md"` SHALL NOT have a click handler attached AND SHALL render as inert prose

---

### Requirement: Chat Widget Mount at Workspace Level

The Chat Widget element SHALL be rendered by the `Workspace` component (not by any individual tab component such as `GoalsTab`, `WikiTab`, or `QuizTab`) so that the widget remains mounted with consistent state across tab switches within the same vault. The widget SHALL be positioned via fixed/absolute CSS such that it overlays the entire Workspace main area regardless of which tab is currently displayed underneath.

When the user changes tabs (Goals → Wiki → Quiz or any other transition), the Chat Widget's state (`expanded`, `width`, `height`, transcript content, session id, active turn) SHALL be preserved without re-mounting the component.

#### Scenario: Chat persists across tab switches

- **WHEN** the user expands the Chat Widget on the Goals tab AND types one turn AND switches to the Wiki tab AND back to Goals
- **THEN** the Chat Widget SHALL still be expanded AND the typed turn SHALL still appear in the transcript AND `useChatStore.sessionId` SHALL still equal the value from before the tab switch

---

## MODIFIED Requirements

### Requirement: Tauri IPC Commands for Goal Lifecycle and Wiki Read

The system SHALL register Tauri commands beyond the foundation's nine commands, covering goal-mode lifecycle, chat-turn lifecycle, and wiki read paths. The full added set is:

- `spawn_goal(vault_path: String, goal_text: String) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::goal::run_goal` with the given vault and goal text. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the new `RunId` (where `RunId` equals the run's `started_at` slug), and emit each `VerbEvent` produced by the closure to a Tauri event channel named `"goal-stream"` with payload `{ run_id: String, event: VerbEvent }`. On thread completion (success, failure, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_goal(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (run already terminated), return `Ok(())` (idempotent).

- `list_runs(vault_path: String, mode_filter: ModeFilter) -> Result<Vec<RunLogSummary>, AppError>` — read all `runs-*.jsonl` files under `<vault>/.codebus/log/`, parse each row to `RunLogSummary`, apply `mode_filter` (`Goal` keeps only `mode=="goal"`; `All` keeps everything), then scan `events-*.jsonl` files for interrupted detection per the next requirement, merge virtual entries, and return the combined list sorted by `started_at` descending.

- `get_run_detail(vault_path: String, run_id: String) -> Result<RunDetail, AppError>` — find the matching `RunLogSummary` (real or virtual interrupted), open the corresponding `events-*.jsonl`, replay all events into `Vec<RecordedEvent>`, and return `RunDetail { summary, events }`.

- `list_wiki_pages(vault_path: String) -> Result<Vec<WikiPageMeta>, AppError>` — glob `<vault>/.codebus/wiki/**/*.md`, parse each file's frontmatter to extract `title`, derive slug from the filename (without `.md`), and return one `WikiPageMeta { slug, path, title }` per file. Files without parseable frontmatter SHALL still be returned with `title` equal to the slug.

- `read_wiki_page(vault_path: String, page_slug: String) -> Result<String, AppError>` — look up the page by slug among the wiki files, read its raw bytes, strip the leading frontmatter block (delimited by `---\n...\n---\n` at the start), and return the remaining markdown body as a `String`. If the slug does not match any wiki file, return `AppError::Invalid { field: "page_slug", message: "no such page" }`.

The chat-turn lifecycle commands (`spawn_chat_turn`, `cancel_chat_turn`) are defined separately under `Tauri IPC Commands for Chat Turn Lifecycle` and SHALL coexist with the above in `codebus-app/src-tauri/src/ipc/mod.rs` registration.

`ModeFilter` SHALL be a serde-tagged enum with variants `Goal` and `All` (snake_case).

`AppError` SHALL be the same discriminated union used by the foundation's commands — no new variants added by this change.

#### Scenario: spawn_goal returns run id derived from started_at

- **WHEN** the frontend calls `invoke("spawn_goal", { vault_path: "/some/vault", goal_text: "X" })` AND the spawned `run_goal` invocation's first stream event timestamps the run at `2026-05-13T14:56:21Z`
- **THEN** the IPC call resolves with `"2026-05-13T14-56-21Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string

#### Scenario: cancel_goal idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_goal", { run_id: "nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

#### Scenario: list_runs filters by mode

- **WHEN** the frontend calls `invoke("list_runs", { vault_path: ..., mode_filter: { kind: "goal" } })` AND the vault's `runs-*.jsonl` contain three goal rows, two chat rows, and one fix row
- **THEN** the returned `Vec<RunLogSummary>` length is 3 AND every entry has `mode == "goal"`

#### Scenario: read_wiki_page strips frontmatter

- **WHEN** the frontend calls `invoke("read_wiki_page", { vault_path: ..., page_slug: "uv-lib" })` AND the file at `<vault>/.codebus/wiki/modules/uv-lib.md` contains a frontmatter block followed by markdown body
- **THEN** the IPC returns the markdown body string without the leading `---\n...\n---\n` block

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
