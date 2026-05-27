## Context

### AUDIT GP8 校準（pre-apply grep 對齊實機）

AUDIT.md `GP8 · Running row 設計沒落地` 寫的 spec 段落，跟實機現況的差距：

1. **AUDIT 假設**：「Phase 5.1 應建立 `useActiveGoalRunning` 之類 hook 可 reuse」。**實機**：5.1 archive（`2026-05-27-chatwidget-pulse-and-cancel-move`）並未建立專屬 hook，ChatWidget 直接 `useGoalsStore((s) => s.activeRun != null)` 取 active-goal 信號。本 change 沿用「reuse store seam」精神，但因下列 #2 #3 限制需擴 store。
2. **AUDIT 假設**：「per-goal stream subscription seam」可 reuse `useWatcherEvent`。**實機**：`useWatcherEvent` 只有 `goal-run-changed` payload `{ run_id }`，只通知有變化、不帶 event 內容；唯一帶 `VerbEvent` 的 channel 是 `useGoalsStore` 內部 `listen("goal-stream", ...)`，且只 route 到 `activeRun.runId` 一個槽。
3. **AUDIT 假設**：「reuse 5.1 active-goal state 信號」。**實機限制**：`activeRun` 只記 user 在本 session 透過 `spawnGoal` 開的 run；terminal-spawned goal 不會進 activeRun；terminal 時 `_onTerminal` 把 activeRun 設回 null（events 一併消失）。這跟「goal done → tail 凍結」+「多 goal 同時 running 各自 tail」直接衝突。

### 既有結構（apply 階段不要再 default、以此為準）

- 元件 `RunListItem`：button + flex layout `[indicator] [goal text flex-1 truncate] [timestamp]`，3 欄。需在 timestamp 之前插 tail 欄（僅 outcome=running 時 render）。
- 元件 `GoalsTab`：list 渲染由 RunListItem 自己負責、container 不需動。
- 元件 `ActivityStreamItem`：本 change 抽 helper 但**不改 component 行為**（5.3 W4+X1 才會動）。
- Store `useGoalsStore`：在 store init 時掛 `goal-stream` listener，按 run_id route 到 `activeRun.events`。本 change 新加 `tailByRunId` slot 在同一 `_onStreamEvent` 內順手寫入。
- Hook `useWatcherEvent`：本 change 不動；tail subscription 走 store 而非新 watcher event。
- IPC 模組：本 change 不動；無新 Tauri command。
- i18n 模組：單一 TS module，`messages.en` / `messages.zh` 兩 bundle，`workspace.goals.*` 既有 key 皆 camelCase 後綴；新 key 取名 `workspace.goals.runningTailPending` 符合 convention。

### Memory / 教訓沿用

- `feedback_propose_prompt_anchor_concrete_paths`：i18n 路徑明確 anchor 到 `messages.ts` 不是 JSON（已寫進 proposal）。
- `feedback_spectra_propose_grep_naming_first`：i18n key 用 camelCase 後綴（已 grep 對齊）。
- Phase 4A G-copy-2 教訓：i18n key 不改名 value-only；本 change 不涉及既有 key rename，純新增。
- `project_cdp_smoke_webview2_pitfalls`：apply 階段 CDP smoke 開跑前掃 5 雷。

## Goals / Non-Goals

**Goals：**

- Goals list 內 running row 右側直接顯示「該 run 最新一條非 thought stream event」的一行縮寫，user 不用點進 Run detail 即可感知 goal 進度。
- tail 在 goal terminal（done / interrupted / failed）後**凍結在最後一條 event**，store 層保留、不消失。
- 沿用既有 `goal-stream` Tauri 訂閱、不新增 IPC、不新增 Tauri event channel、不動 backend stream emit schema。
- RunListItem 跟 ActivityStreamItem 共用一份 summary helper（emoji + 一行 summary 邏輯），不複製、不重造。
- `prefers-reduced-motion` 友善：reduce 時 tail 變化 instant swap、無 transition。

**Non-Goals：**

- 不動 ActivityStreamItem 元件本體 output 行為（Phase 5.3 W4+X1 範圍）；本 change 只「抽 helper module」屬純 refactor、ActivityStreamItem 等價。
- 不動 backend stream emit schema（不加 `kind` field、不改 `VerbEvent` 形狀）。
- 不做 2-phase cluster rendering（Phase 5.3 範圍）。
- 不造新的「active goal running」抽象信號（如 `useActiveGoalRunning` hook）；本 change 引入的 `useLatestStreamEvent(runId)` 是「per-run latest stream event」、與 5.1 `activeRun != null` 是不同抽象、互補不互斥。
- 不把 `ThoughtItem` 也納入 tail（一行裝不下，設計 intent 是 banner / tool_use 摘要）。
- 不 inline `useGoalsStore` 在 RunListItem 內，要透過 `useLatestStreamEvent` hook 走 seam。
- 不做大幅動畫（marquee、slide）干擾 reading。
- 不寫 GUI form 防禦邏輯（無 user input 表單），本 change 不適用 `project_phase_3a_blind_spots_cleanup_lessons` 教訓 3。

## Decisions

### 在 store 加 tailByRunId 而非 per-row subscribe

**選擇**：useGoalsStore 加 `tailByRunId: Record<string, VerbEvent>`，由既有 `_onStreamEvent` 在收到 stream event 時順手寫入（filter thought 後）；`reset()` 時清空；terminal 時**不清空**（凍結）。

**Why**：

- 既有 `goal-stream` listener 是單一 listener、不會因 N rows 增加訂閱次數；走 store map 天然解決「N rows × N events」的 fan-out 疑慮。
- terminal 時保留 entry 滿足「goal done → tail 凍結」需求；vault 切換時的 `reset()` 整批清空避免跨 vault 殘留。
- 不引入新 Tauri channel、不動 backend、改動範圍縮在 frontend。

**Alternatives**：

- (B) 每個 RunListItem 自己訂閱 `goal-run-changed` + 新 IPC `read_latest_stream_event(run_id)`：要新 Tauri command、watcher event 沒帶 payload、且 N rows × poll 不划算。**Reject**。
- (C) 新加 `goal-stream-tail` Tauri event channel 從 backend 廣播：要動 backend stream emit、跨 Phase 5.3 範圍。**Reject**。
- (D) RunListItem 直接 inline 讀 activeRun.events 最後一筆：只能服務 activeRun 那一個 row、不能凍結、不支援 terminal-spawned goal。**Reject**。

### filter thought 在「寫入時」而非「讀取時」

**選擇**：`_onStreamEvent` 寫入 `tailByRunId` 時**只 filter thought**（不寫 thought 進 tail 槽），保留 banner / tool_use；hook `useLatestStreamEvent` 直接回傳 store 值不再 filter。

**Why**：

- 寫入時 filter thought 讓 tail 槽永遠對應到「使用者該看到」的東西，hook 不再需要回放 history 找上一個非 thought。
- 不影響 `activeRun.events` 全紀錄（Running detail 仍需要 thought 渲染）；兩條路各自獨立、不互相污染。

**Alternatives**：

- (B) 寫入全部 event、讀取時往後找最新非 thought：要在 hook 內 scan `activeRun.events`、且不能服務 terminal-spawned goal（activeRun 不會收）。**Reject**。

### 抽 streamEventSummary shared helper

**選擇**：把 ActivityStreamItem 內 `bannerLabel` / `summarizeToolInput` / `writeEditPath` / `extractInnerCommand` 抽到 `codebus-app/src/lib/streamEventSummary.ts`，export pure functions（接 `t: TFunction` 注入），新加 `summarizeVerbEvent(event, t)` 一站式 facade。RunListItem + ActivityStreamItem 共用。ActivityStreamItem 行為**完全等價**（純 refactor）。

**Why**：

- 避免複製 80 行 summary 邏輯到 RunListItem。
- helper 變 testable pure function，可用 table-driven test 驗多 event 型態。
- 既有 ActivityStreamItem 測試已覆蓋行為等價的回歸面。

**Alternatives**：

- (B) RunListItem 直接從 ActivityStreamItem 匯入那兩個 function：那兩個 function 不是 export、要先 export；export 完還是兩處用、不如抽 module 乾淨。**Reject**。

### Tail 視覺：固定欄寬 + 一行 ellipsis

**選擇**：RunListItem grid 改 `[indicator] [goal text flex-1 truncate min-w-0] [tail max-w-[40ch] truncate hidden lg:block] [timestamp]`（實際 break-point 與 width budget apply 時校準 Workspace 寬度）。tail 樣式 `font-mono text-meta text-fg-secondary tabular-nums truncate`。

**Why**：

- goal text 仍是首要、tail 是輔助；用 max-width 限制 tail 不擠壓 goal text。
- 小視窗（lg 以下）藏 tail 避免擠崩 row；只在寬度充足時顯示。
- `truncate` 是 Tailwind `text-overflow: ellipsis + overflow: hidden + whitespace: nowrap`，達成一行 ellipsis 需求。

**Alternatives**：

- (B) tail 換行多行：違反 prompt「一行內」要求。**Reject**。
- (C) tail 用 marquee 滾動長文：違反「不要大幅動畫」+ `prefers-reduced-motion` 友善。**Reject**。

### Reduced-motion 處理

**選擇**：tail 內容變更時 default instant swap（無 fade、無 slide）。可選 200ms opacity fade（跟 5.1 pulse dot 同 timing）用 Tailwind `transition-opacity duration-200 motion-reduce:transition-none`。Apply 時 default 先做 instant swap、視 review 決定要不要加 fade。

**Why**：

- stream event 高頻時（>50 events/sec）若每次 fade 都跑動畫，可能掉 frame；instant swap 是安全 baseline。
- 加 fade 是 nice-to-have、不是 spec 強需求；reduced-motion 永遠不跑 transition。

## Implementation Contract

### 行為（observable）

- RunListItem：當 `run.outcome === "running"` 時，row 右側（timestamp 左側）渲染 tail 區塊；tail 內容是 `useLatestStreamEvent(run.run_id)` 回傳 `VerbEvent` 經 `summarizeVerbEvent` 轉成的一行縮寫字串。
- 當 hook 回傳 `null`（tail 槽空、尚未收到第一條非 thought event）：render `workspace.goals.runningTailPending` i18n value 作 placeholder。
- 當 `run.outcome !== "running"`：不 render tail 區（即使 `tailByRunId[runId]` 還有殘值；row 不再是 running、不應顯示 tail）。**注意**：terminal 後 outcome 由 RunLog refresh 變 `succeeded`/`failed`/`interrupted`，tail 區自動消失——這跟 prompt「tail 凍結不消失」的字面有 tension，**校準解釋**：tail 凍結指**儲存層**（tailByRunId 不清空），UI 層因 row outcome 變化自然停止 render。如此「凍結」的價值在於：若使用者在 terminal 前後快速 vault 切換 / 回看，store 仍持有歷史，不會 race condition 出現空 tail。

### Interface

- `codebus-app/src/lib/streamEventSummary.ts`
  - `summarizeVerbEvent(event: VerbEvent, t: TFunction): string | null` — 一站式 facade；thought / 無 summary 的 event 回 null。
  - `bannerLabel(banner: VerbBanner, t: TFunction): string`
  - `summarizeToolInput(input: unknown): string`
  - `writeEditPath(input: unknown): string`
  - `extractInnerCommand(raw: string): string`
- `codebus-app/src/hooks/useLatestStreamEvent.ts`
  - `useLatestStreamEvent(runId: string): VerbEvent | null` — Zustand selector 訂閱 `tailByRunId[runId]`。
- `codebus-app/src/store/goals.ts` — store slot 新增
  - `tailByRunId: Record<string, VerbEvent>` — 每 run 最新一筆非 thought event。
  - `_onStreamEvent`：除既有 activeRun.events 寫入外，順手寫 tailByRunId（thought 跳過）。
  - `reset()`：清空 tailByRunId。
  - `_onTerminal`：**不清空** tailByRunId entry。

### 失敗模式

- `summarizeVerbEvent` 收到沒有對應 summary 的 event（如 stream + thought）：回 `null`。caller 顯示 placeholder。**注意**：`_onStreamEvent` 寫入時已 filter thought，正常情況 hook 不會回到 thought，本路徑是純防禦。
- `useLatestStreamEvent(runId)` 對未知 runId：回 `null`、caller 顯示 placeholder。
- `tailByRunId` 累積但 `reset()` 整批清空（vault 切換、Workspace unmount 觸發）：避免跨 vault 殘留。

### Acceptance Criteria

- `pnpm tsc` 綠（新 types 通過）。
- `pnpm test` 綠，**新測試**：
  - streamEventSummary 測試：table-driven 覆蓋 banner / tool_use Write/Edit/Read/Glob / shell command / unknown event；驗 thought 回 `null`、驗 80-char truncate。
  - useLatestStreamEvent 測試：mock store 驗 hook 拿到 tail / 未知 runId 拿 null / store reset 後拿 null。
  - useGoalsStore 測試擴充：`_onStreamEvent` 寫入 tailByRunId、filter thought、`_onTerminal` 不清空 tail entry、`reset` 整批清空。
  - RunListItem 測試擴充：running outcome 渲染 tail、非 running outcome 不渲染、tail 為 null 渲染 placeholder、ellipsis class 存在。
  - workspace i18n 測試擴充：新 i18n key 在 en/zh 都存在且相等（純標點 `…`）。
- 真實 CDP smoke（zh + en、注意 `project_cdp_smoke_webview2_pitfalls` 5 雷）：開 vault → 跑 goal → Goals row 右側 tail 隨 stream event 流式更新、thought 跳過、字超長 ellipsis、goal done 後 row 從 running 變 succeeded（tail 區自然消失，store 槽保留）、切 vault 後再回來 tail 槽已清。截圖存 `codebus-app/scripts/.running-tail-smoke/`。
- 效能驗：dev tools performance 看 >50 events/sec 高頻 stream session 跑一遍、Goals list 不掉 frame。

### Scope（明確 in / out）

**In scope：**

- RunListItem render tail（grid 改、樣式、placeholder）
- useGoalsStore 加 tailByRunId slot 與寫入/清空邏輯
- 新 hook useLatestStreamEvent
- 新 module streamEventSummary 抽 helper（ActivityStreamItem 純 refactor 等價）
- i18n 新 key `workspace.goals.runningTailPending` 兩 bundle
- app-workspace spec 加 `Goals List Running Row Stream Tail` Requirement + scenarios

**Out of scope：**

- ActivityStreamItem 元件本體 output 行為改動（Phase 5.3 W4+X1）
- backend stream emit schema 變動（如 `kind` field）
- 2-phase cluster rendering（Phase 5.3）
- 新 Tauri command / event channel
- RunDetailRunning / RunDetailDone view 改動
- ChatWidget pulse dot 邏輯改動（Phase 5.1 結束）

## Risks / Trade-offs

- [tailByRunId 跨 vault 累積佔記憶體] → vault 切換 / Workspace unmount `reset()` 整批清空；單 vault session 內最多 N rows × 1 event ≈ <1KB。
- [terminal 後 tail 槽保留但 row outcome 變化導致 UI 停 render] → 校準解釋寫進 Implementation Contract 行為段；spec scenario 明確驗 UI 層停 render 即可、不是 store 層清空。
- [helper module 抽離後 ActivityStreamItem 行為走樣] → 純 refactor、ActivityStreamItem 既有測試 case 全綠才能 ship；apply 階段先抽 helper 跑既有 test 確認等價、再加新 case。
- [RunListItem 寬度因 tail 欄擠壓 goal text 視覺] → 用 `max-w-[40ch]` + `hidden lg:block` 限縮、小視窗自動藏 tail；apply 時對齊實機 Workspace 寬度校準 break-point。
- [stream event 高頻時 N row 同步 re-render] → tailByRunId 是 Record，Zustand selector 用 `useGoalsStore((s) => s.tailByRunId[runId])` 只訂閱該 runId 的槽、其他 row 不 re-render；apply 階段確認 selector 寫法正確。
- [terminal-spawned goal（從別處跑進來的 goal、非 spawnGoal 開啟）的 tail 來源] → `_onStreamEvent` 收任意 run_id 都寫進 tailByRunId（不只 activeRun.runId）；activeRun 限制不傳染給 tail 機制。
