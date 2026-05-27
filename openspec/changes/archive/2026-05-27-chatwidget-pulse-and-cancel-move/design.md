## Context

### 為何兩個 sub-feature 綁同 change

ODI-4（ChatWidget pulse dot）+ R7-2（RunDetailRunning Cancel 搬位）在視覺上是「右下角 action 區 collision」的 rationale 共生體：

- ChatWidget 收合圓鈕固定 viewport 右下角（`bottom: 48px / right: 16px`、3rem × 3rem 圓鈕、`z-50`）
- 02a `RunDetailRunning` 目前 Cancel button 在 `<footer>` `justify-end`、`variant="danger"` 紅色圓角矩形按鈕，視覺上跟 ChatWidget 圓鈕同樣是「右下角紅/重視覺權重元素」
- 加 pulse dot 到 ChatWidget 圓鈕之前若不先把 Cancel 搬離右下角，pulse dot（accent 色點）+ Cancel button（danger 色圓鈕）+ ChatWidget bubble（中性 raised 圓鈕）三者擠在同一區，使用者掃視時很容易誤觸 Cancel

把兩個 sub-feature 切兩個 change 會出現「中間狀態 visually worse」的暫態（先做 pulse 但 Cancel 還在右下角 → 三元素擠一起；先搬 Cancel 但沒 pulse → user 還是看不到 active goal）。綁同 change 才能一次到位。

### Pre-apply 校準

依 `project_phase_3a_blind_spots_cleanup_lessons` lesson 1：ground truth grep 結果（2026-05-27）。

**ChatWidget.tsx 現況**：

- collapsed bubble：`h-12 w-12 rounded-full` 圓鈕、emoji `💬`、`fixed z-50`、`bottom: 48px right: 16px`
- 已有 `chat-widget-promote-badge`：紅點、`absolute right-1 top-1 h-2.5 w-2.5`、`bg-error`、條件 `promoteSuggestion` truthy
- ChatWidget 目前 **未** subscribe `useGoalsStore`、需新增 active-run subscription
- ChatWidget 接 `vaultPath` prop，但本 change 不需用 —— `useGoalsStore` 已在 store 內 vault-scope（`_currentVaultPath` field、vault 切換時 `set({ activeRun: null })`）

**RunDetailRunning.tsx 現況**：

- header 容器標 `data-tauri-drag-region`，內含 ← back link / goal text / `running-badge` StatusPill；**整個 header 都標 drag region**，加 right action 必須跳出
- header 已有 `pr-[160px]` padding（給 Windows traffic light 留空）—— 加 Cancel 必須在這個 padding 範圍**內**（不能擠進 traffic light 區）
- Cancel button 在 `<footer>` 內、`variant="danger"`、`data-testid="cancel-button"`、disabled 條件 `activeRun.cancelling`、label 兩態 `cancelButton` / `cancellingButton`

**globals.css 現況**：

- 已有 `@keyframes status-pulse` + `prefers-reduced-motion: reduce` media query 完整 setup（Phase 2 已 promote）
- `RunListItem.tsx` 已用 `animate-pulse` Tailwind utility（reduced-motion 自動退化）
- 結論：本 change 不需新增 keyframes / motion CSS —— fade in/out 走簡單 `transition-opacity duration-200`，reduced-motion 退化由 Tailwind / 既有 query 處理

**i18n module 現況（2026-05-27 apply preflight 校準補錄）**：

- 本 change 原 propose 階段在 proposal / design / tasks 內把 i18n 路徑寫成 `codebus-app/src/i18n/en.json` + `zh.json` 兩個 JSON 檔，apply 階段 preflight critical 抓到此 drift —— disk 上實際是單一 TS module `codebus-app/src/i18n/messages.ts`，內含 `messages.en` + `messages.zh` 兩個 `Record<string, string>` bundle，鍵集合由 TS 型別 (`keyof typeof messages.en`) 強制兩 bundle 對齊
- 既有同 surface key：`chat.widget.aria.openChat`、`chat.widget.aria.closeChat`、`chat.widget.aria.resizeChat`、`chat.widget.aria.minimizeChat`（兩 locale 皆有）
- 本 change 新增 key SHALL 同步加入 `messages.en` 與 `messages.zh` 兩個 const object literal 內、保留既有 dotted 鍵命名慣例 `chat.widget.aria.<purpose>`
- 教訓（同 `project_phase_3a_blind_spots_cleanup_lessons` lesson 1、`feedback_spectra_propose_grep_naming_first` 補強）：propose 階段對 i18n 介質的假設（JSON vs TS module）必須先 `Glob codebus-app/src/i18n/**` 校準；本次 apply preflight 才抓到、archive 前已 in-place 修正三個 artifact 與本段落

## Goals / Non-Goals

**Goals:**

- ChatWidget collapsed bubble 在「當前 vault 有 active goal」期間顯示 accent pulse dot、user 不在 Goals tab 也能察覺 goal 還在跑
- 02a `RunDetailRunning` Cancel button 搬離右下角 footer、改放 header right action slot、跟 ChatWidget 圓鈕在 viewport 兩個不同區
- 兩者共存時無視覺 collision、無誤觸風險、無 drag region 卡住 click
- 全部走既有 token（`--color-accent`、`bg-error`、Tailwind motion utility），不新增 design-system spec requirement

**Non-Goals:**

- ChatWidget 三 modes（bubble / floating / centered modal）—— Phase 6 範圍
- Stream tail rendering 改動 —— Phase 5.2 / 5.3 範圍
- `RunDetailInterrupted` / `RunDetailCancelled` component 本體調整 —— Phase 6 範圍
- 新增 amber pulse 動畫 keyframes —— reuse 既有 `status-pulse` 概念與 `prefers-reduced-motion` query、用 `transition-opacity` 處理 fade
- 改 `cancel-button` testid / i18n key 名稱（value-only）
- ODI-3 centered modal、ChatWidget bubble 動畫升級

## Decisions

### ChatWidget pulse dot 與 promote badge 並存

ChatWidget 圓鈕目前已有 promote badge（`right-1 top-1`、紅點）—— 兩個 indicator 必須能同時顯示。

**選**：promote badge 維持 `right-1 top-1`（右上角內側），pulse dot 放 `right-0.5 top-0.5`（更外側）並用不同 color（`bg-accent` vs `bg-error`）+ 不同 size（pulse dot 7px、promote badge 10px）。兩者語意：promote = user 該注意的事（紅 = error severity）、pulse = goal 跑中（accent = neutral activity）。

**否**：把兩個 badge 合併成一個多色 indicator —— 違反「不同語意各自 indicator」UX 慣例、增加實作複雜度。

### Pulse dot 動畫：transition-opacity vs status-pulse keyframes

**選**：用 Tailwind `transition-opacity duration-200` 處理 fade in/out。dot 元素**永遠 mount**（`opacity-0` 或 `opacity-100` 切換），`prefers-reduced-motion: reduce` 時透過 Tailwind 既有 `motion-reduce:transition-none` 退化成 instant 切換。

**否**：reuse 既有 `status-pulse` infinite animation —— 那是 RunListItem running 狀態的 `animate-pulse`，不是 fade in/out 而是循環 opacity 呼吸；用在這裡會讓 dot 一直閃，干擾 reading。pulse dot 出現後 SHALL 維持靜態 opacity 100%、不循環。

**否**：unmount 元素切換 —— unmount 會直接消失（沒有 fade-out），且 React DOM diff 速度不一定能完成 200ms transition。

### Active-goal 判定：store subscription 而非 prop

ChatWidget 透過 `useChatStore` 訂 chat state、`useGoalsStore` 訂 goal state（store-level subscription）；不從 Workspace 傳 `hasActiveGoal` prop。

**選**：`useGoalsStore((s) => s.activeRun !== null)`（selector form）讓 ChatWidget 直接訂閱。

**理由**：store 已 vault-scope（`_currentVaultPath` 切換時 reset `activeRun`）、不需透過 Workspace 中介；selector 形式避免整個 store 變動就 re-render；ChatWidget 已是 `useChatStore` consumer、加另一個 store subscription 是同樣 pattern。

**否**：從 Workspace 傳 prop —— 增加 Workspace ↔ ChatWidget coupling、為了一個 boolean 多開 prop channel。

### RunDetailRunning header right action slot

**選**：在 `<header>` 內、`running-badge` `<span>` 右側、`pr-[160px]` padding **內**新增 `<span>` wrapper（**不**標 `data-tauri-drag-region`，否則 Button click 被 drag handler 吞）裝 Cancel button。Cancel button 內部結構保留 `<Button variant="danger" data-testid="cancel-button">`、disabled / onClick 不變、只調 size 為較小變體（match header bar 視覺重量）。

**理由**：

- header `pr-[160px]` 是 Windows traffic light 預留空間；Cancel slot 必須在 padding **左側**（即 header 內主 flex 區）
- `data-tauri-drag-region` 在 Cancel wrapper 上會吞 mousedown、Cancel 點不到 —— 必須在 wrapper 上**不**標 drag region；header 其他元素（back link、goal text、badge）仍保留 drag region 屬性，所以 header 整體還是可拖
- footer 移除後 activity stream `<div>` 直接 fill remaining height、`overflow-auto` 行為不變

**否**：把 Cancel 放在 metadata line（`elapsed/tokens` 那行）—— metadata 是只讀資訊區、塞 action button 語意不一致。

### i18n key：openChatWithActiveGoalRunning 新增 vs reuse

**選**：新增 `chat.widget.aria.openChatWithActiveGoalRunning`（en + zh），collapsed bubble pulse dot 顯示時動態切換 aria-label。`openChat` 既有 key 保留、無 active goal 時用。

**理由**：螢幕閱讀器使用者需要知道 dot 的語意（不只是看不到 visual dot、更要從 aria-label 聽到 "active goal running"）。reuse 同一 key 改 value 會破壞無 active goal 時的 label 語意。

## Implementation Contract

### Behavior（user-observable）

1. **Active-goal pulse dot**：當 `useGoalsStore.activeRun` 為非 null（vault 內有 goal 在跑），ChatWidget collapsed 圓鈕右上角 SHALL 顯示一顆 7px accent-coloured 圓點 dot；activeRun 變回 null 時 dot SHALL 在 200ms 內 fade-out 消失。`prefers-reduced-motion: reduce` user 看到 instant 顯隱、無 fade transition。
2. **Promote badge 並存**：當 `useChatStore.promoteSuggestion` 非空，紅色 promote badge SHALL 維持原行為（10px、`right-1 top-1`）；pulse dot 與 promote badge SHALL 可同時顯示、視覺位置不重疊。
3. **ChatWidget expanded 不顯示 dot**：當 ChatWidget 展開狀態（`useChatStore.expanded === true`），pulse dot SHALL 不渲染（panel 本身是 affordance）。
4. **Aria-label 切換**：collapsed bubble 的 `aria-label` SHALL 在 active goal 期間為 `chat.widget.aria.openChatWithActiveGoalRunning` 的翻譯值、無 active goal 時為 `chat.widget.aria.openChat` 的翻譯值。
5. **Cancel 在 header right**：02a `RunDetailRunning` Cancel button SHALL 位於 header `<header>` 內、`running-badge` 右側、`pr-[160px]` traffic-light reserved padding 左側；Cancel button SHALL 不在 `data-tauri-drag-region` 範圍內、SHALL 點得到、SHALL 不被 window drag handler 吞。Cancel button 的 onClick (`cancelGoal`) / disabled (`activeRun.cancelling`) / 兩態 label (`cancelButton` / `cancellingButton`) 行為 SHALL 與搬位前等價。
6. **Footer 移除**：`RunDetailRunning` `<footer>` SHALL 移除；移除後 activity stream 區塊 SHALL fill 原本 footer 佔用的高度。

### Interface / data shape

- 新增 testid：`chat-widget-active-goal-pulse`（dot 元素）
- 新增 i18n key：
  - `chat.widget.aria.openChatWithActiveGoalRunning`（en + zh，兩個 locale 同步新增）
- 保留既有 testid：`chat-widget`（圓鈕本體）、`chat-widget-promote-badge`（紅點）、`cancel-button`（Cancel）、`running-badge`（StatusPill）、`run-detail-back`（back link）
- 保留既有 i18n key：`chat.widget.aria.openChat`、`workspace.runDetail.cancelButton`、`workspace.runDetail.cancellingButton`、`workspace.runDetail.backLink`

### Failure modes

- ChatWidget store subscription 失敗 / `useGoalsStore` 未初始化：`activeRun` 取得 `undefined`、coerce 視為 falsy、dot 不顯示（degrade gracefully、不 crash）
- `prefers-reduced-motion` 偵測不到：Tailwind `motion-reduce:` variant 自動降為「無 transition」、由 CSS layer 處理、無 JS fallback 需要

### Acceptance criteria

1. `pnpm tsc` 綠
2. `pnpm test` 綠，含：
   - `ChatWidget` test：activeRun !== null 時 pulse dot 渲染、null 時不渲染、expanded 狀態不渲染
   - `ChatWidget` test：activeRun + promoteSuggestion 同時非 null 時兩個 indicator 都渲染
   - `ChatWidget` test：activeRun 非 null 時 aria-label 為 openChatWithActiveGoalRunning 的值
   - `RunDetailRunning` test：Cancel button 渲染在 header 內、不在 footer；既有 cancel 行為（onClick / cancelling 狀態 label）等價
3. **真實 CDP smoke**（zh + en、注意 `project_cdp_smoke_webview2_pitfalls` 5 雷）：
   - 開 vault → 跑 goal → ChatWidget collapsed bubble 右上 pulse dot 出現
   - 02a header right 看到 Cancel button、點得到、不在 footer
   - 點 Cancel → goal 中斷 → dot 200ms fade-out
   - reduced-motion emulation 開啟（**注意 lesson：WebView2 不吃 CDP `Emulation.setEmulatedMedia` prefers-reduced-motion**，改用 CSSOM rule 驗）→ instant 切換
   - 截圖存 `codebus-app/scripts/.chatwidget-cancel-smoke/`
4. **i18n 完整性**：新增的 key 在 en + zh 兩個 locale 都有翻譯，無 missing key warning

### Scope boundaries

**In scope**:

- codebus-app/src/components/workspace/ChatWidget.tsx collapsed bubble 改：新增 `useGoalsStore` subscription + pulse dot 元素 + aria-label 條件切換
- codebus-app/src/components/workspace/RunDetailRunning.tsx 改：Cancel button 從 footer 搬到 header right、footer 移除、header 內新增 non-drag-region action wrapper
- codebus-app/src/components/workspace/ChatWidget.test.tsx / RunDetailRunning.test.tsx 加 test case
- codebus-app/src/i18n/messages.ts 內 `messages.en` 與 `messages.zh` 兩個 bundle 新增 `chat.widget.aria.openChatWithActiveGoalRunning` key

**Out of scope**:

- ChatWidget expanded panel layout
- ChatTranscript / stream tail rendering
- `RunDetailRunning` activity stream 內部行為
- `RunDetailDone` / `RunDetailCancelled` / `RunDetailInterrupted` header layout
- 新增 design-system spec requirement / 新 motion token / 新 color token

## Risks / Trade-offs

- **[Risk] ChatWidget 加 `useGoalsStore` subscription 引入 chat ↔ goals 雙 store 耦合** → Mitigation：用 selector form `useGoalsStore((s) => s.activeRun !== null)` 只訂 boolean 結果、不取整個 activeRun、re-render 次數最小；ChatWidget 已是多 store consumer 模式（既有 useChatStore + useT），新加一個 selector 不破壞既有架構
- **[Risk] header right Cancel button 被 `data-tauri-drag-region` 吞 click** → Mitigation：Cancel wrapper 明確**不**標 drag region；test 加 case 驗 cancel-button click 觸發 cancelGoal；CDP smoke 真實點擊驗
- **[Risk] Tailwind `motion-reduce:` variant 在 WebView2 不生效** → Mitigation：依 `project_cdp_smoke_webview2_pitfalls` lesson 1，WebView2 確實不吃 CDP `Emulation.setEmulatedMedia`；改用 CSSOM rule probe 驗 transition-duration === 0s。實機 OS 層 prefers-reduced-motion 設定（Windows Settings → Ease of Access → Display → "Show animations"）SHALL 仍正常 propagate 到 WebView2 CSS query
- **[Risk] pulse dot 與 promote badge 視覺擠** → Mitigation：promote badge 在 `right-1 top-1`、pulse dot 在 `right-0.5 top-0.5`、兩者不同 size + 不同 color、視覺可區辨；test 加 case 驗兩個並存時都 visible
- **[Risk] footer 移除後 activity stream 高度撐爆 / 視覺斷層** → Mitigation：activity stream `<div>` 既有 `flex-1` + `overflow-auto`、自動 fill footer 騰出的空間；test 加 RTL render snapshot or layout assertion
- **[Trade-off] 不重用 `status-pulse` keyframes** → 接受 dot 是 static-on 而非呼吸式 pulse；理由是呼吸 pulse 一直閃會干擾長 goal run 期間的 reading 焦點。命名雖然叫「pulse dot」、視覺實際是「fade in 後 static」、命名延續 design 約定但行為偏 indicator dot
