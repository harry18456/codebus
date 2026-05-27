## Why

Phase 5.1 收的 ChatWidget pulse dot 讓 user 在 collapsed bubble 知道「vault 內有 goal 在跑」，但**仍要點進 Workspace → Goals tab → row 才能看到「現在這個 goal 在做什麼」**。AUDIT.md `GP8 · Running row 設計沒落地`（line 775+）+ Phase 5 sequencing 5.2 已標明：Goals list 內 running row 右側應該直接顯示**最新一條 stream event 的一行縮寫**（tail），讓 user 不用點進 Run detail 就能感知 goal 進度。

實機現況：`RunListItem.tsx` 只有 🚌 + truncated goal text + 相對時間，沒有 tail；`useGoalsStore.activeRun` 是單一槽且終態清空，沒有「per-run latest event」的 seam 可直接 reuse。

## What Changes

- `RunListItem`：當 `run.outcome === "running"` 時，在既有 grid 內新增右側 tail 區，渲染該 run 最新一條非 thought stream event 的一行縮寫（`✍️ <path>` / `🛠️ <name> · <input-summary>` / banner italic）。tail 樣式 `font-mono text-meta text-fg-secondary tabular-nums`、單行 ellipsis。
- `useGoalsStore`：擴 store 增加 `tailByRunId: Record<string, VerbEvent | null>` slot，由既有 `goal-stream` 訂閱寫入（filter thought 後取最新）；**terminal 時 NOT clear**，讓 tail 在 goal done 後凍結在最後一條 event。`reset()` / vault 切換時整批清空（沿用既有 reset 路徑）。
- 新加 hook `useLatestStreamEvent(runId)`：讀 `tailByRunId[runId]`，回傳 `{ tail: VerbEvent | null }`。RunListItem 透過這個 hook 拿 tail，**不 inline `useGoalsStore` 在 RunListItem 內**（保 seam）。
- 從 `ActivityStreamItem.tsx` 抽 shared helper（`bannerLabel` / `summarizeToolInput` / `writeEditPath` / `extractInnerCommand`）成 `lib/streamEventSummary.ts`（或同名 module，apply 時對齊既有 `src/lib/` 命名 convention）；RunListItem 與 ActivityStreamItem 共用此 helper。`ActivityStreamItem` 行為等價（純 refactor、不改 output）。
- i18n `messages.ts` 兩 bundle 新增：
  - `workspace.goals.runningTailPending`：tail 還沒收到第一條 event 時 placeholder（en `…` / zh `…`，純標點不翻字）

## Non-Goals (optional)

design.md 將建立，Non-Goals 寫進 design.md「Goals / Non-Goals」段。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: Goals list `RunListItem` 加 running-row stream tail 渲染；新加 `useLatestStreamEvent` hook 與 `useGoalsStore.tailByRunId` slot；抽 ActivityStreamItem summary helper 為 shared module。

## Impact

- Affected specs:
  - `app-workspace` (modified)
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/RunListItem.tsx
    - codebus-app/src/components/workspace/RunListItem.test.tsx
    - codebus-app/src/components/workspace/ActivityStreamItem.tsx
    - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx
    - codebus-app/src/store/goals.ts
    - codebus-app/src/store/goals.test.ts
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src/i18n/workspace.test.ts
  - New:
    - codebus-app/src/lib/streamEventSummary.ts
    - codebus-app/src/lib/streamEventSummary.test.ts
    - codebus-app/src/hooks/useLatestStreamEvent.ts
    - codebus-app/src/hooks/useLatestStreamEvent.test.ts
  - Removed: (none)
