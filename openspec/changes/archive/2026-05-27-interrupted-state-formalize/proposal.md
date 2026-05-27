## Summary

合併 `RunDetailCancelled` 與 `RunDetailInterrupted` 兩個並存且 99% duplicate 的 component 為單一 `RunDetailInterrupted`，按 v1.1 mock §02c 正式化 Interrupted state 的 state machine + 視覺，並在 run log 上新增 `interrupt_reason` 欄位以區分 app-close / user-cancel / network-drop 三類中斷原因。

## Motivation

- **兩 component 並存的歷史包袱**：`codebus-app/src/components/workspace/RunDetailCancelled.tsx` 同檔同時 export `RunDetailCancelled`（outcome=cancelled|failed）與 `RunDetailInterrupted`（outcome=interrupted），兩者 layout / header / banner / footer / partialTimeline / NewGoalModal pre-fill 行為**幾乎一模一樣**，只差 testid 與 i18n key。維護兩份等同邏輯易產生不對稱 bug（例如現況 `RunDetailCancelled` 的 badge 仍用 `StatusPill status="interrupted"` 顯示 cancelled — 一處已經偏移）。
- **mock §02c 把 Interrupted 列為第三正式 state**：`codebus-app/design-handoff/v1.1-mocks.html` 第 1158 行起的 02c 區明確要求「v1.1 第三個正式 goal-detail state — Running / Done / Interrupted」、改名 `RunDetailCancelled` → `RunDetailInterrupted`、依 backend `interrupt_reason` 切 banner 文案（app-close / user-cancel / network-drop）、並對 Failed（red）vs Interrupted（amber）拉開視覺色語。實機尚未對齊。
- **AUDIT 已記錄此 obsoleted 5.1 應併入本 change**：`codebus-app/design-handoff/AUDIT.md` 2137-2152 行載明原 Phase 5.1 純 rename change 已 obsolete，rename + 行為合併 + state machine 整理 + backend `interrupt_reason` 接通全部併入本 `interrupted-state-formalize`。
- **Frontend `RunOutcome` 已含 interrupted virtual variant，但 backend 無 reason 欄位**：`codebus-app/src/lib/ipc.ts` 588-613 行 comment 指出 interrupted 是「orphan log 補成 virtual entry」、不寫真實 RunLog row。要顯示 banner 子變體就必須在 run log projection 上補 `interrupt_reason` 欄位（Optional + backward compat）。

## Proposed Solution

依照「mock §02c 為視覺契約來源、AUDIT 為決策記錄、實機現況為起點」三方對齊，拆 4 個面向處理：

**A · Component 合併 + rename**
- 把 `RunDetailCancelled.tsx` 內兩個 export 合併成單一 `RunDetailInterrupted` component；檔案 rename 為 `codebus-app/src/components/workspace/RunDetailInterrupted.tsx`，test 檔同步 rename 為 `RunDetailInterrupted.test.tsx`。
- `codebus-app/src/components/workspace/Workspace.tsx` 約 555-585 行的 outcome switch 由「cancelled|failed → RunDetailCancelled / interrupted → RunDetailInterrupted」整併為**所有非 succeeded 終態 outcome 統一進 `RunDetailInterrupted`**，component 內部依 `outcome` + `interrupt_reason` 切顯式 state machine。
- 既有 `RunDetailCancelled` named export 全數移除、相關 import 更新。

**B · State machine 顯式化 + v1.1 mock 視覺對齊**
- Component 接 `outcome: RunOutcome` + `interruptReason?: InterruptReason` props，內部用一個顯式 switch 決定 banner sub-variant：
  - `outcome === "failed"` → 紅色語 banner（agent failure，跟 Interrupted amber 拉開、per mock 1284-1297 行 Failed vs Interrupted 對照表）
  - `outcome === "cancelled" | "interrupted"` → amber 語 banner，並依 `interrupt_reason` 細分文案（`app-close` / `user-cancel` / `network-drop`，per mock 1325/1329/1333 行）
- StatusPill 的 status 入參依 outcome 動態決定（不再寫死 `"interrupted"`），修掉 cancelled 顯 interrupted pill 的現況偏移。
- 3 sub-reason 共用同一殼層，只 banner 文案 + icon + 邊框色語不同。

**C · Backend `interrupt_reason` 欄位**
- 在 run log projection（`RunLogSummary` 同層、frontend 側 `codebus-app/src/lib/ipc.ts` `RunLogSummary` interface）加 `interrupt_reason?: InterruptReason` 欄位（Optional，未提供 deserialize 為 None / undefined）。
- Backend 側在 `codebus-core/src/log/sink.rs` `RunLog` 寫入結構新增對應 `interrupt_reason: Option<InterruptReason>` 欄位（`#[serde(skip_serializing_if = "Option::is_none")]` 維持向後相容、不影響既有 jsonl 行 schema）。
- 定義 `InterruptReason` enum：`AppClose` / `UserCancel` / `NetworkDrop` / `Other(String)`，`#[serde(rename_all = "kebab-case")]` 對齊 mock 字面（`"app-close"` / `"user-cancel"` / `"network-drop"`）。命名與既有 `#[serde(tag = "kind" | "sink" | "data")]` enum 不 collision（grep 結果：parser.rs、log/factory.rs、verb/event.rs 三處 tag 鍵都不撞）。
- 既有 RunLog jsonl（無 `interrupt_reason` 行）reload 走 None default，frontend 渲染 graceful（不 crash、不空白）。

**D · Retry 行為 + Cluster 渲染（保守維持現況）**
- Retry button 行為**維持現行 NewGoalModal pre-fill** 模式（user 在 modal 內手動按 Run 才 spawn），不改成「點 Retry 直接 spawn 新 GoalRun」。原 prompt 對「建立新 GoalRun」的描述以實機 NewGoalModal seam 解讀；如需引入「直接 spawn」行為，留待後續 change 處理。
- 內部簡化版 `partialTimeline` reading/writing/other 三行 count 維持不動，**不**在本 change 引入 5.3 `ActivityCluster` / `clusterTimeline` reuse —— 視覺對齊 mock §02c 本身沒指明改用 cluster 呈現，且引入 cluster 會擴大本 change 範圍超過 1 天工時上限。

**i18n（en + zh 兩 bundle、value-only、不改既有 key）**
新增於 `codebus-app/src/i18n/messages.ts`：
- `workspace.runDetail.banner.failedTitle` / `failedSubtitle`（紅色語 failed banner）
- `workspace.runDetail.banner.interruptedTitle` / `interruptedSubtitle`（amber 語 interrupted 殼層 fallback）
- `workspace.runDetail.banner.reason.appClose` / `userCancel` / `networkDrop` / `other`（4 sub-variant 文案）

`interrupt_reason` enum kebab-case identifier（`app-close` / `user-cancel` / `network-drop`）為 schema identifier 性質，bundle value 都填英文字面、不譯。既有 `cancelledWarning` / `interruptedWarning` / `cancelledBadge` / `interruptedBadge` key **不改名**，遷移期間以 banner.reason.* + banner.failedSubtitle 取代舊文案，待 obsolete 後再清。

## Non-Goals

- 不在「outcome」概念上開創新 variant（cancelled / failed / interrupted 已是 frontend `RunOutcome` 既有值，本 change 不擴）。
- 不把 `interrupt_reason` enum 字面翻譯（identifier 性質、Cat D）。
- 不改 `RunDetailDone` 或 `RunDetailRunning` 的視覺與行為（兩者不在合併範圍）。
- 不引入「Retry = 直接 spawn 新 GoalRun」行為（維持現行 NewGoalModal pre-fill seam）。
- 不引入 5.3 `ActivityCluster` / `clusterTimeline` reuse（本 change 維持簡化 partialTimeline 三行 count）。
- 不改 5.1 ChatWidget pulse、不改 5.3 ActivityCluster 行為、不改 Wiki / Quiz / Settings（Phase 6 其他 change 範圍）。

## Alternatives Considered

**Alt 1：純檔名 rename，不合併兩 component。** 拒絕。原 Phase 5.1 已實驗過此路線並判定 obsolete（AUDIT 2141 行），「純 rename」前提 broken（兩 export 同檔、命名 collision），rename 本身不解決 duplicate 維護問題。

**Alt 2：把 `interrupt_reason` 寫成 frontend-only synthesis（不動 backend）。** 拒絕。mock §02c 1243 行明確指 banner 文案「根據 backend 提供的 `interrupt_reason`」，且 1325/1329/1333 行寫的 reason 串（`app-close` / `user-cancel` / `network-drop`）需要 backend 在 RunLog 寫入時點分類，frontend 無從合成。

**Alt 3：把 Failed banner 拉到單獨 component（不合併進 `RunDetailInterrupted`）。** 拒絕。Failed 與 Interrupted 共用 layout 殼層（header / partial timeline / Retry footer / NewGoalModal）相同，差別只在 banner 色語 + 文案，合進同一 component 由 outcome 切 sub-variant 比拆兩 component 更少重複。

## Impact

- **Affected specs（modified）**：
  - `app-workspace`：RunDetail views 規格中「Cancelled and Interrupted」段要改成「Interrupted（含 cancelled / failed / interrupted 三 outcome sub-variant）」，明列 state machine + banner sub-variant 規則。
  - `run-log`：RunLog projection schema 新增 `interrupt_reason: Option<InterruptReason>` 欄位定義，列出 4 個 enum value + backward compat 規則。

- **Affected code**：
  - Modified:
    - `codebus-app/src/components/workspace/Workspace.tsx`（outcome switch 整併）
    - `codebus-app/src/lib/ipc.ts`（`RunLogSummary` 加 `interrupt_reason?` 欄位、新 `InterruptReason` type alias）
    - `codebus-app/src/i18n/messages.ts`（en + zh bundle 新增 banner.* key）
    - `codebus-core/src/log/sink.rs`（`RunLog` 結構新增 `interrupt_reason: Option<InterruptReason>` 欄位 + `#[serde(skip_serializing_if = "Option::is_none")]`）
  - New:
    - `codebus-app/src/components/workspace/RunDetailInterrupted.tsx`（從 RunDetailCancelled.tsx rename + 合併兩 export）
    - `codebus-app/src/components/workspace/RunDetailInterrupted.test.tsx`（從 RunDetailCancelled.test.tsx rename + 覆蓋三 outcome × 四 reason variant）
    - codebus-core 內 `InterruptReason` enum 定義（具體放在 sink.rs 或 log module 的位置由 design 決定）
  - Removed:
    - `codebus-app/src/components/workspace/RunDetailCancelled.tsx`（rename 後刪）
    - `codebus-app/src/components/workspace/RunDetailCancelled.test.tsx`（rename 後刪）

- **Backward compatibility**：legacy RunLog jsonl 行（無 `interrupt_reason` 欄位）reload 後 deserialize 為 `None` / `undefined`，frontend 渲染 fallback 走 `banner.interruptedTitle` 殼層 + 通用 amber 警示語，不 crash。

- **AUDIT 更新**：archive 階段順手把 `codebus-app/design-handoff/AUDIT.md` 三處標記 archived 2026-05-27：R7-2 partial 段、Phase 5.1 obsolete trailer、Phase 6 `interrupted-state-formalize` 條目。
