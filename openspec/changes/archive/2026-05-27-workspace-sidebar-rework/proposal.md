## Why

Phase 4A `lobby-holistic-refresh` 已交付 Lobby visual baseline。Workspace 左側 sidebar 仍停留在 v1 stub 樣貌（純文字 tab、無 emoji prefix、無 count、active state 整塊填充、底部空白、Settings 從共用 BottomStrip 觸發）—— 跟 Lobby 重整後的 design language（amber bar 視覺定位 / SectionLabel 政策 / footer 收斂）脫節，user「在哪一頁」與「每個分頁多少東西」缺乏 ambient 訊號。

本 change 把 Workspace sidebar 跟 Lobby 同一 design language 對齊（per AUDIT § 01 · S3-S7 / F1，spec-locked 決策），完成 Phase 4 三步走的中間段（4A → **4B** → 4C），讓後續 Phase 4C `workspace-content-header-row` 的 active amber bar 跟 content header h1 視覺呼應更自然。

## What Changes

- **S3 · VAULT section label**：sidebar nav 區頂部不顯示 `VAULT` 等任何 section label（current code 本來就沒；本 change 把「不加」這條決策落到 spec、避免日後又被加回去）
- **S4 · Nav rows emoji prefix**：三條 tab row 開頭加 inline emoji（🚏 Goals / 📂 Wiki / 🎓 Quiz），emoji 用 `<span aria-hidden="true">` 直接寫進 component，不進 i18n value
- **S5 · Nav rows 右側 mono count**：每條 tab row 右側顯示 store-driven count（goals runs / wiki pages / quiz attempts），樣式 `font-mono tabular-nums fg-tertiary`；count 即時反映 store 變化、不靠 prop drill
- **S6 · Active row 左 2px amber bar**：active tab row 改用左側 2px amber bar 標示「你在這頁」，取代現況 `bg-accent/20` 整塊 amber 填充；非 active row 不顯示 bar
- **S7 · Sidebar footer**：sidebar 底部新增 footer row，左側 settings icon button（觸發既有 SettingsModal）+ 右側 `⌘K` kbd chip（標示 ChatWidget toggle shortcut）；不放 refresh button（current code 本來就沒；本 change 把「不加」這條決策落到 spec）
- **F1 · BottomStrip 改 Lobby-only**：S7 把 Settings 搬進 sidebar 後 BottomStrip 在 Workspace 失去存在意義，`App.tsx` 改條件 render、Workspace route 不顯示 BottomStrip；Lobby route 維持現況
- **Settings invocation source**：spec 中「Settings modal SHALL be invoked by the bottom-left gear in either Lobby or Workspace state」更新為「Lobby 由 BottomStrip 觸發、Workspace 由 sidebar footer 觸發」

## Non-Goals

- 不動 Workspace 右側 content area（Phase 4C `workspace-content-header-row` 範圍）
- 不動 Goals tab / Quiz tab 內容（Phase 4C / Phase 5 範圍）
- **不新增 i18n key**（per Phase 4A G-copy-2 教訓 + AUDIT 無新 key 指示）；既有 sidebar nav label key 保留、value 不動
- 不重構 ChatWidget 圓鈕（Phase 5 `chatwidget-three-modes` 範圍）
- 不為 BottomStrip 在 Workspace 被隱藏後的版本號顯示找替代位置（若有需要、檔案層後續再補；本 change 不開洞）
- 不修改 sidebar 其他可能存在的 section label（如有；只處理 `VAULT` 決策，per AUDIT S3）
- 不重構 QuizTab 既有 attempts loading 流程；本 change 只引入「quiz count 來源 seam」，QuizTab 內部的 attempts 仍可保留其 fetch / refresh 行為（design.md 會記錄選用的 seam 形式）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: sidebar layout 與 nav 行為改變（emoji prefix / mono count / active left bar / footer + Settings entry）
- `app-shell`: BottomStrip 改 Lobby-only render、Settings modal invocation source 在 Workspace 切到 sidebar footer

## Impact

- Affected specs: `app-workspace`, `app-shell`
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/Workspace.tsx
    - codebus-app/src/App.tsx
    - codebus-app/src/components/workspace/Workspace.test.tsx
    - codebus-app/src/components/BottomStrip.test.tsx
  - New:
    - codebus-app/scripts/.sidebar-rework-smoke/ (CDP smoke screenshot 收容資料夾)
  - Removed: (none)
- 共用元件考量：sidebar nav row + footer 元件可獨立抽出（design.md 會評估是否值得；若不抽就保留 inline rewrite）
- Quiz count seam：本 change 必須建立一條 sidebar 可訂閱的 quiz attempt count 來源（store getter / 共用 hook / 輕量 count store），具體形式於 design.md 決策；不在 Phase 4B 之外開新增能力
