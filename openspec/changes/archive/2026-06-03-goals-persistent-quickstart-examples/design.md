## Context

Goals 分頁（`GoalsTab.tsx`）目前：goal 清單為空時渲染 hero + 3 顆 prefill pills（i18n key `examplePlaceholder1..3`），非空時改渲染 RECENT 清單，**範例此時完全消失**。範例來源為元件內常數 `GOAL_EXAMPLE_KEYS`，點擊走 `openModalWith(text)` 帶 prefill 進 `NewGoalModal`。文案在 `messages.ts`（en/zh 各一份），由 `workspace.test.ts` 的 `WORKSPACE_KEYS` 守 en/zh 完整性，行為由 `GoalsTab.test.tsx` 守。

純前端 + i18n 改動，不動後端 / IPC / goal verb。

## Goals / Non-Goals

**Goals:**

- 範例常駐：goal 清單非空時，RECENT 上方仍提供 quick-start 範例入口，點擊直接帶 prefill 開 NewGoalModal。
- 範例改成任何專案都適用的 4 例通用導覽式起手式，en/zh 對齊。
- 空狀態 pills 與常駐 chips 共用同一組範例來源，杜絕內容漂移。

**Non-Goals:**

- 不動後端 / IPC / goal verb。
- 不碰 quiz 語言 change。
- 不改 `+ New goal` CTA、`N` 快捷鍵、RECENT 清單行為。
- 不引入 localStorage 記憶或隱藏 quick-start 列的偏好。

## Decisions

**D1 — 版面：RECENT 上方 quick-start chips 列（已與 user 定稿）。**
populated 狀態在 RECENT `SectionLabel` 之上、`goals-tab` 捲動容器內，插入一橫排 quick-start chips；空狀態維持現有 hero + 垂直 pills 結構不變。
- 否決「header CTA 旁放 chips」：空間有限、範例多時與 `+ New goal` 擠同列。
- 否決「兩狀態統一成同一條 chips 列」：需重排空狀態 hero、改動面大且失去空狀態引導感。

**D2 — 單一範例來源。**
`GOAL_EXAMPLE_KEYS` 擴為 4 個 key（`examplePlaceholder1..4`），空狀態 pills 與常駐 chips 皆 map 此同一陣列；兩處唯一差異是版面（pills 垂直、chips 橫排 + 區塊標籤），資料與點擊行為一致（皆 `openModalWith(t(key))`）。

**D3 — 通用範例定稿（en / zh）：**
1. `describe what this project does` ／ 說明這個專案在做什麼
2. `list the key dependencies and frameworks` ／ 列出主要依賴套件與框架
3. `summarize the main features` ／ 整理主要功能
4. `map the project structure` ／ 畫出專案結構

舊 3 例（authentication flow / data ingestion pipeline / public API surface）整組汰換，避免「侷限於認證」印象。

**D4 — quick-start 區塊標籤。**
新增 i18n key `workspace.goals.quickStartLabel`（en `Quick start` ／ zh `快速開始`），以 `SectionLabel variant="caps"` 渲染於 chips 列上方，與 RECENT 標籤視覺一致。與 RECENT（不可翻譯的識別字）不同，本標籤是一般 UI 文字，故走 i18n。

**D5 — testid 命名。**
- 空狀態 pills 維持既有 `goals-empty-prefill-{i}`（i=0..3，現由 3 變 4）。
- 常駐 chips 列新增 `goals-quickstart-chip-{i}`（i=0..3），列容器 `goals-quickstart`。

## Implementation Contract

**Behavior（觀察得到的結果）：**
- goal 清單為空：渲染 hero + 4 顆垂直 pills（testid `goals-empty-prefill-0..3`），文案為 D3 對應 locale 值；點任一顆開 NewGoalModal 且 textarea 預填該範例。
- goal 清單非空：RECENT 區塊上方渲染 quick-start 區塊（testid `goals-quickstart`），含 `Quick start`/`快速開始` 區塊標籤與 4 顆 chips（testid `goals-quickstart-chip-0..3`），文案同 D3；點任一顆開 NewGoalModal 且 textarea 預填該範例。RECENT 清單與其餘行為不變。
- 切換 locale（en↔zh）時 pills 與 chips 文案皆隨之切換；zh locale 下不得出現任何英文範例字面值（如 `describe what this project does`）。
- 所有 4 例文案皆來自 i18n key，元件原始碼不得 hard-code 範例字面值。

**Interface / data shape：**
- `GOAL_EXAMPLE_KEYS`：`readonly ["workspace.goals.examplePlaceholder1", ...2, ...3, ...4]`。
- i18n 新增/修改 key（en + zh 各一）：`examplePlaceholder1..4`（內容換為 D3）、`quickStartLabel`（新增）。
- `workspace.test.ts` `WORKSPACE_KEYS` 補入 `workspace.goals.examplePlaceholder4` 與 `workspace.goals.quickStartLabel`。
- 複用 `openModalWith` / `NewGoalModal` / `Button` / `SectionLabel`，不新增元件抽象（chips 列可為 GoalsTab 內聯 JSX）。

**Failure modes：**
- 無 runtime 失敗路徑（純展示 + 既有 modal 流程）。i18n key 缺漏由 `workspace.test.ts` 在 CI 攔截（值需為非空字串、且不得等於 key 字面值）。

**Acceptance criteria：**
- `npm run test`（Vitest）全綠，含更新後 `GoalsTab.test.tsx` 與 `workspace.test.ts`。
- 新增/更新測試涵蓋：(a) populated 狀態渲染 `goals-quickstart` + 4 顆 chips；(b) 點 chip 開 modal 並帶對應 prefill；(c) 空狀態 pills 數為 4 且文案為新 D3 值；(d) zh locale 下 chips/pills 無英文範例字面值；(e) `quickStartLabel` 與 `examplePlaceholder4` 在 en/zh 皆存在。
- `npm run typecheck` 通過。

**Scope boundaries：**
- In scope：`GoalsTab.tsx`、`messages.ts`、`workspace.test.ts`、`GoalsTab.test.tsx`、`openspec/specs/app-workspace/spec.md` delta。
- Out of scope：後端 / IPC / goal verb、quiz 語言 change、`N` 快捷鍵與 `+ New goal` CTA 行為、localStorage 偏好。

## Risks / Trade-offs

- [空狀態與非空狀態渲染兩套版面同一份資料，未來改範例需確認兩處皆吃 `GOAL_EXAMPLE_KEYS`] → 以單一來源 D2 + 測試 (a)(c) 同時覆蓋兩狀態降低漂移。
- [chips 列在範例變多時可能換行] → 本 change 固定 4 例、chips 容器允許 wrap，視覺可接受；數量再擴張屬後續 change。
- [新增 i18n key 漏補 en/zh 任一份] → `workspace.test.ts` 完整性測試攔截。
