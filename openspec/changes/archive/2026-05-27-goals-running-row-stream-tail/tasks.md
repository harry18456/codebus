<!--
Each task description MUST state:
- the behavior or contract being delivered (what is observably true when the
  task is complete), and
- the verification target that proves completion (test, CLI invocation,
  analyzer check, manual assertion, or content review).

File paths are supporting context for locating the work, never the task
itself.
-->

## 1. Pre-apply ground-truth grep 校準

- [x] 1.1 對齊 AUDIT GP8 校準（pre-apply grep 對齊實機）寫進 design 的三點實機差距，apply 開工前再跑一次 grep 確認 `useGoalsStore.activeRun` 結構未變、`_onStreamEvent` 仍是唯一寫入點、`useWatcherEvent` 仍未帶 `VerbEvent` payload；交付物為一行 commit log（或本 task 內補一行 git output 引用）證明三項事實仍成立，否則停止改實作並回 design 重新校準。**驗證**：grep `_onStreamEvent`、`activeRun`、`goal-stream` 三個 anchor 之 hit 數與 design 描述一致；輸出貼進本 task 完成註解。
- [x] 1.2 對齊既有結構（apply 階段不要再 default、以此為準）：grep 確認 `RunListItem.tsx` flex layout 仍是 `[indicator][goal text flex-1 truncate][timestamp]` 三欄、`GoalsTab.tsx` 不直接渲染 row；如結構已變，先在本 task 紀錄差異再決定 tail 欄插入點。**驗證**：本 task 完成註解內附 grep 結果摘要 + 確認 tail 欄插入位置。
- [x] 1.3 對齊 Memory / 教訓沿用：apply 階段前確認 `codebus-app/src/i18n/messages.ts` 為單一 TS module 結構、`workspace.goals.*` 仍為 camelCase 後綴 convention、無 JSON bundle；如已遷移即停下對齊。**驗證**：grep `workspace.goals.` 取 5 條 hit 出來 quick scan，所有 key 後綴為 camelCase。

## 2. Stream Event Summary Helper Module（純 refactor 等價優先）

- [x] 2.1 [P] 在 `codebus-app/src/lib/streamEventSummary.test.ts` 為 Stream Event Summary Helper Module 寫 RED tests：table-driven 覆蓋 banner (`sync_start` / `start` / `goal` / `lint_done` / `commit_done` / `done` / `hint`)、tool_use Write/Edit（`✍️ <path>`）、tool_use Read/Glob（`🛠️ <name> · <input-summary>`）、shell command 三個 wrapper 解 + 80-char truncate、thought 回 `null`、unknown event 回 `null`。**驗證**：`pnpm test src/lib/streamEventSummary` RED（module 尚未存在）。
- [x] 2.2 抽 streamEventSummary shared helper：建立 `codebus-app/src/lib/streamEventSummary.ts` export `summarizeVerbEvent` / `bannerLabel` / `summarizeToolInput` / `writeEditPath` / `extractInnerCommand`，邏輯從 `ActivityStreamItem.tsx` 逐字搬移、保留 80-char truncate 與 `extractInnerCommand` 兩個 regex 行為等價。**驗證**：2.1 的 test 全綠（GREEN）。
- [x] 2.3 改 `ActivityStreamItem.tsx` 從 `lib/streamEventSummary` 匯入 helper、刪除 file-scope 重複定義；component 渲染 output 與 pre-extraction 等價。**驗證**：`pnpm test src/components/workspace/ActivityStreamItem` 既有 test 全綠（未改 test、純驗證 refactor 等價）。

## 3. useGoalsStore Tracks Latest Stream Event Per Run（store 擴展）

- [x] 3.1 [P] 在 `codebus-app/src/store/goals.test.ts` 為 useGoalsStore Tracks Latest Stream Event Per Run 寫 RED tests，covering：a) stream event for active run 寫入 `tailByRunId` 同時保留 `activeRun.events`；b) terminal-spawned goal（`activeRun` 為 null）的 stream event 仍寫入 `tailByRunId`；c) thought event 不寫入；d) `_onTerminal` 保留 tail 槽不清空；e) `reset()` 整批清空 `tailByRunId`。**驗證**：`pnpm test src/store/goals` RED（`tailByRunId` slot 尚未存在）。
- [x] 3.2 在 store 加 tailByRunId 而非 per-row subscribe：擴 `useGoalsStore` 加 `tailByRunId: Record<string, VerbEvent>` 初值 `{}`，`_onStreamEvent` 寫入時做 filter thought 在「寫入時」而非「讀取時」（thought event 直接跳過、其他 event 寫入），`_onTerminal` 不動 tail 槽，`reset()` 清空 tail 槽。**驗證**：3.1 的 test 全綠（GREEN）。
- [x] 3.3 在 `goals.test.ts` 新增 boundary case：vault A → spawnGoal → stream event 流入 → reset → vault B 開啟新 goal、舊 vault tail 不殘留；驗證 `tailByRunId` 不跨 vault 累積。**驗證**：新 test 通過、整支 goals.test.ts 綠。

## 4. useLatestStreamEvent Hook Provides Per-Run Tail Access（hook seam）

- [x] 4.1 [P] 在 `codebus-app/src/hooks/useLatestStreamEvent.test.ts` 為 useLatestStreamEvent Hook Provides Per-Run Tail Access 寫 RED tests：a) 已知 runId 回對應 `VerbEvent`；b) 未知 runId 回 `null`；c) 不相關 runId 的 stream event 不觸發 consumer re-render（用 render counter 觀察）。**驗證**：`pnpm test src/hooks/useLatestStreamEvent` RED（hook 尚未存在）。
- [x] 4.2 實作 `codebus-app/src/hooks/useLatestStreamEvent.ts` export `useLatestStreamEvent(runId): VerbEvent | null`，內部用 Zustand selector `useGoalsStore((s) => s.tailByRunId[runId] ?? null)` 確保只訂閱該 runId 槽。**驗證**：4.1 test 全綠（GREEN）。

## 5. Goals List Running Row Stream Tail（UI 渲染 + i18n placeholder）

- [x] 5.1 [P] 在 `codebus-app/src/components/workspace/RunListItem.test.tsx` 為 Goals List Running Row Stream Tail 寫 RED tests：a) running outcome + tail 有值 → `data-testid="run-row-tail"` 存在、文字含 event summary；b) running outcome + tail null → tail 渲染 i18n placeholder `…`；c) 非 running outcome → `data-testid="run-row-tail"` 不存在；d) tail 元素 class 含 `font-mono text-meta text-fg-secondary tabular-nums truncate`。**驗證**：`pnpm test src/components/workspace/RunListItem` RED（tail render 尚未實作）。
- [x] 5.2 [P] 在 `codebus-app/src/i18n/workspace.test.ts` 把 i18n Key for Running Tail Pending Placeholder（`workspace.goals.runningTailPending`）加入 en/zh 兩 bundle 對等性檢查列表；驗證兩 bundle 值皆為 `"…"`（U+2026）。**驗證**：`pnpm test src/i18n/workspace` RED（key 尚未新增）。
- [x] 5.3 在 `codebus-app/src/i18n/messages.ts` 的 `messages.en` 與 `messages.zh` 兩 bundle 加 `workspace.goals.runningTailPending: "…"`（U+2026 單字元）。**驗證**：5.2 test 綠（GREEN）。
- [x] 5.4 改 `RunListItem.tsx` 套用 Tail 視覺：固定欄寬 + 一行 ellipsis 與行為（observable）契約：grid 改 `[indicator][goal text flex-1 truncate min-w-0][tail max-w-[40ch] truncate hidden lg:block][timestamp]`，僅 `run.outcome === "running"` 渲染 tail；tail 內透過 `useLatestStreamEvent(run.run_id)` 取 `VerbEvent`、呼叫 `summarizeVerbEvent` 取一行字串、空值時 fallback 到 `workspace.goals.runningTailPending` i18n value。**驗證**：5.1 test 全綠（GREEN）；`pnpm tsc` 綠驗證 Interface 與 失敗模式 型別契約（`useLatestStreamEvent` 回 `VerbEvent | null`、`summarizeVerbEvent` 接受 `null` 回 placeholder）。
- [x] 5.5 套用 Reduced-motion 處理：tail 變更 default instant swap（不加 `transition-opacity`）；apply 階段若決定加 200ms fade 則同步加 `motion-reduce:transition-none`。**驗證**：本 task 視 5.4 review 結果決定是否補 fade；無 fade 走 instant swap、test 不需改；補 fade 則加 motion-reduce class 並寫一條 RTL test 驗 `prefers-reduced-motion: reduce` 下無 transition class。

## 6. Acceptance Criteria 整體驗收

- [x] 6.1 全 repo `pnpm tsc` 綠，並對齊 Scope（明確 in / out）：scope 外的檔案（`RunDetailRunning.tsx` / `RunDetailDone.tsx` / `ChatWidget.tsx` / backend stream emit / Tauri command）git diff 應為 0 line。**驗證**：`pnpm tsc` exit 0、`git diff --stat` 對 scope-out 路徑為空。
- [x] 6.2 全 repo `pnpm test` 綠（含 Section 2–5 的 RED→GREEN 全部 case）。**驗證**：`pnpm test` exit 0、coverage 不退化於 RunListItem / store / hook / lib 四個目標。
- [x] 6.3 真實 CDP smoke（zh + en locale）：開 vault → 跑 goal → Goals row 右側 tail 流動 → 切到顯示 thought 時 tail 跳過保留前一條 → 字超長 ellipsis → goal terminal 後 row outcome 從 running 變 succeeded、tail 區自動消失（per 行為（observable）的校準解釋）→ vault 切換後 store 槽清空。截圖存 `codebus-app/scripts/.running-tail-smoke/`，至少 6 張覆蓋上述狀態 × 2 locale。**驗證**：開跑前先掃 `project_cdp_smoke_webview2_pitfalls` 5 雷；截圖人工 review。
- [x] 6.4 效能驗：跑一條 >50 events/sec 的 stream session（可用 mock 灌入或真實 high-fanout goal），dev tools Performance panel 確認 Goals list render 不掉 frame（main thread frame >16ms 比例 <5%）。**驗證**：performance trace 截圖貼進本 task 完成註解。
