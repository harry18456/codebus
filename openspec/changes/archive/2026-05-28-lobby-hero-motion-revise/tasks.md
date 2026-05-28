<!--
Behavior + verification per task. File paths are locator context.
parallel_tasks: true → [P] marker on tasks targeting different files with no shared dep.
-->

## 1. Pre-apply ground-truth recheck

- [x] 1.1 重跑 Pre-apply 校準（grep 證實後填）：用 Grep 重驗 design.md Context 表格 5 項仍然成立——`codebus-bus-idle-y` / `codebus-bus-idle-x` 於 `codebus-app/src/` 0 hit、`Idling in place` 於 live source 3 hit（AUDIT × 2 + `codebus-app/src/styles/globals.css` × 1）、`codebus-bus-roll-mirrored` 於 globals.css ≥1 hit、`codebus-bus-roll`（無 mirrored 後綴）於 globals.css + `codebus-app/src/components/LoadingOverlay.tsx` ≥1 hit each、`EmptyState.test.tsx` 仍只斷言 `.codebus-bus-idle` class。驗證：Grep 命令 echo 出來 + 任一項不符立刻 stop 並對齊 design.md。

## 2. Spec rename + content revise

- [x] 2.1 在 `openspec/specs/app-shell/spec.md` 套用 Lobby Empty State Hero Motion requirement（Decision 2 · Spec Requirement 標題改 Lobby Empty State Hero Motion）：把 line ~1328 標題從 `### Requirement: Lobby Empty State Idle Motion` 改為 `### Requirement: Lobby Empty State Hero Motion`、requirement 正文與 3 個 scenario 換成 specs/app-shell/spec.md delta 的 MODIFIED 版本（mirrored cyclic + ±50px + ±2° + -3px Y + 2.5s dwell-return）、`@trace` 區段補本 change name `lobby-hero-motion-revise` 進 `source` 與 `updated: 2026-05-28`。驗證：Grep `Requirement: Lobby Empty State Hero Motion` 於 `openspec/specs/app-shell/spec.md` = 1 hit、`Requirement: Lobby Empty State Idle Motion` 於同檔 = 0 hit、`spectra validate lobby-hero-motion-revise` 綠。

## 3. AUDIT brand motion lock revise

- [x] 3.1 [P] 在 `codebus-app/design-handoff/AUDIT.md` line ~540-562 套用 Decision 1 · Brand Motion Vocabulary 架構選定 Moving forward family + loading cyclic 變體：把「2 個合法 mood」段（含表格 + Hard Nos 區 + 必加約束區）改寫為「Moving forward family + loading / cyclic 兩 variant」結構、保留 Hard Nos 原 3 條、保留必加約束 prefers-reduced-motion fallback 句、移除 ODI-1 句中「Idling in place」用詞、改引用新 spec requirement 名 `Lobby Empty State Hero Motion`。驗證：Grep `Idling in place` 於 `codebus-app/design-handoff/AUDIT.md` = 0 hit、Grep `Moving forward` + `cyclic` + `loading` 於同檔 ≥1 hit each、Grep `Wordmark 🚌 動畫` + `Goal Running 加 bus 動畫` + `Quiz generation 加 bus 動畫` Hard Nos 三條完整保留。

- [x] 3.2 [P] 在 `codebus-app/design-handoff/AUDIT.md` LO-3 段（line ~625-630）動畫詞彙表更新（Decision 1 配套）：把「行進中 = LoadingOverlay、待機怠速 = 04b hero」二分法改寫為明列 `codebus-bus-roll`（loading variant）= LoadingOverlay vs `codebus-bus-roll-mirrored`（cyclic variant）= 04b hero、inline spinner 仍 TBD 不動。驗證：Grep `codebus-bus-roll-mirrored` 於 `codebus-app/design-handoff/AUDIT.md` LO-3 段 ≥1 hit、Grep `待機怠速` 於 LO-3 段 = 0 hit。

- [x] 3.3 [P] 在 `codebus-app/design-handoff/AUDIT.md` ODI-1 archived entry（line ~1827-1837）末尾 append Revision Footnote（Decision 4 · ODI-1 Archived Entry 不重寫本體只 append Revision Footnote）：原 archived 內容 100% 不動，於 entry 最後一行下方加 `**Revision 2026-05-28 · lobby-hero-motion-revise**:` 段、說明原 spec「2px bob + 1px shake、無 rotation」實機接近視覺臨界、user-as-design 階段廢除「Idling in place」mood、改採 mirrored cyclic（±50px / 2.5s / scaleX(-1) / dwell-return）、引用 `openspec/changes/archive/2026-05-28-lobby-hero-motion-revise/`（archive 後路徑、apply 階段先寫此路徑）。驗證：Grep `Revision 2026-05-28 · lobby-hero-motion-revise` 於 ODI-1 entry = 1 hit、ODI-1 原 archived 標題 + 風味 + 約束 + 不在範圍 四節文字逐字未動（手動 diff 確認）。

## 4. globals.css comment + dead keyframe cleanup

- [x] 4.1 在 `codebus-app/src/styles/globals.css` line ~111-138 套用 Decision 5 · globals.css comment 改成正式 spec 引用而非 TEMP experiment + Decision 3 · 廢棄 keyframe codebus-bus-idle-y 與 codebus-bus-idle-x 直接刪除不留 fallback：comment 從「TEMP experiment 2026-05-28」改寫為引用新 spec requirement 名 `Lobby Empty State Hero Motion` 與檔案路徑 `openspec/specs/app-shell/spec.md`、刪除 `@keyframes codebus-bus-idle-y` 與 `@keyframes codebus-bus-idle-x` 兩支 keyframe block、`@keyframes codebus-bus-roll-mirrored` 與 `.codebus-bus-idle` selector 與 `@media (prefers-reduced-motion: reduce)` fallback 三段保留不動。驗證：Grep `codebus-bus-idle-y` 與 `codebus-bus-idle-x` 於 `codebus-app/src/` = 0 hit、Grep `TEMP experiment` 於 `codebus-app/src/styles/globals.css` = 0 hit、Grep `Idling in place` 於 `codebus-app/src/styles/globals.css` = 0 hit、Grep `codebus-bus-roll-mirrored` 於同檔 ≥1 hit、Grep `codebus-bus-roll`（無 -mirrored 後綴、line-anchored）於同檔 ≥1 hit（LoadingOverlay keyframe 仍在）。

## 5. Test + build verification（Acceptance criteria）

- [x] 5.1 跑 `pnpm tsc` 於 `codebus-app/` 確認 type check 綠（Acceptance criteria 1）。驗證：command 退出 code = 0、stdout/stderr 無 error。

- [x] 5.2 跑 `pnpm test` 於 `codebus-app/` 確認 `EmptyState.test.tsx` 「applies the idle-motion class on the empty-state hero only」test case 仍綠（Acceptance criteria 2、Failure modes 中「test 仍綠 不代表 CSS 對」風險已在 design.md 標註、改靠 CDP smoke 補）。驗證：command 退出 code = 0、test output 含該 test name 且 status = passed。

- [x] 5.3 跨檔最後一輪同名詞 disambiguation sweep（Acceptance criteria 3、對應 design.md 同名詞 disambiguation 表所有「revise 後」欄位）：Grep `Idling in place` 於 `openspec/specs/` + `codebus-app/design-handoff/AUDIT.md` + `codebus-app/src/` 應 0 hit；若 archive doc 有保留歷史快照（如 `design-reply.html`、`FEEDBACK.md`）不在此 sweep 範圍。驗證：三條 Grep 命令各回 0 result。

## 6. CDP smoke（Acceptance criteria 11、Behavior 段 user-observable 驗證）

- [x] 6.1 跑 dev server `pnpm tauri dev` 並透過 WebView2 `--remote-debugging-port=9222` 拉起、用 `codebus-app/scripts/cdp.mjs` connectOverCDP 連線；按 [[project_cdp_smoke_webview2_pitfalls]] 5 雷處理 React batching 兩段 eval 與 transition focus 等待。驗證：cdp script 連線成功、能讀 DOM（如 querySelector `.codebus-bus-idle` 取得 element）。

- [x] 6.2 CDP smoke 路徑 a：開 Lobby 04b empty state（無 vault）→ wait for `.codebus-bus-idle` 元素 → 截圖三張間隔 ~800ms 至 `codebus-app/scripts/.lobby-hero-motion-revise-smoke/empty-hero-{1,2,3}.png` → 肉眼確認 🚌 朝左（scaleX(-1) 鏡像）+ 動的範圍明顯（X 跨 100px 總 forward → dwell → return 2.5s）+ rotation 可見 + dwell-return 表達 ambient 巡迴非真前進（對應 design.md Behavior（user-observable）第 1 條 + Interface / data shape 中 `codebus-bus-roll-mirrored` keyframe contract）。驗證：三張截圖 file size > 0、肉眼 review 全 pass。

- [x] 6.3 CDP smoke 路徑 b：透過 CSSOM iterate 全 stylesheet 找 `@media (prefers-reduced-motion: reduce)` rule 內 `.codebus-bus-idle { animation: none }`（CDP `Emulation.setEmulatedMedia` 不吃 WebView2 / per [[project_cdp_smoke_webview2_pitfalls]] 第 1 雷退一步驗 CSSOM）。驗證：CDP eval 回 `{ found: true, rule: "animation: none" }`（對應 design.md Behavior（user-observable）第 2 條）。

- [x] 6.4 CDP smoke 路徑 c：在 page eval 呼叫 `window.__codebus_test_add_vault__` trigger LoadingOverlay → 截圖至 `codebus-app/scripts/.lobby-hero-motion-revise-smoke/loading-overlay-{1,2}.png` → 肉眼確認公車仍朝右（無 scaleX(-1) 鏡像、`codebus-bus-roll` 1.8s 單向未被本 change 污染、Scope boundaries Out of scope 第 1 條 + Behavior 段第 3 條 + Failure modes「未察覺 consumer」風險防線）。驗證：兩張截圖 file size > 0、肉眼 review 確認公車方向與 Lobby empty hero 相反。

## 7. Final validation

- [x] 7.1 跑 `spectra validate lobby-hero-motion-revise` 確認 spec delta 與 tasks 結構通過。驗證：command 退出 code = 0、output 無 Critical / Warning。

- [x] 7.2 停在 archive 前對 user report：列出本 change 所有 modified file 與 grep 證據（Acceptance criteria 1-10 對表 + smoke 截圖路徑）、等 user 決定是否 archive + commit（per [[feedback_archive_commit_immediately_after_apply]] 不自作主張 archive）。驗證：report 訊息在 chat 內、未執行 `spectra archive` 或 `git commit`。
