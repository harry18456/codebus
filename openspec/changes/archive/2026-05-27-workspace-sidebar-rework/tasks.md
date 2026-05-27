<!--
每個 task 都必須聲明：
- 任務完成時可觀察的行為 / 合約
- 驗證方式（test name、CLI 指令、analyzer check、手動斷言、artifact 內容檢視）
檔案路徑只是定位輔助、不是 task 本身。
-->

## 1. Pre-apply 校準

- [x] 1.1 完成「Pre-apply 校準」grep sweep：對 design.md 開頭表格中列出的 6 條 AUDIT 差異逐項到 codebus-app/src/components/workspace/Workspace.tsx、codebus-app/src/App.tsx、codebus-app/src/components/BottomStrip.tsx、codebus-app/src/components/workspace/QuizTab.tsx 確認現況跟表格一致；發現新差異就在 design.md 校準表格底下追加一行。驗證：apply session 在 chat 內貼出 6 條校準逐項結果（match / mismatch + 證據行）後才往下走

## 2. Quiz count seam (S5 quiz nav)

- [x] 2.1 [P] 建立 `useQuizHistoryStore`：契約是 `{ vaultPath, attempts: QuizAttemptMeta[], loading }` state + `loadAttempts(vaultPath)` / `reset()` actions，selector `useQuizHistoryStore((s) => s.attempts.length)` 回傳目前 vault 的 quiz attempt 總數；切 vault 時 attempts 必須清空。store 內部訂閱 `quiz-changed` watcher event（payload 為 null、同 QuizTab）並在事件觸發時重新拉 `listQuizAttempts`。驗證：`codebus-app/src/store/quiz-history.test.ts` 新增 vitest 跑 load / reset / watcher-driven refresh / mismatched vault path 4 個 case，`pnpm test -- quiz-history` 全綠
- [x] 2.2 把 `useQuizHistoryStore` lifecycle 接到 Workspace mount/unmount：Workspace mount 時呼叫 `loadAttempts(vault.path)`、unmount 時呼叫 `reset()`，跟既有 `goalsReset()` / `wikiReset()` 同 timing。驗證：Workspace.test.tsx 加 case 確認 mount 後 store 收到 vault.path 對應的 attempts、unmount 後 attempts 與 vaultPath 都清空

## 3. App shell BottomStrip Lobby-only (F1)

- [x] 3.1 修改 `AppShell`：BottomStrip render 包進 `route.kind === "lobby"` 條件；Workspace render 時不掛 `<BottomStrip>`、DOM 中無 `data-testid="bottom-strip"`。額外把 `onOpenSettings={() => setSettingsOpen(true)}` 透過 prop 傳進 `<Workspace>`，sidebar footer 可重用。驗證：`pnpm test` 跑 App 層 test 驗 Lobby route 有 BottomStrip / Workspace route 無 BottomStrip；對應 spec scenario「Bottom strip is hidden in the Workspace route」、「Bottom strip reappears when returning to the Lobby」與 `Lobby Two-State Rendering` 修正條款
- [x] 3.2 [P] 補 `Settings Modal Invocation From Workspace Sidebar Footer` 與「Lobby and Workspace share a single Settings modal instance」scenario 的 test：vitest 起 Lobby + Workspace 兩 route 各觸發 settings open，斷言 DOM 只有單一 `<SettingsModal>` 實例、`settingsOpen` 來自 `AppShell` 單一 state。驗證：相對應的 App-level test case 全綠

## 4. Workspace sidebar rewrite

- [x] 4.1 落實 `Workspace Sidebar Nav Row Visual Contract`：把 `TabButton` 改成三段式 row（左 active 2px amber bar / emoji `<span aria-hidden="true">` / label / 右 `font-mono tabular-nums text-meta text-fg-tertiary` count）。emoji 對應：🚏 Goals / 📂 Wiki / 🎓 Quiz。count selector 走 store（goals: `useGoalsStore().runs.length` / wiki: `useWikiStore().pages.length` / quiz: `useQuizHistoryStore().attempts.length`），不要 prop drill。active row 同時拿掉現有 `bg-accent/20 text-accent` 整塊填充、僅保留弱表態（label 微 tint 或加粗）讓 left bar 成為主訊號。對應 design.md decision「Sidebar nav row visual contract (S4 + S5 + S6)」。驗證：Workspace.test.tsx 增加 case 驗每條 nav row 渲染 emoji（aria-hidden）+ label + 右側 mono count + active 行的 left bar；切 active 從 Goals → Wiki → Quiz 驗 left bar 只在當前 active row、無殘影
- [x] 4.2 [P] 落實 `Workspace Sidebar Section Label Policy`：在 sidebar nav `<nav>` 元素上方寫一條短 comment 引用 AUDIT S3 決策來源（design v1 mock 中的 `VAULT` label 蓄意不採），明確標出「nav 區頂部 SHALL NOT 有任何 section label」。對應 design.md decision「Section label 政策 (S3)」。驗證：Workspace.test.tsx 加 case 用 DOM query 確認 vault path 區塊與第一條 nav row 之間沒有 `VAULT` 文字、沒有 `<SectionLabel>` 元素
- [x] 4.3 落實 `Workspace Sidebar Footer`：在 `<aside>` 末端用 `mt-auto` 加 footer row，左側 `<button>` Settings icon（lucide `Settings`、aria-label/title 重用 `bottomStrip.settings` i18n key）、右側 `<span><kbd>⌘</kbd><kbd>K</kbd></span>` 標 `aria-hidden`（`⌘K` 不翻譯）；不放 refresh button。Settings button onClick 呼叫從 props 傳入的 `onOpenSettings`，不在 Workspace 內部 mount 第二個 `<SettingsModal>`。對應 design.md decision「Sidebar footer 結構 (S7)」。驗證：Workspace.test.tsx 增加 case 驗 footer 有 Settings button + `⌘K` chip、無 refresh button；點擊 Settings button 呼叫 `onOpenSettings` mock 一次

## 5. CDP smoke 真實驗證

- [x] 5.1 跑 zh locale CDP smoke：開 Workspace 任一 vault，截 sidebar 全貌存 `codebus-app/scripts/.sidebar-rework-smoke/zh-workspace-default.png`；驗收項：nav 區無 `VAULT` 等 section label（S3 / `Workspace Sidebar Section Label Policy`）、三條 nav row 各有 emoji prefix 🚏 / 📂 / 🎓（S4 / `Workspace Sidebar Nav Row Visual Contract`）、三條 row 右側 mono count 對應 store 值（S5）、目前 active row 左側 2px amber bar（S6）、sidebar 底部 settings icon + `⌘K` chip、無 refresh button（S7 / `Workspace Sidebar Footer`）。CDP 開跑前先依 `project_cdp_smoke_webview2_pitfalls` 過五雷檢核。驗證：截圖五項目視覺檢視全綠 + chat 內貼截圖 path
- [x] 5.2 [P] 跑 en locale CDP smoke：透過 Settings modal 「Language」dropdown 切 en（一定走 `settings-save` testid 不靠 Esc），再開 Workspace 任一 vault，截圖存 `codebus-app/scripts/.sidebar-rework-smoke/en-workspace-default.png`；驗收項同 5.1。驗證：截圖檢視 + count 顯示為 ASCII 數字、emoji 渲染穩定
- [x] 5.3 跑 BottomStrip conditional smoke（F1 / `Lobby Two-State Rendering` 修正條款）：(a) 進 Workspace 後 DOM eval 確認 `document.querySelector('[data-testid=bottom-strip]')` 為 null、截圖 `codebus-app/scripts/.sidebar-rework-smoke/workspace-no-bottomstrip.png`；(b) 點 sidebar 「Back to Lobby」回 Lobby、DOM eval 確認 `data-testid=bottom-strip` 存在、截圖 `codebus-app/scripts/.sidebar-rework-smoke/lobby-bottomstrip-restored.png`。驗證：兩段 eval 結果 + 兩張截圖
- [x] 5.4 跑 active bar 切換 no-flicker smoke（`Workspace Sidebar Nav Row Visual Contract` 第三條 scenario）：CDP eval 依序點 Goals → Wiki → Files/Quiz → Goals × 5 輪（per CDP pitfall「分兩段 eval」），每段後 query `[data-active="true"]` 應只有一條 + 該條 left bar 存在；截圖 `codebus-app/scripts/.sidebar-rework-smoke/active-bar-cycle-{N}.png`。驗證：5 輪都無重複 active row、無孤立 bar 殘留
- [x] 5.5 跑 count 即時性 smoke（`Workspace Sidebar Nav Row Visual Contract` 第四條 scenario）：(a) Goals：開一個現有 vault，記下當前 Goals row count、走 NewGoalModal 觸發一條 goal、等 spawn 解析後 query sidebar Goals row count、確認 +1；(b) Quiz：同樣方式答完一份 quiz / 用 fixture 觸發 quiz-history watcher，確認 sidebar Quiz row count +1。截圖 before/after 存 `.sidebar-rework-smoke/`。驗證：兩段 count 比較 + 截圖
- [x] 5.6 `pnpm tsc` + `pnpm test` 收尾：確保所有 sidebar / store / App 層 test 全綠。驗證：兩個指令 exit 0、貼最後一行輸出
