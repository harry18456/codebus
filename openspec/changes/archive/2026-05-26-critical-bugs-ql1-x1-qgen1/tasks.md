<!--
TDD：先 test 後 impl。
parallel_tasks: true — 加 [P] 標記僅在「目標檔案不同 + 無 pending 依賴」時使用。
QL1 動 QuizTab*；X1 + QGEN1 都動 ActivityStreamItem*（彼此互卡），故只有 QL1 vs X1/QGEN1 之間可 [P]。
-->

## 1. QL1 · Quiz History Row Title Displays User-Authored Topic

- [x] 1.1 [P] 為 Quiz History Row Title Displays User-Authored Topic requirement 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 加 vitest test case，covering：(a) Goal flow（frontmatter 帶 `topic: 專案目的`）row 主標應為 `專案目的`；(b) Page flow（frontmatter 帶 `target_page` 無 `topic`）row 主標應為 page 名；(c) legacy attempt（無 `topic` 也無 `target_page`）row 主標 fallback 為 slug 而非空字串。驗證：`pnpm --filter codebus-app test QuizTab` 三條新 case 均 fail（red baseline）。
- [x] 1.2 實作 QuizTab row title resolver：在 row render 處（或 quiz history 資料轉換層）改成讀取 `topic` → `target_page` → slug fallback 三段優先序；hash slug 不再作為主標來源。驗證：1.1 三條 test 全部 green，且 `pnpm --filter codebus-app test QuizTab` 整檔通過。

## 2. X1 · Activity Stream Shell Command Wrapper Extraction

- [x] 2.1 [P] 為 Activity Stream Shell Command Wrapper Extraction requirement 在 `codebus-app/src/components/workspace/ActivityStreamItem.test.tsx`（不存在則新建檔，遵循專案既有 vitest 慣例）加測試，覆蓋 spec scenario 表的全部 6 種 row（PowerShell 雙引號路徑、PowerShell 無引號、`/bin/sh -c` 單引號、`bash -c` 單引號、`sh -c` 雙引號、無 wrapper passthrough）+ 一條「200 字 inner command 套 PS wrapper → 顯示 80 字 + ellipsis、不含 wrapper 字串」。驗證：`pnpm --filter codebus-app test ActivityStreamItem` 新 case 均 red。
- [x] 2.2 實作 `extractInnerCommand(raw: string): string` helper（與 `ActivityStreamItem.tsx` 同檔或同目錄獨立檔皆可），detect 三類 wrapper 後抽 inner command 並去除外圍 single/double quote；無 wrapper 則 passthrough。在 `summarizeToolInput` 對 Shell tool 的 `obj.command` path 改成 `truncate(extractInnerCommand(obj.command), 80)`，確保 80-char cap 套用在抽完的 inner command 上。驗證：2.1 全部 test green、Shell row scenario 表 6 行全通過。

## 3. QGEN1 · Activity Stream Internal Sentinel Marker Filter

- [x] 3.1 為 Activity Stream Internal Sentinel Marker Filter requirement 在 `codebus-app/src/components/workspace/ActivityStreamItem.test.tsx` 續加測試，覆蓋四個 scenario：(a) zh-tw locale + `[CODEBUS_QUIZ_NO_VALIDATE]` 開頭 → 渲染為 `codex 沙箱無法跑 quiz 結構驗證，跳過此步`、不含原 marker；(b) `[CODEBUS_UNKNOWN_MARKER]` 開頭 → 渲染為空、不含原 marker；(c) 純文字 thought 不被觸發；(d) marker 在中間（非開頭）→ 不觸發 filter、原文 verbatim。驗證：四條 case 在無 impl 時 red。
- [x] 3.2 在 `codebus-app/src/i18n/messages.ts` 補 `activity.marker.codebusQuizNoValidate` key（zh-tw：`codex 沙箱無法跑 quiz 結構驗證，跳過此步`；en 對應翻譯）。在 `ActivityStreamItem.tsx` thought block render 加入 marker registry detection：text 開頭符合 `^\s*\[CODEBUS_[A-Z0-9_]+\]` 時，查 i18n registry → 有對應 key 顯示翻譯；無則整段不 render；其他 thought 走原渲染路徑。驗證：3.1 四條 test 全 green；mid-sentence marker case 確認未誤殺。

## 4. 整體驗證

- [x] 4.1 跑 `pnpm --filter codebus-app test` 完整套件，確認三個 requirement 對應的 test 全 green、無回歸（既有 QuizTab.test.tsx / ActivityStreamItem 相關既有 case 全保留通過）。
- [x] 4.2 跑 `pnpm --filter codebus-app typecheck`（或專案等效 `tsc --noEmit` 指令）確認 type-clean、`extractInnerCommand` helper 與 marker registry signature 對齊既有 i18n / render type。
- [x] 4.3 手動 smoke：`pnpm --filter codebus-app dev` 啟動，在 zh-tw locale 下開一個既有 quiz attempt（驗 QL1 row 顯示 user topic）、跑一個 codex provider goal 觀察 Activity Stream（驗 X1 PowerShell wrapper 已抽 inner / QGEN1 `[CODEBUS_QUIZ_NO_VALIDATE]` 已翻譯）。結果記錄在 commit / PR description；無法重現 codex 環境時記為「無法手測 codex 端、單元測試覆蓋已足」。
