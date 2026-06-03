## Summary

把 Goals 分頁的預設範例從「只在空狀態出現」改成常駐 quick-start chips，並把範例改成任何專案都適用的通用導覽式起手式（共 4 例，en/zh 對齊）。

## Motivation

同仁回報兩點：

1. **議題 3（常駐）**：再次新增 goal 需要先點 `+ New goal` 再從頭打字。預設範例只在空狀態（`goalRuns.length === 0`）渲染，一旦有任何 goal，整段 empty state 被 RECENT 清單取代，範例就消失，使用者失去快速起手的入口。
2. **議題 2（多樣性）**：現況 3 個範例（authentication flow / data ingestion pipeline / public API surface）假設了特定子系統，給人侷限感（「都是認證之類的」）。範例應改成任何專案都答得出來的高層導覽問題，貼合 codebus「替陌生程式碼建 wiki」的定位。

## Proposed Solution

- **常駐版面**：goal 清單非空時，在 RECENT 區塊上方新增一橫排 quick-start chips（沿用 SectionLabel caps 風格的區塊標籤），點 chip 直接帶 prefill 開啟 NewGoalModal。空狀態維持現有 hero + pills 結構不變，僅 pills 內容換成新範例。
- **單一來源**：空狀態 pills 與常駐 chips 共用同一組範例 i18n key（GOAL_EXAMPLE_KEYS），避免兩處內容漂移。
- **通用範例（4 例，en/zh 對齊）**：
  1. `describe what this project does` ／ 說明這個專案在做什麼
  2. `list the key dependencies and frameworks` ／ 列出主要依賴套件與框架
  3. `summarize the main features` ／ 整理主要功能
  4. `map the project structure` ／ 畫出專案結構
- 複用既有 `openModalWith` / `NewGoalModal` / `Button` / `SectionLabel`，不新造重複抽象。

## Non-Goals

- 不動後端 / IPC / goal verb，純前端 + i18n。
- 不碰 quiz 語言那個 change（另一個獨立 proposal 在處理）。
- 不改 `+ New goal` CTA、`N` 快捷鍵、RECENT 清單既有行為。
- 不引入每 vault 的 localStorage 記憶或可隱藏 quick-start 列的偏好設定。

## Alternatives Considered

- **chips 放 header CTA 旁**：空間有限，範例多時會與 `+ New goal` 按鈕擠在同一列，否決。
- **兩狀態統一成同一條 chips 列**：需重排空狀態 hero，改動面較大且失去空狀態的引導感，否決；改為「空狀態維持 hero+pills、非空狀態加 chips 列」。

## Impact

- Affected specs: `app-workspace`（Modified — Goals overview 的 empty-state 與 populated-state 範例渲染 requirement）
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/GoalsTab.tsx
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src/i18n/workspace.test.ts
    - codebus-app/src/components/workspace/GoalsTab.test.tsx
