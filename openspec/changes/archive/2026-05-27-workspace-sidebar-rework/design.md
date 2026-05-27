## Pre-apply 校準（AUDIT vs 實機差異）

Per `project_phase_3a_blind_spots_cleanup_lessons` lesson 1，propose 階段先做了 ground truth grep。以下差異須在 apply Task 1.1 再確認、並落地到 component / scenario：

| AUDIT 描述 | 實機現況（2026-05-27） | apply 處理 |
|---|---|---|
| S3「drop `VAULT` section label」 | Workspace.tsx 目前 sidebar 沒有任何 section label（design v1 mock 才有） | spec 寫「SHALL NOT 顯示」；component 不需刪、加 comment 標明決策來源即可 |
| S4「📂 → Files / Vault nav」 | 現況沒有 Files nav；tab 是 Goals / Wiki / Quiz | 採 design v1 mock 對應：🚏 Goals / 📂 **Wiki** / 🎓 Quiz（user prompt 文字屬筆誤、以 mock 為準）|
| S7「drop refresh button」 | sidebar 目前沒 refresh button（design v1 mock 才有） | spec 寫「SHALL NOT 顯示 refresh control」；component 不需刪 |
| S6 active state | 現況 `bg-accent/20 text-accent` 整塊 amber 填充 | 改為左側 2px amber bar；整塊填充弱化或拿掉 |
| S7 footer | 現況 sidebar 無 footer | 新增 footer row |
| F1 BottomStrip | 現況 `App.tsx` 對所有 route 都 render `<BottomStrip>` | 加 `route.kind === "lobby"` gating |

實機檔案路徑：sidebar 程式碼集中在 Workspace 元件的 `<aside data-testid="workspace-sidebar">` 區塊（含 `TabButton` helper）+ 同檔的 `<button data-testid="workspace-back">` 與 vault name/path 區塊；BottomStrip mounting 在 App 元件的 `AppShell` return。

## Context

- Phase 4A `lobby-holistic-refresh` 已建立 Lobby visual baseline（amber pill / footer token / SectionLabel caps 變體）並 archived。
- Workspace sidebar 仍是 v3-app-workspace stub 時期樣貌：純文字 tab、active 用 `bg-accent/20 text-accent` 整塊填充、無 emoji / count / footer。
- Settings modal 目前唯一觸發點是 BottomStrip 的 settings gear，由 `AppShell` 在 Lobby + Workspace 兩 route 都 render。
- ChatWidget 用 `useChatShortcut` 綁 `Cmd/Ctrl+K` toggle、`useChatShortcut` 只在 Workspace mount 期間生效（per `app-workspace` spec）。
- 三條 nav count 來源狀況：
  - Goals: `useGoalsStore().runs` 已有、Workspace mount 時 `refreshRuns` + watcher 訂閱會自動更新。
  - Wiki: `useWikiStore().pages` 已有、Workspace mount 時 `listPages` + watcher 訂閱會自動更新。
  - **Quiz: 沒有 quiz store**。QuizTab 元件用 component-local `useState<QuizAttemptMeta[]>` + 自己 mount/watcher 拉 `listQuizAttempts`，sidebar 無法直接訂閱。

## Goals / Non-Goals

**Goals:**

- Workspace sidebar 跟 Lobby Phase 4A 同 design language（左 amber bar / 統一 SectionLabel 政策 / footer 收斂）。
- 三條 nav row 都即時顯示 store-driven count；vault 開啟期間 goal 新增 / wiki page 變化 / quiz attempt 新增都會即時反映到 sidebar。
- Settings modal 在 Workspace 由 sidebar footer 觸發；BottomStrip 不再在 Workspace render。
- 不新增 i18n key、不破壞既有 sidebar 行為（Back to Lobby / vault path open / 3 tab switch / quiz home re-tap）。
- 純 frontend 改動、零 backend；可一天內完成、CDP smoke 驗收。

**Non-Goals:**

- 不重構 QuizTab 既有 attempts loader / 既有 phase machine；只新增「sidebar 可訂閱的 quiz count seam」。
- 不為 Workspace 提供 BottomStrip 替代品；版本號在 Workspace 不顯示是接受的取捨。
- 不抽出 Sidebar / SidebarFooter 共用元件給其他畫面用；本 change 鎖在 Workspace 範圍內 inline rewrite（若 Phase 4C 顯示需要再抽）。
- 不動 ChatWidget 圓鈕、Cmd+K 行為（Phase 5 範圍）。

## Decisions

### Sidebar nav row visual contract (S4 + S5 + S6)

把現有 `TabButton` 改成三段式 layout：

```
[左 2px amber bar · active 才顯示]  [emoji span aria-hidden]  [label]  [右 mono count]
```

- emoji：用 `<span aria-hidden="true">` 直接 inline 寫進 component；不進 i18n value（per Phase 4A G-copy-2 教訓、且 emoji 跨 locale 不變）。spacing 用固定 gap（如 `gap-2`）、不要靠純空白字串排版。
- count：`<span className="font-mono tabular-nums text-meta text-fg-tertiary">{n}</span>`，靠右；store 還沒載 / count = 0 仍顯示 `0`（informational、per AUDIT S5 「empty 時顯 0 是 informational」）。
- active bar：採 `border-l-2 border-accent` 或 `before:` pseudo-element；非 active 不顯示 bar（不是用透明 4px 占位）。同時把現有 `bg-accent/20 text-accent` 整塊填充改成「active label 用 `text-fg` 或加粗 / 微 amber tint」之類弱表態，避免雙重視覺信號。
- focus ring 與 hover 行為保留現況；keyboard nav 不變。

**替代方案**：
- (a) 抽 `<SidebarNavRow>` 共用元件 → 砍掉、單一 caller、抽出無 ROI、增加閱讀成本。
- (b) 用 CSS pseudo-element 而非 border → `border-l-2` 更簡單、跟 Phase 4C content header h1 視覺策略對齊；不需處理 z-index。
- (c) emoji 進 i18n value → 砍掉、Phase 4A 教訓 + 跨 locale emoji 不變、徒增翻譯 surface。

### Quiz count seam (S5 quiz nav)

新增 `useQuizHistoryStore`（最小版本）：

- state: `{ vaultPath: string | null, attempts: QuizAttemptMeta[], loading: boolean }`
- actions: `loadAttempts(vaultPath)` / `reset()`
- selector pattern: `useQuizHistoryStore((s) => s.attempts.length)` 給 sidebar 用
- 訂閱 quiz history watcher event：跟 QuizTab 既有 `useWatcherEvent("quiz-changed", ...)` 同一個 channel；store 自己 subscribe（mount 階段做、unmount 解除）；或由 Workspace mount 統一觸發 load + 訂閱。
- Workspace mount 時 `loadAttempts(vault.path)`；unmount 時 `reset()`，跟既有 `goalsReset()` / `wikiReset()` 同樣的生命週期。
- QuizTab 仍然保留自己的 attempts state 與 phase machine 不動（避免大改、降風險）；count 顯示專用一個 store、attempts 詳細列表還是 component 拉。對 IPC 來說是兩支獨立 `listQuizAttempts` 呼叫，初期可接受；apply 階段若發現重複 fetch 太頻繁，可再評是否把 QuizTab 改成讀新 store。

**替代方案**：
- (a) 把 QuizTab attempts 整個搬到新 store、QuizTab 改用 selector → ROI 高但 blast radius 大（QuizTab 既有 phase machine 跟 attempts state coupling 不淺）；本 change 工時 1 天、不適合。
- (b) Workspace 自己拉 quiz count（不開 store）→ Workspace 跟 IPC contract 直接耦合、未來想加 cache / 多 consumer 都要再重構；store seam 是更乾淨的形。
- (c) 沿用 component-local state、靠 ref / callback 上拋 count → prop drill，user 明確說「不要 prop drill」。

**Trade-off**：apply 期間驗證雙重 fetch 對 IPC 壓力 → 若可忽略，沿用；若顯著，把 QuizTab 改成 store consumer 列入 follow-up（不在 4B 範圍內）。

### Sidebar footer 結構 (S7)

在 `<aside>` 末端新增 footer row（`flex justify-between items-center`）：

- 左：`<button>` settings icon（lucide `Settings`，跟 BottomStrip 同 icon）；aria-label / title 重用既有 `bottomStrip.settings` i18n key（不新增 key）；onClick 打開 SettingsModal。
- 右：`<span>` 內 `<kbd>⌘</kbd><kbd>K</kbd>` chip（design v1 spec 規格、跟 design-handoff 中的 sidebar mock 對齊）；aria-hidden（純視覺）；chip 文字 `⌘K` 屬 Cat D（不翻譯）。
- footer 與其上 nav 之間：`mt-auto` 讓 footer 黏 sidebar 底部；上方視需要加 `border-t border-border` 微分隔。
- **不**加 refresh button（per AUDIT S7 決策 + watcher 已存在、手動 refresh 是視覺噪音）。

**Settings open 路徑**：

- 現有 `settingsOpen` state 由 `AppShell` 持有、`SettingsModal` 在 `AppShell` 層 render。Workspace 需要一個觸發點傳遞。
- 把 `onOpenSettings: () => void` 從 `AppShell` 傳進 Workspace 元件、再傳到 sidebar footer 的 settings button。SettingsModal 本身仍在 `AppShell` 層 render（modal 全域可達），避免在 Workspace 內部 mount modal 造成 unmount 時 state 流失。

**替代方案**：
- (a) 把 SettingsModal 搬進 Workspace → 砍掉、Lobby 也要 modal、避免 mount/unmount 重複。
- (b) 用 context provider 暴露 openSettings → 砍掉、過度抽象、單一 consumer。
- (c) 把 settings open state 移進 zustand store → 砍掉、UI-only state 沒持久化需求、props 即可。

### BottomStrip Lobby-only (F1)

`AppShell` 把 BottomStrip 包進 `route.kind === "lobby"` 條件 render：

- Workspace route 完全不 render BottomStrip → DOM 不存在、無 `data-testid="bottom-strip"`。
- Workspace 內 sidebar footer settings button 共用同一個 `setSettingsOpen(true)`（由 `AppShell` props 傳下去）。
- 版本號在 Workspace 不顯示：可接受取捨；spec scenario 明寫「Workspace SHALL NOT render BottomStrip」即可。

**替代方案**：
- (a) BottomStrip 內部判斷 route 自我隱藏 → 砍掉、把 routing 知識洩漏到 BottomStrip、組件邊界不對。
- (b) 在 Workspace 內部塞個小版本號顯示 → 砍掉、Non-Goal 已排除。

### Section label 政策 (S3)

spec 寫「Workspace sidebar SHALL NOT 在 nav 區頂部顯示 section label（包括 design v1 mock 中的 `VAULT` label）」；component 不需要刪任何東西、本來就沒。在 Workspace 元件 sidebar 區塊上面加一行 comment 引用 AUDIT S3 決策來源、避免日後 reviewer 又把 label 加回來。

## Implementation Contract

**對 end user 的觀察結果**：

1. 進入任一 vault Workspace：
   - sidebar 左欄頂端是 「← 回到 Lobby」 + vault display name + vault path（**不變**）。
   - nav 區頂端**沒有** `VAULT` 等任何 section label。
   - 三條 nav row：行內依序「🚏 Goals 12」/「📂 Wiki 38」/「🎓 Quiz 5」（emoji + label + 右側 mono count）。
   - 目前 active 的 row 左側有 2px amber 垂直 bar；非 active 行不顯示 bar。
   - sidebar 底部一條 footer row：左側 settings icon（hover/title 仍是「Settings」）+ 右側 `⌘K` kbd chip。
   - sidebar 底部**沒有** refresh button。
   - 應用視窗最下方**沒有** BottomStrip（即 `data-testid="bottom-strip"` 在 Workspace 不存在）。

2. 回到 Lobby（`Back to Lobby` 點擊後）：
   - 應用視窗最下方仍有 BottomStrip（settings gear + 版本號 `v3.0.0`）。
   - Lobby 本身視覺**不變**（Phase 4A 結果保留）。

3. 點 sidebar footer settings icon：SettingsModal 打開、行為跟現在從 BottomStrip 點 settings 完全等價（同一 modal 實例）。

4. count 即時性：
   - 在 Workspace 內 New goal → Goals row count +1（無 page refresh / 無 tab 切換）。
   - watcher 觸發 wiki page 新增 → Wiki row count 跟上。
   - 答完一份 quiz → Quiz row count +1（watcher trigger）。

5. nav 切換無 flicker：點 Goals → 點 Wiki → 點 Quiz，amber bar 跟著 active row 走、不留殘影、無視覺閃爍。

**Interface / 資料形狀**：

- 新 store `useQuizHistoryStore`（新增檔案放 `codebus-app/src/store/quiz-history.ts`）：
  - state: `vaultPath: string | null`、`attempts: QuizAttemptMeta[]`、`loading: boolean`
  - actions: `loadAttempts(vaultPath: string): Promise<void>`、`reset(): void`
  - selector pattern: `useQuizHistoryStore((s) => s.attempts.length)`
- Workspace 元件新增 props: `onOpenSettings: () => void`（由 `AppShell` 注入）。
- `AppShell` 改動：BottomStrip render 包 route 條件；`onOpenSettings` 傳 Workspace。
- i18n keys：**不新增**；sidebar footer settings button 重用 `bottomStrip.settings`；其他既有 `workspace.*` key 不動。

**Failure modes**：

- `loadAttempts` IPC 失敗：sidebar Quiz count 顯示為 0；不彈 toast（不打斷用戶）；console.error log。QuizTab 本身錯誤處理不變。
- watcher event payload 不含 vaultPath 或不匹配當前 vault：store 忽略事件（pattern 跟 wiki / goals store 一致）。
- 切 vault：Workspace 元件 unmount 時呼叫 `useQuizHistoryStore.getState().reset()`、跟 goals/wiki reset 同 timing。

**Acceptance criteria**（apply 完成時須滿足）：

- `pnpm tsc` 通過。
- `pnpm test` 通過；至少新增 / 更新：
  - Workspace 元件測試驗 sidebar 渲染 emoji prefix、mono count、active left bar、footer settings button、`⌘K` chip。
  - BottomStrip 既有測試不破；`App` 層測 BottomStrip 在 Workspace route 不 render。
  - 新增 quiz-history store unit test（load / reset / watcher event handling）。
- **CDP smoke**（zh + en locale）截圖存 `codebus-app/scripts/.sidebar-rework-smoke/`，驗收項目見 proposal 驗收清單。
- 切換 nav 5 次連續點擊不出現 amber bar 殘影（DOM inspect 確認 active row 只有一條 bar）。
- count 即時性：新建 goal 後不 reload 即見 Goals count +1。

**Scope boundaries**：

- 在範圍內：Workspace sidebar 結構 / nav row 渲染 / sidebar footer / App 元件 BottomStrip conditional / 新增 useQuizHistoryStore / 兩個 spec delta（`app-workspace`、`app-shell`）/ 對應 component test。
- 不在範圍內：Workspace 右側 content area / Goals / Wiki / Quiz 任一 tab 內容 / ChatWidget / SettingsModal 內部欄位 / i18n 新增 key / 版本號替代位置 / Sidebar 共用元件抽出（除非 4C 需要再開）。

## Risks / Trade-offs

- [Risk] Quiz count 兩支 fetch（store + QuizTab 自己）造成 IPC 重複呼叫 → Mitigation: 兩支都靠 watcher 推送，loading-time 各自 once；若實機觀察到顯著重複，列入 follow-up 改 QuizTab 走 store。
- [Risk] active row 從整塊填充改 left bar 後，視覺對比度可能不足、user 不易看出「我在哪頁」 → Mitigation: amber bar 同時加 active label 微 amber tint 或加粗；CDP smoke zh + en 兩 locale 都看。
- [Risk] BottomStrip 隱藏後 version label 從 Workspace 消失，使用者找不到版本 → Mitigation: 接受取捨（per Non-Goal）；Workspace 內 user 已知是哪個 vault 哪個 tab、version 在 Settings modal 內可加（不在本 change 範圍）。
- [Risk] CDP smoke 撞 `project_cdp_smoke_webview2_pitfalls` 五雷（emulation / React batching / Tailwind transition / 副作用累積 / Settings save flow） → Mitigation: 開跑前過一次 memory checklist；切 locale 走 `settings-save` testid、不用 Esc；amber bar 切換驗證做兩段 eval 而非同 eval click + query。
- [Risk] Quiz history store 與 QuizTab 兩處對 `QuizAttemptMeta[]` 認知差 → Mitigation: 新 store 不負責 phase machine、只負責 `attempts.length`，QuizTab 不受影響。
- [Risk] Workspace 元件多一個 `onOpenSettings` props 傳遞鏈讓 component 邊界看起來變鬆 → Mitigation: 單一 callback、傳一層、不堆疊；不引入 context。
