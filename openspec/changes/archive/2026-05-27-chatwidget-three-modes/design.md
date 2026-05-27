## Context

ChatWidget 是 codebus Workspace 右下角 chat 入口、現有實作（`codebus-app/src/components/workspace/ChatWidget.tsx`）只有 `collapsed bubble` ↔ `expanded floating panel` 兩態，state 由 `useChatStore.expanded: boolean` toggle。Phase 6 v1.1 mock（`codebus-app/design-handoff/v1.1-mocks.html` §05）+ AUDIT R7-modes lock（2026-05-26）把 ChatWidget 升級為三 mode：

1. **Bubble mode**（沿用既有 collapsed）—— idle 待命、active-goal pulse dot 已在 5.1 接好
2. **Floating mode**（沿用既有 expanded、改固定大小）—— inline 聊天、不擋畫面
3. **Centered modal mode**（新增）—— spotlight 體驗、⌘K 預設觸發、取代 cut 掉的 05 Cmd+K Overlay

切換矩陣 8 條 row（AUDIT R7-modes 表）、Esc 行為分層、modal 開啟前 mode 還原、focus trap、a11y 等都要正確處理。

Workspace 結構：`codebus-app/src/components/workspace/Workspace.tsx` 在 vault open 時 mount 一個 `<ChatWidget />`、跨 tab survive；`useChatShortcut` hook 在 Workspace 層 register ⌘K listener；session state 在 `useChatStore`。

## Pre-apply 校準

Per `project_quiz_fullscreen_wizard_view_term_disambiguation` + `project_phase_3a_blind_spots_cleanup_lessons` 教訓，apply 第一步必須做以下校準：

### 同名詞 disambiguation —「ChatWidget」四層意義

| 「ChatWidget」 | 含意 | 本 change spec / tasks 用詞 |
|---|---|---|
| Umbrella concept | 整個 chat 功能 / 該元件家族 | 一律寫「the chat widget」/「ChatWidget shell」、不單獨用「ChatWidget」當 mode 名 |
| Mode 1 · 收合圓鈕 | 右下 44×44 圓鈕 + 💬 + active-goal pulse dot | **bubble mode** |
| Mode 2 · 浮動面板 | 360×460 右下浮動 panel | **floating mode** |
| Mode 3 · 中央彈窗 | 640 wide centered modal + backdrop | **modal mode**（或 centered modal mode 全寫） |

→ spec scenarios 任何「ChatWidget」字眼附帶 mode prefix（bubble / floating / modal）；mock 上的「ChatWidget · 3 modes」標題只是 section header、不直接進 spec 文字。

### Ground truth grep 結果（2026-05-27）

1. **主入口**：`codebus-app/src/components/workspace/ChatWidget.tsx`（350 行、含 `ExpandedPanel` 子 component）
2. **state 來源**：`codebus-app/src/store/chat.ts`
   - 現有 fields：`expanded: boolean`, `width: number`, `height: number`, `onboardedVaults: Set<string>`
   - 現有 actions：`toggleExpanded()`, `setSize(w, h)`
   - session fields 已在 store：`sessionId / sessionProviderKey / turns / activeTurn / tokensTotal / promoteSuggestion / lastTranscript / lastSessionId` → **三 mode 共享 zero-cost**
3. **5.1 active-goal pulse**：`hasActiveGoal = useGoalsStore((s) => s.activeRun != null)`、bubble mode renderer 內 `<span data-testid="chat-widget-active-goal-pulse">`、200ms opacity transition、`motion-reduce:transition-none`。本 change **直接 inherit**、不改邏輯。
4. **mount 點**：`codebus-app/src/components/workspace/Workspace.tsx` `<ChatWidget vaultPath={vault.path} ... />`、Workspace 層級、跨 tab survive（comment 已說明）。
5. **shortcut hook**：`codebus-app/src/hooks/useChatShortcut.ts`、現綁 ⌘K / Ctrl+K → `toggleExpanded()`、僅 Workspace 用、Lobby 不註冊。
6. **既有 chat i18n keys**：`codebus-app/src/i18n/messages.ts` 已有 `chat.widget.aria.openChat / openChatWithActiveGoalRunning / closeChat / resizeChat / minimizeChat`、`chat.widget.title.dragToResize / minimizeShortcut`、`chat.placeholder.*`、`chat.button.*`、`chat.toast.*`、`chat.error.*`、`chat.token.tooltip.*`、`chat.undoToast.*`、`chat.tokens.indicator`、`chat.onboarding.*`。本 change 新增「不改現有 key、value-only」。
7. **05 Cmd+K Overlay 殘留檢查**：grep `CmdKOverlay` / `cmdk-overlay` / `Cmd+K` 過 codebus-app/src/、無 runtime 殘留；只在 `codebus-app/design-handoff/design_files/components/cmdk-overlay.jsx` + 對應 index.html 兩處 mock 檔（不在 runtime bundle）。本 change 不清 mock。
8. **Dialog primitive**：`codebus-app/src/components/ui/dialog.tsx`、wrap radix-ui `@radix-ui/react-dialog`、含 `Dialog / DialogTrigger / DialogPortal / DialogClose / DialogOverlay / DialogContent / DialogHeader / DialogTitle / DialogFooter`、已用於 `codebus-app/src/components/lobby/NewVaultFlow.tsx`、提供 focus trap + aria-modal + restore focus。本 change **reuse**。
9. **既有 floating panel resize handle**：`chat-widget-resize-handle` testid + `setSize` + `[18, 40]rem × [24, 60]rem` clamp + viewport resize `useEffect` → AUDIT R7-modes lock「固定大小 360×460、拿掉 resize handle」要全砍。

### AUDIT / mock / 實機差異列表

| 來源 | 描述 | 校準決議 |
|---|---|---|
| Current spec `app-workspace` | 用 `collapsed / expanded` 兩態 | 本 change 升 `bubble / floating / modal` 三 mode、移除 resize handle scenarios |
| AUDIT R7-modes | floating「360×460 固定大小」 | 拿掉現有 18-40rem × 24-60rem resize handle + `setSize` + viewport-clamp `useEffect`；`width / height` field 從 store 移除 |
| AUDIT R7-modes 切換矩陣 row「modal · Esc / click backdrop · 回到觸發前 mode（bubble 或 floating）」 | 需要記錄 modal 開啟前的 mode | 加 `modalReturnMode: "bubble" \| "floating" \| null`、`openModal()` 進入前 snapshot、Esc / backdrop 還原 |
| Mock §05 5.2 | floating Esc 不關閉（黏著） | floating mode keydown Esc → no-op（不 minimize、不關）；user **必須**按 `▿` minimize button |
| Mock §05 5.3 + AUDIT 切換矩陣 | modal Esc 還原 / `⤡` dock → floating / `✕` close → bubble | modal 三條退出路徑各自處理 |
| ⌘K 衝突 | sidebar S7（Phase 4）只是 mock 上的 kbd chip 顯示「⌘K」、無實際 binding | ⌘K 唯一 owner 為 `useChatShortcut`、不衝突；archive 階段確認 sidebar chip 點擊行為（若實作則調用同 action） |
| Task 1.1 校準新增 | `codebus-app/src/store/chat.ts:376` `acceptPromoteSuggestion` 設 `expanded: false` | 改設 `mode: "bubble"` + `modalReturnMode: null`（語意：promote 完成回 bubble） |
| Task 1.1 校準新增 | `codebus-app/src/components/workspace/ChatTranscript.tsx:627-632` wiki-link click `if (expanded) toggleExpanded()` | 依當前 `mode` 分派：`floating` → `minimizeToBubble()`；`modal` → `closeModalToBubble()`；`bubble` → no-op |
| Task 1.1 校準新增 | seed `expanded: INITIAL_STATE.expanded` / `expanded: true` 散落多 test file（`ChatTokenDisplay.test.tsx` / `ChatUndoToast.test.tsx` / `ChatTranscript.test.tsx` lines 130/211/248/288/415 / `Workspace.test.tsx` lines 62/386 / `ChatWidget.test.tsx` lines 174/183/210/212/222 / `store/chat.test.ts` lines 42/71/89/175/186）| 4.x / 6.x / 2.x task 同 batch 替換為 `mode` seed |
| Task 1.1 校準新增 | `DEFAULT_WIDTH_REM` / `DEFAULT_HEIGHT_REM` consts 在 `codebus-app/src/store/chat.ts:131-132` | 跟 `setSize / width / height` 同 batch 移除 |
| Task 1.1 校準新增 | `codebus-app/src/components/workspace/Workspace.tsx:119` 註解寫「(expanded, width, height, onboardedVaults) 跨 vault 切換 survive」 | 更新註解為「(mode, modalReturnMode 不 survive、由 resetForVault 重置；onboardedVaults survive)」 |
| Task 1.1 校準新增 | `codebus-app/src/components/workspace/Workspace.test.tsx:345` 既有 test「keeps chat widget expanded state across tab switches」 | 改寫為「keeps chat widget mode across tab switches」、用 `mode` 斷言（屬 Task 6.1 範圍） |
| Task 1.1 校準新增 | 05 Cmd+K Overlay 殘留檢查 grep 結果 | `codebus-app/src/` 內 `CmdKOverlay` / `cmdk-overlay` 0 hit（只在 `codebus-app/design-handoff/design_files/` mock）、本 change 不清 mock |

## Goals / Non-Goals

**Goals:**

- 把 ChatWidget state machine 從 binary `expanded: boolean` 升級為三態 enum `mode: "bubble" | "floating" | "modal"`、所有切換走顯式 action
- 實作 centered modal mode（reuse radix Dialog、640 wide、top 60px、backdrop blur 2px + 55% black）
- 三 mode 共用同一 chat session、切換時 transcript / token usage / activeTurn / scroll position 全保留
- modal 開啟前的 mode 由 `modalReturnMode` snapshot、Esc / backdrop click 還原
- 5.1 active-goal pulse dot 在新 bubble mode renderer 完整 inherit、行為不變
- modal mode focus trap + restore focus on close（由 radix Dialog 提供）
- 新增 5-8 條 mode-aware aria-label i18n key（en + zh）、既有 key value-only 不改名
- 拿掉 floating mode 既有 resize handle（AUDIT R7-modes lock「固定大小」）
- ⌘K 行為從 toggle expand 改為 open modal、不管當下 mode

**Non-Goals:**

- 不改 5.1 active-goal pulse dot 邏輯（fade timing / `useGoalsStore.activeRun` 偵測規則 / `motion-reduce` 行為）—— spec scenarios 既有 sentence 沿用
- 不動 6.1 RunDetailInterrupted / Phase 4 Sidebar / Phase 4 Content header
- 不重造 ChatTranscript / ChatInput / ChatTokenDisplay / ChatNewChatButton / ChatUndoToast / 既有 promoteSuggestion 流
- 不把 emoji 💬 換 MessageSquare（design v1 R7-1 已 pushback ack）
- 不復活 05 Cmd+K Overlay
- 不持久化 mode 偏好（每次 vault open 從 bubble 起、per AUDIT 5.5）
- 不譯 ⌘K / Cmd+K / Ctrl+K / Esc 字面（Cat D identifier）
- 不抽 chat session 到新 store（已在 `useChatStore`）
- 不做大幅 mode 切換動畫（subtle fade only、`prefers-reduced-motion` 提供 instant）
- 不清 `codebus-app/design-handoff/design_files/components/cmdk-overlay.jsx` 等 mock 殘留（不在 runtime bundle）

## Decisions

### State machine: expanded boolean → mode enum + modalReturnMode snapshot

定義：

- `mode: ChatWidgetMode` 取代既有 `expanded: boolean`，值為 `"bubble" | "floating" | "modal"`
- `modalReturnMode: "bubble" | "floating" | null`，記錄 modal 開啟前的 mode

切換 action 對應 AUDIT R7-modes 切換矩陣 8 條 row：

| Action | from | to | 副作用 |
|---|---|---|---|
| `openFloating()` | bubble | floating | `modalReturnMode = null` |
| `minimizeToBubble()` | floating | bubble | `modalReturnMode = null` |
| `openModal()`（⌘K universal） | bubble OR floating | modal | snapshot 當前 mode 到 `modalReturnMode`；modal 已開時 no-op |
| `dockToFloating()` | modal | floating | `modalReturnMode = null` |
| `closeModalToReturnMode()` | modal | `modalReturnMode` 對應 mode（bubble OR floating） | `modalReturnMode = null` |
| `closeModalToBubble()` | modal | bubble（`✕` close） | `modalReturnMode = null` |

理由：action 集合跟切換矩陣 row 一對一、reducer 顯式、unit test 覆蓋容易。

**Rejected alternatives**:
- Two booleans (`expanded` + `modal`): 4 種組合中 `{expanded: true, modal: true}` 無語意、易出 bug。
- 只用 `mode` 不存 `modalReturnMode`：違反 AUDIT R7-modes「modal Esc → 回到觸發前 mode」row。

### Modal renderer reuse radix Dialog primitive

modal mode renderer 用 `<Dialog open={mode === "modal"} onOpenChange={(o) => !o && closeModalToReturnMode()}>` + `<DialogContent>` 包 chat body / footer；`DialogContent` 透過 className override 達 640 wide × max 480 tall + top 60px + backdrop blur 2px + 55% black（既有 `DialogOverlay` 用 `bg-black/50`、需 extend 到 55% + blur）。focus trap / aria-modal / restore focus on close 全由 radix 提供、不自寫。

**Rejected alternative**: 自寫 `<dialog>` element + 手動 focus trap — codebase 已 standardize radix Dialog（lobby NewVaultFlow / SetKeyDialog 都用）、重造徒增維護負擔且 a11y 容易漏。

### Esc 行為分層

- **modal mode**：Esc → `closeModalToReturnMode()`（還原 bubble 或 floating）—— 由 radix Dialog `onOpenChange` 自動處理
- **floating mode**：Esc → **no-op**（per mock §05 5.2「Esc / 點外面不關閉」黏著）—— 不註冊 floating-level Esc listener
- **bubble mode**：Esc → no-op

⌘K 任何 mode 都呼叫 `openModal()`、但 modal 已開時 `openModal()` 內部 no-op（避免 re-snapshot return mode）。

### Floating mode 移除 resize handle

AUDIT R7-modes lock「**固定大小 360×460（拿掉 resize handle）**」。動作：

- `chat-widget-resize-handle` DOM + pointer event handlers 全刪
- `width / height / setSize` field 從 `useChatStore` 移除；ChatWidget renderer 直接用 `360px × 460px` 定值
- viewport resize `useEffect` 全刪
- `chat.widget.aria.resizeChat` / `chat.widget.title.dragToResize` i18n key 從 callsite 移除（key value 本 change 不清、archive 階段視需要）
- spec delta 將既有 resize / clamp / viewport-shrink 三條 scenario 列為 REMOVED

### Modal mode mount strategy

`<ChatWidget />` 仍 mount 在 Workspace.tsx 單一掛載點；renderer 依 mode 走三 branch：

- bubble mode → render bubble button
- floating mode → render floating panel
- modal mode → render `<Dialog>`（radix portal 處理）

modal 開啟時 bubble 不渲染（避免雙重視覺元素干擾）；floating 開啟 modal 時 floating 也不渲染（modal 取代之、不疊圖）。

### Cmd+K 統一走 openModal、不再 toggle

`codebus-app/src/hooks/useChatShortcut.ts` 從 `toggleExpanded()` 改 `openModal()`；alread-in-modal 時 `openModal()` 內部 no-op。

⌘K 是 modal mode 的 universal fast-path、不論當下在 bubble 還是 floating。

### Chat session 共享

session state 全在 `useChatStore`（`sessionId / turns / activeTurn / tokensTotal / promoteSuggestion / onboardedVaults / lastTranscript / lastSessionId`）、本 change **不動**；三 mode renderer 都 subscribe 同樣 selector、共享 zero-cost。spec scenarios 必須有「mode 切換時 transcript / token usage / activeTurn 保留」row（明示）。

### i18n 新 key（mode-aware aria-label）

新增 8 key（en + zh，4 對）：

- `chat.widget.aria.bubble.openFloating` — bubble click aria-label（mode-aware；舊 `openChat` / `openChatWithActiveGoalRunning` 保留作為 bubble idle / active-goal 預設、本 change 不強制改）
- `chat.widget.aria.floating.title` — floating panel 標題（mock 「Ask about this vault」）
- `chat.widget.aria.floating.minimize` — `▿` minimize button aria-label
- `chat.widget.aria.floating.expandToModal` — `⤢` expand button aria-label
- `chat.widget.aria.modal.title` — modal title
- `chat.widget.aria.modal.dockToFloating` — `⤡` dock button aria-label
- `chat.widget.aria.modal.close` — `✕` close button aria-label
- `chat.widget.aria.modal.input` — modal input field aria-label

**Decision**：既有 `chat.widget.aria.openChat / openChatWithActiveGoalRunning / closeChat / minimizeChat` 不改名、value 不動（per Phase 4A G-copy-2「既有 key 不改名」教訓）；bubble mode renderer 仍可用既有 `openChat` 系列。實際 key 數量可能 5-8 條、apply 階段視 callsite 收斂。

## Implementation Contract

### Behavior（user-observable）

- Workspace 載入時、ChatWidget 預設 `mode = "bubble"`、`modalReturnMode = null`
- bubble click → floating（panel 360×460、右下、Esc 不關閉）
- bubble + ⌘K → modal（centered、backdrop blur、modal input 預設 focus）
- floating + `▿` minimize → bubble
- floating + `⤢` expand → modal、`modalReturnMode = "floating"`
- floating + ⌘K → modal、`modalReturnMode = "floating"`
- modal + Esc / backdrop click → `modalReturnMode` 對應 mode（bubble 或 floating）
- modal + `⤡` dock → floating（不論進入前是 bubble 還是 floating、都進 floating；對應 AUDIT 切換矩陣 row「modal · ⤡ dock · floating」）
- modal + `✕` close → bubble
- 三 mode 切換時 transcript / token usage / activeTurn / promoteSuggestion 不變
- bubble mode 在 `useGoalsStore.activeRun != null` 時顯示 7px amber pulse dot、fade 200ms、`motion-reduce` instant（**沿用 5.1**）
- floating / modal mode **不渲染** active-goal pulse dot（user 已在 chat、不需提示）
- modal mode tab / Shift+Tab focus 循環只在 modal 內
- modal mode 關閉後 focus 還原到觸發 modal 的元素（radix Dialog 行為）
- `prefers-reduced-motion: reduce` 時 modal 開合無 fade animation、instant

### State shape changes (useChatStore)

新增 / 修改 fields：

- `mode: ChatWidgetMode`（取代 `expanded: boolean`）
- `modalReturnMode: "bubble" | "floating" | null`（新）

移除 fields：

- `expanded: boolean`
- `width: number`
- `height: number`

新增 actions：

- `openFloating(): void`
- `minimizeToBubble(): void`
- `openModal(): void`（⌘K universal；snapshots current mode if bubble/floating；modal 已開時 no-op）
- `dockToFloating(): void`
- `closeModalToReturnMode(): void`
- `closeModalToBubble(): void`

移除 actions：

- `toggleExpanded(): void`
- `setSize(w, h): void`

`resetForVault(vaultPath)` 同時 reset `mode = "bubble"` & `modalReturnMode = null`。

### Interfaces

- ChatWidget component（`codebus-app/src/components/workspace/ChatWidget.tsx`）：read-only consumer of `useChatStore.mode` + actions；renderer 依 mode 三 branch；無新 props
- useChatShortcut hook（`codebus-app/src/hooks/useChatShortcut.ts`）：⌘K / Ctrl+K → `useChatStore.getState().openModal()`；modal 已開時 no-op

### Failure modes

- modal 開啟時 backdrop click 在某些 portal/z-index 邊界可能失靈 → reuse 既有 radix Dialog（已驗證 in NewVaultFlow）、不額外處理
- 多重 ⌘K 連按 → `openModal()` 第二次 no-op、`modalReturnMode` 不被覆蓋
- 三 mode 切換 race（user 同 tick 內按 ⌘K + click `▿`）→ Zustand store action 單 thread、最後一個 wins、不需特別處理
- modal 開啟中 vault 切換 → `resetForVault(newPath)` 同時 reset mode；spec scenario 明示

### Acceptance criteria

1. `pnpm tsc` 綠
2. `pnpm test` 綠、含：
   - `codebus-app/src/store/chat.test.ts`：6 個新 action 各自 unit test、`modalReturnMode` snapshot 行為、`resetForVault` reset mode
   - `codebus-app/src/components/workspace/ChatWidget.test.tsx`：三 mode renderer 各自 testid + a11y label 對齊新 i18n key
   - `codebus-app/src/hooks/useChatShortcut.test.tsx`：⌘K 從 toggle 改 `openModal`、modal 已開時 no-op
3. 真實 CDP smoke（截圖落 `codebus-app/scripts/.chatwidget-3modes-smoke/`、zh + en、注意 `project_cdp_smoke_webview2_pitfalls` 五雷）：
   - 開 vault 截 bubble idle / bubble + active-goal pulse / floating / modal 四張
   - 切換矩陣 6 條 row 各驗一次、截切換後 mode 狀態
   - modal 內打字後切 floating、再切 modal、verify transcript 保留
   - 切 locale 驗 zh / en aria-label 各自生效
   - 切 modal 後 tab/Shift+Tab focus 循環不逃出 modal
   - `prefers-reduced-motion: reduce`（CSSOM rule 驗、不靠 CDP `Emulation.setEmulatedMedia` per 五雷 #1）
4. archive 階段 update 四處標 archived 2026-05-27：`codebus-app/design-handoff/AUDIT.md` R7-3 / R7-modes / ODI-3 / Phase 6 sequencing

### Scope boundaries

**In scope**:

- `codebus-app/src/store/chat.ts` 新增 `mode` + `modalReturnMode` field、新增 6 個 mode-switch action、移除 `expanded / width / height / toggleExpanded / setSize`
- `codebus-app/src/store/chat.test.ts` 對應新 unit test、移除 resize / setSize / toggleExpanded test
- `codebus-app/src/components/workspace/ChatWidget.tsx` 三 mode renderer 重寫、移除 `ExpandedPanel` resize 邏輯、modal renderer 用 radix Dialog
- `codebus-app/src/components/workspace/ChatWidget.test.tsx` 三 mode renderer test
- `codebus-app/src/hooks/useChatShortcut.ts` 改 `openModal`、`codebus-app/src/hooks/useChatShortcut.test.tsx` 對應
- `codebus-app/src/i18n/messages.ts` 新 5-8 個 key（en + zh）
- `openspec/specs/app-workspace/spec.md` delta：`collapsed / expanded` 兩態 scenarios 改 `bubble / floating / modal` 三 mode；移除 resize / clamp / viewport-shrink scenarios；新增 modal mode + 切換矩陣 scenarios

**Out of scope**:

- 5.1 active-goal pulse dot 邏輯（沿用、不改）
- 6.1 RunDetailInterrupted / Phase 4 Sidebar / Content header
- ChatTranscript / ChatInput / ChatTokenDisplay / ChatNewChatButton / ChatUndoToast / promoteSuggestion 流的修改（reuse）
- 抽 chat session 到新 store（已在 `useChatStore`）
- 清 `codebus-app/design-handoff/design_files/components/cmdk-overlay.jsx` mock 殘留（archive 階段視需要）
- mode 偏好持久化（per AUDIT 5.5）
- ⌘K / Cmd+K / Esc / Ctrl+K 字面 i18n（Cat D identifier）
- Tauri WebView2 prefers-reduced-motion CDP probe（per `project_cdp_smoke_webview2_pitfalls` 雷 #1）—改驗 CSSOM rule

## Risks / Trade-offs

- **Risk**: 拿掉 `setSize` / `width / height` 破壞既有 user 已調過 size 的 localStorage state → **Mitigation**: 既有實作只在 memory（store 預設 22rem×32rem、未持久化）、無 localStorage migration risk；apply 第一步 grep `localStorage.*width` 確認。
- **Risk**: modal `prefers-reduced-motion` 在 Tauri WebView2 不 honor `Emulation.setEmulatedMedia` CDP API（per 雷 #1）→ **Mitigation**: spec scenario 規定 CSS 行為（`motion-reduce:transition-none` Tailwind variant）、smoke 驗 CSSOM rule 不靠 CDP emulate。
- **Risk**: ⌘K 跟 sidebar S7 的 ⌘K kbd chip 衝突 → **Mitigation**: grep 確認目前 sidebar S7 只是 mock 視覺 chip、無實際 keybinding；若 Phase 4 sidebar 後續綁 ⌘K 走別的行為、本 change 留 `useChatShortcut` 作為唯一 owner、conflict 屆時對齊（記錄為 Phase 6 follow-up）。
- **Risk**: radix Dialog backdrop blur 在 WebView2 性能差 → **Mitigation**: backdrop blur 2px 是 mock 規格、實機若卡頓 archive 階段降為純 55% black（無 blur）；不在本 change 寫死、留 design tweak 空間。
- **Risk**: modal 開啟中切 vault → `resetForVault` 沒 reset mode 會 stuck modal 在新 vault 但 transcript 空 → **Mitigation**: spec scenario + `resetForVault` 同時 reset `mode = "bubble"` & `modalReturnMode = null`、unit test 覆蓋。
- **Trade-off**: floating mode 拿掉 resize handle = 部分既有 user 失去 size 自訂能力 → 設計決議 lock（mock §05 5.2 + AUDIT R7-modes）、UX trade-off 換實作簡潔 + 不撞 Tauri drag region。
- **Risk**: ChatWidget `data-state` 從 `collapsed/expanded` 改 `bubble/floating/modal` 會 break 既有 E2E / unit test 依賴該值 → **Mitigation**: apply 第一步 grep `data-state` callsite + test、一併改；spec scenario 改用 `data-state="bubble"` / `"floating"` / `"modal"`。
