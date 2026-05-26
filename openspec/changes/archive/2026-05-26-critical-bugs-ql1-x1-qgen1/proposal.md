## Why

codebus-app design audit Phase 1 三條 critical bugs，全屬 frontend 顯示層認知衝突——user 在現有 UI 看不出哪個 quiz 是自己跑的、看不懂被 powershell wrapper 包住的 shell 指令、看到 raw 內部 marker 像 bug 字串。三條互不依賴、修改面積小、不需 backend 配合，集中一個 change 內 ship 最省。

依據：`codebus-app/design-handoff/AUDIT.md` § 結算階段 · Next Steps · 實作 sequencing § Phase 1 · Critical bugs。

## What Changes

- **QL1 · Quiz history row title 改用 user 給的 topic**：`codebus-app/src/components/workspace/QuizTab.tsx` 列表 row 渲染目前用 `topic-a7fb67fc` 這種 hash ID 當主標、user 給的真實 topic「專案目的」反而只在副標——換成 user-typed topic 當 row title，hash ID 移到副標或拿掉。
- **X1 frontend · Codex shell wrapper extraction**：`codebus-app/src/components/workspace/ActivityStreamItem.tsx` 內 `summarizeToolInput` 對 Shell tool 的 `obj.command` 直接 80 字截斷；codex provider spawn 出來的 command 都是 `"…/powershell.exe" -Command "<actual>"` 或 `sh -c "<actual>"` 包裝，wrapper 文字會吃掉 60+ 字、user 看不到真實命令。加 `extractInnerCommand(raw)` helper detect `powershell.exe -Command` / `sh -c` / `bash -c` 三種 wrapper、抽 inner command 再截 80 字。
- **QGEN1 · Internal marker filter**：`ActivityStreamItem.tsx` thought block 渲染目前會把 `[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run …` 這種內部 sentinel marker 整段 raw 顯示給 user，看起來像 bug。對 `[CODEBUS_*]` 前綴 detect 後轉成 i18n 文案（首發 case `[CODEBUS_QUIZ_NO_VALIDATE]` → 「codex 沙箱無法跑 quiz 結構驗證，跳過此步」），其他未知 marker 整段過濾。

三條共同：TDD 走 vitest、各自先寫測試再實作；i18n key 命名沿用既有 `codebus-app/src/i18n/messages.ts` bundle 結構，en/zh 兩 locale 同步補。

## Non-Goals

- **不重新設計 quiz history 列表 layout / 排序 / kebab menu**——這些屬 Phase 4 之後的批次（GP6 / QL5 等），本 change 只動 row title 取值。
- **不動 codex backend / spawn wrapping 邏輯**——X1 修法只在 frontend display 層 extract inner command，wrapper 本身是 codex provider 的 sandbox 包法，不在本 change 觸碰。QGEN2 UTF-8 encoding 屬 codex-side bug、獨立追蹤。
- **不調 backend marker emit 行為**——`[CODEBUS_QUIZ_NO_VALIDATE]` 等 sentinel 由 `codebus-quiz` skill emit，本 change 只在 frontend filter / 翻譯顯示，不改 backend marker contract。
- **不抽 `<ActivityStreamThought>` / `<ShellCommandRow>` 共用 component**——保持 single-call site；共用化等 Phase 5 activity-stream-2-phase-cluster 結構性改造再一起做。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `quiz`: 新增 quiz history row 顯示規則（user-typed topic 為主標、hash ID 不作主視覺）。
- `app-workspace`: 新增 activity stream 顯示規則——Shell tool 對 `powershell.exe -Command` / `sh -c` / `bash -c` wrapper 抽出 inner command 再截 80 字；thought block 對 `[CODEBUS_*]` 內部 sentinel marker 走 i18n 翻譯或過濾，不 raw 顯示給 user。

## Impact

- Affected specs: `quiz`, `app-workspace`
- Affected code:
  - Modified:
    - codebus-app/src/components/workspace/QuizTab.tsx
    - codebus-app/src/components/workspace/ActivityStreamItem.tsx
    - codebus-app/src/i18n/messages.ts
  - New:
    - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx（若不存在則建；已存在則加 test case）
    - codebus-app/src/components/workspace/QuizTab.test.tsx 新增 row title test case（既有測試檔擴增）
  - Removed: (none)
