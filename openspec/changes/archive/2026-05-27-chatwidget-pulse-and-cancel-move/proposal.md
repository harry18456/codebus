## Why

ChatWidget 收合圓鈕目前無法讓使用者在「不在 Workspace tab」或「ChatWidget 收合」狀態下察覺 vault 內仍有 goal 在跑（ODI-4：active-goal awareness gap）。同時 02a `RunDetailRunning` 把 Cancel button 放在底部 `<footer>`，視覺上跟 viewport 右下角的 ChatWidget 圓鈕屬於同一塊「右下角 action 區」，兩個都是圓形/danger 色相近的視覺重點，造成 collision + 誤觸風險（R7-2：Cancel button mislocated）。Phase 5 sequencing 把這兩個議題綁在同一塊「palate cleanser」change 處理，因為 R7-2 把 Cancel 搬離右下角後，pulse dot 加在 ChatWidget 圓鈕右上角才不會跟 Cancel 搶視覺焦點。

## What Changes

- ChatWidget 收合 bubble 右上角新增 `chat-widget-active-goal-pulse` dot：
  - 出現條件：`useGoalsStore.activeRun !== null`（vault 切換時 store 已 reset）
  - 視覺：7px 圓點、`bg-accent`（reuse Phase 2 已 promote 的 `--color-accent`）、與既有 `chat-widget-promote-badge` 紅點視覺共存（不同位置 / 不同 color / 不同 testid）
  - 動畫：fade-in 200ms 出現、fade-out 200ms 消失；`prefers-reduced-motion: reduce` 時 instant 切換（reuse `globals.css` 已有的 reduce-motion 處理慣例）
  - ChatWidget 展開狀態（expanded panel）不顯示 dot（panel 本身已 affordance、不需 dot）
- 02a `RunDetailRunning` Cancel button 從 `<footer>` 搬到 header right action slot：
  - 既有 `cancel-button` testid + i18n key + onClick / disabled hook 全部保留（value-only 等價）
  - 在 header 內新增 right-aligned action container（**不在 `data-tauri-drag-region` 區內**——drag region 吞 mousedown，會讓 Cancel 點不到）
  - 既有 `running-badge` `StatusPill` 在 header right 內保留並重排（Cancel 緊鄰 badge）
  - footer 移除
- 新增 i18n key `chat.widget.aria.openChatWithActiveGoalRunning`（en + zh，皆加入 `codebus-app/src/i18n/messages.ts` 的 `messages.en` / `messages.zh` bundle）：pulse dot 顯示時 collapsed bubble aria-label 改為宣告「有 goal 在跑」的版本；無 active goal 時退回既有 `chat.widget.aria.openChat`

## Non-Goals (optional)

- 不動 ChatWidget 三 modes（bubble / floating panel / centered modal）—— Phase 6 `chatwidget-three-modes` 範圍
- 不動 ChatTranscript stream tail rendering —— Phase 5.2 GP8 + Phase 5.3 W4/X1 範圍
- 不動 `RunDetailInterrupted` / `RunDetailCancelled` component 本體 —— Phase 6 `interrupted-state-formalize` 範圍
- 不動 ODI-3 centered modal —— Phase 6 範圍
- pulse dot 不寫死 amber 顏色字面 hex —— 走 `--color-accent` token、theme 切換時自動跟著走
- 不新增 design-system spec requirement —— pulse 動畫 reuse 既有 `status-pulse` keyframes、`--color-accent` token、`prefers-reduced-motion` query，皆屬 Phase 2 已 promote 設施
- 不改 `cancel-button` / `cancelButton` / `cancellingButton` i18n key 命名（value-only）—— per Phase 4A G-copy-2 教訓

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: ChatWidget collapsed bubble 加 active-goal pulse dot；RunDetailRunning Cancel button 從 footer 搬 header right action slot；新增 i18n key 處理 pulse-active 時的 aria-label

## Impact

- Affected specs:
  - `app-workspace` (modified)
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/ChatWidget.tsx
    - codebus-app/src/components/workspace/ChatWidget.test.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.tsx
    - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
    - codebus-app/src/i18n/messages.ts
  - New: (none)
  - Removed: (none)
