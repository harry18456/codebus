## Summary

擴展 ChatWidget 從現有 binary `collapsed / expanded` 兩態升級為三 mode（`bubble / floating / modal`），新增 centered modal mode 取代已 cut 的 05 Cmd+K Overlay；三 mode 共用同一 chat session、切換時對話內容保留。

## Motivation

設計 v1.1（2026-05-26 lock）把 05 Cmd+K Overlay 從藍圖砍掉、改用 ChatWidget centered modal mode 達成 spotlight 體驗（AUDIT `R7-3 / R7-modes / ODI-3` 已 spec lock）。現有 ChatWidget 只有 collapsed bubble 跟 expanded floating panel 兩態，缺：

- centered modal mode（640 wide、top 60px、backdrop blur + 55% black）作為 ⌘K 預設體驗
- 從 floating 升 modal（`⤢` expand）/ 從 modal dock 回 floating（`⤡`）/ ⌘K 從 bubble 直接跳 modal 等切換路徑
- 三 mode 共用同一 chat session 的明確 invariant（避免實作走 hidden conditional 把 session reset）
- modal mode 必備的 focus trap + restore focus on close + Esc / backdrop click 關閉

同時 `expanded: boolean` 的 state shape 已撐不住三態切換矩陣，改 `mode: "bubble" | "floating" | "modal"` reducer 才能在 spec 跟 unit test 表達切換邏輯。

## Proposed Solution

**State machine 升級**：`useChatStore.expanded: boolean` → `useChatStore.mode: ChatWidgetMode`（`"bubble" | "floating" | "modal"`），所有切換走顯式 action（`openFloating / openModal / dockToFloating / minimizeToBubble / closeToBubble`），不再 toggle。新增 `modalReturnMode: "bubble" | "floating" | null` 記錄 modal 開啟前的 mode、Esc / backdrop click 時還原（per AUDIT R7-modes 切換矩陣 row「modal · Esc / click backdrop · 回到觸發前 mode」）。

**Bubble mode**：沿用既有 collapsed bubble layout（`44×44`、`bg-raised` + `border-strong`、💬 emoji 20px）+ 5.1 留下的 active-goal pulse dot（7px amber、200ms fade、`motion-reduce` instant）+ 既有 PromoteSuggestion red badge。Click → floating；⌘K → modal。

**Floating mode**：對齊 v1.1 mock 規格、`360×460` 固定大小（拿掉現有 resize handle、`width / height / setSize` 連同 viewport-clamp `useEffect` 移除）；header 三件事 = 💬 + title「Ask about this vault」+ 右側 `⤢` expand / `▿` minimize；body / footer 沿用 ChatTranscript + ChatInput + ChatTokenDisplay + ChatNewChatButton + ChatUndoToast（不重造）；Esc / 點外面**不關閉**（黏著）。

**Centered modal mode**：reuse `src/components/ui/dialog.tsx` 既有 radix-ui Dialog primitive（已含 focus trap + aria-modal + restore focus）；640 wide × max 480 tall、top 60px from viewport top、backdrop blur 2px + 55% black；header 含 `⤡` dock to floating + `✕` close；Esc / backdrop click → `modalReturnMode`；body / footer 同 floating（**reuse 同樣** ChatTranscript / ChatInput / ChatTokenDisplay 等元件、不重造）。Modal 開啟時 input field 預設 focus。

**Chat session 共享**：現有 `useChatStore.{sessionId, turns, activeTurn, tokensTotal, promoteSuggestion}` 已在 store 層、三 mode 共享 zero-cost；ChatWidget 本身不持 session local state。

**05 Cmd+K Overlay 砍除確認**：grep 確認 `src/` 已無 Cmd+K Overlay 殘留（只在 `design-handoff/design_files/components/cmdk-overlay.jsx` 留 mock、不影響 runtime）。

**Shortcut**：`useChatShortcut.ts` 從 toggle expand 改 open modal（⌘K / Ctrl+K）；不管當下 mode 為何、⌘K 都跳 modal（per AUDIT 切換矩陣）。

## Non-Goals

- **不動 5.1 pulse dot 機制**：active-goal pulse dot 渲染條件 / fade timing / `useGoalsStore.activeRun` 偵測規則沿用既有實作、僅在新 bubble mode renderer 內 inherit。
- **不動 6.1 RunDetailInterrupted**：另一 surface、不衝突。
- **不動 Phase 4 Sidebar / Content header**：不同範圍。
- **不在 ChatWidget 內加 chat session 邏輯**：session ownership 留在 `useChatStore`、ChatWidget 只是渲染層 + mode reducer。
- **不把 emoji 💬 改成 MessageSquare icon**：design v1 R7-1 已 pushback ack、keep 💬。
- **不復活 05 Cmd+K Overlay**：centered modal 取代。
- **不做大幅 mode 切換動畫**：subtle fade only、`prefers-reduced-motion` 提供 instant。
- **不譯 `⌘K` / `Cmd+K` / `Esc` 字面**：identifier 性質、Cat D 不譯。
- **不持久化 mode 偏好**：每次 vault open 都從 bubble idle 開始（per AUDIT 5.5「Mode 偏好不持久化」）。
- **不在本 change 抽 chat session 到新 store**：已在 `useChatStore`、不重構。
- **不改既有 i18n key**：value-only 新增 mode-aware aria-label 新 key（per Phase 4A G-copy-2 教訓）。

## Alternatives Considered

- **保留 `expanded: boolean` 加第二 boolean `modal`**：撐不住三態 invariant（兩 boolean 有 4 種組合、`{expanded: true, modal: true}` 無語意），且切換邏輯散落多處 conditional。捨棄、改顯式 enum。
- **三 mode 各持獨立 session**：違反 AUDIT R7-modes「同 widget 不同呈現」mental model；user 從 bubble 開 floating 看不到 history、UX 災難。
- **自寫 modal + focus trap**：codebase 已有 `src/components/ui/dialog.tsx`（radix-ui-based）提供 a11y、reuse 即可。

## Impact

- Affected specs: `app-workspace`（modified — chat widget state machine 改 `collapsed/expanded` 兩態 → `bubble/floating/modal` 三 mode、新增 modal 行為 + 切換矩陣 scenarios、移除 resize handle 相關 scenarios）
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/ChatWidget.tsx
    - codebus-app/src/store/chat.ts
    - codebus-app/src/store/chat.test.ts
    - codebus-app/src/hooks/useChatShortcut.ts
    - codebus-app/src/hooks/useChatShortcut.test.tsx
    - codebus-app/src/components/workspace/ChatWidget.test.tsx
    - codebus-app/src/i18n/messages.ts
  - New:
    - codebus-app/scripts/.chatwidget-3modes-smoke/ (CDP smoke 截圖目錄)
  - Removed:
    - 既有 resize handle DOM + `chat-widget-resize-handle` testid + `chat.widget.aria.resizeChat` / `chat.widget.title.dragToResize` i18n 引用點（key value 保留待 archive 階段決定是否清）
- Affected design-handoff anchors（archive 階段同步標 archived 2026-05-27）:
  - codebus-app/design-handoff/AUDIT.md R7-3 / R7-modes / ODI-3 / Phase 6 sequencing 四處
