<!--
Traceability：每 task 標交付行為、驗證目標、對應 design 決策與 spec requirement（逐字）。TDD：(RED) 先寫失敗測試，(GREEN) 才實作到綠。audit：config 缺省/容錯安全、content-review 缺值不得當 ok、verify 失敗保守 flagged、auto_commit 不被擋。動到已 ship 的 quiz＝行為保持式 refactor，既有 quiz 全測試綠為硬門檻。檔案重疊（verb/quiz.rs、新 content_verify 模組、verb/goal.rs、config、skill_bundle、cli、app ipc、mock）且有資料依賴，故序列、無 [P]。
-->

## 1. 共用 `verb::content_verify` core 抽取 + quiz 行為不變重用（design D1；spec `verb-library` / Goal Content Verification and Repair）

- [x] 1.1 (RED) 新增 `codebus-core` `verb::content_verify` 單元測試：`parse_content_defects` 各態（`CONTENT_OK`→空、`<id> | type | sug`→defects、皆無→None，語意同 quiz 原實作）；`run_content_verify_loop` 注入 stub verify/repair 閉包驗 cap=3、到頂保留最佳、無缺陷即停、verify Err→保守 flagged。測試先 fail（模組未存在）。對應 design D1 與 spec `verb-library` / Goal Content Verification and Repair。
- [x] 1.2 (GREEN) 建 `verb::content_verify`（`ContentReview`、`ContentDefect`、`parse_content_defects`、`run_content_verify_loop`，由 quiz.rs 行為保持搬出）；重構 `run_quiz_generate` 改呼叫共用 core（adapter：verify=字串、repair=regenerate），`QuizContentReview` 對應 `ContentReview`。驗證：1.1 全綠；**既有 quiz 全自動測試（codebus-core + codebus-cli quiz_flow）綠且 `content_review` 落檔/events/cap/best-effort 行為逐項不變**（行為保持門檻）。對應 design D1。依賴 1.1。

## 2. `goal.content_verify` config loader（codebus-core config）（design D5；spec `cli` / Goal Content Verify CLI Behavior）

- [x] 2.1 (RED) 新增 `config::goal` 單元測試：缺鍵→`content_verify==false`；`true`/`false` 解析；型別錯/未知值 graceful 不 panic（仿 `config::quiz`）。測試先 fail（loader 未存在）。對應 design D5 與 spec `cli` / Goal Content Verify CLI Behavior。
- [x] 2.2 (GREEN) 新增 `codebus_core::config::goal`（top-level `goal.content_verify: bool`，缺省 false，forward-compat 容錯），mod 匯出。驗證：2.1 全綠；`cargo test -p codebus-core` 不回歸。對應 design D5。依賴 1.2。

## 3. codebus-goal SKILL `verify:` mode（codebus-core skill bundle）（design D5；spec `skill-bundles` / Codebus-Goal Verify Mode）

- [x] 3.1 (RED) skill-bundle materialization 測試：`codebus-goal/SKILL.md` 含 `verify:` mode，定義 3 缺陷（unfaithful/off-goal/taxonomy-misplaced）、逐頁輸出 `path | type | 建議` 或 `CONTENT_OK`、明示可讀 `raw/code/` 供 grounding 且不外洩 raw 內容、不重述 lint 規則；既有 goal ingest 內容不回歸。測試先 fail。對應 design D5 與 spec `skill-bundles` / Codebus-Goal Verify Mode。
- [x] 3.2 (GREEN) codebus-goal SKILL 加 `verify:` mode 段（3 契約 + 輸出格式 + raw/code grounding 說明 + 與 lint 分離），保留既有 ingest workflow。驗證：3.1 全綠；既有 skill-bundle 測試不回歸（含長度 cap 若有）。對應 design D5。依賴 1.2。

## 4. run_goal content-verify 整合（codebus-core/src/verb/goal.rs）（design D2/D3/D4；spec `verb-library` / Goal Content Verification and Repair）

- [x] 4.1 (RED) mock-spawn 測試（codebus-cli goal flow + `bins/mock_claude.rs` 新增 goal `verify-clean`/`verify-flag` behaviors）：`content_verify=false`→不跑 verify、`GoalReport` 無 content-review、auto_commit/exit 不變；無 wiki 變更→直接 ok 不 spawn；clean→content-review ok、auto_commit 照常；flag→≤3 輪 repair（Write 修頁）→殘餘 flagged+非致命 warning+不還原頁+exit 不變+**auto_commit 照常**；verify 失敗/不可解析→保守 flagged 非致命。測試先 fail。對應 design D2/D3/D4 與 spec `verb-library` / Goal Content Verification and Repair。
- [x] 4.2 (GREEN) `run_goal`：spawn 前記 vault HEAD；fix 迴圈後/auto_commit 前，`content_verify=true` 時 `git -C <vault> diff --name-only <pre> -- wiki/` 取變更頁（空→ok 短路）；獨立 read-only verify spawn（toolset 可讀 `raw/code/`）判 3 缺陷→經共用 core loop 跑 Write repair spawn 只修被點名頁→re-verify，cap=3、best-effort；`GoalReport` 加 content-review 狀態；`GoalOptions`/輸入加 `content_verify` 與 goal 文字（off-goal 用）；auto_commit 與 exit 不受影響。驗證：4.1 全綠；`cargo test -p codebus-core -p codebus-cli` 不回歸（含既有 quiz/goal）。對應 design D2/D3/D4。依賴 1.2、2.2。

## 5. CLI goal 串接（codebus-cli）（design D6；spec `cli` / Goal Content Verify CLI Behavior）

- [x] 5.1 (RED) `codebus-cli` 測試：`codebus goal "<g>"` 在 `goal.content_verify=true` 時把 config 解析結果與 goal 文字傳入 `run_goal`（off-goal 可判）、run 反映 content-review；缺省 false 時行為與決定性流程一致；`codebus --help` subcommand 不變。測試先 fail。對應 design D6 與 spec `cli` / Goal Content Verify CLI Behavior。
- [x] 5.2 (GREEN) `codebus-cli/src/commands/goal.rs`：用 core `default_config_path`+`config::goal` 解析 `goal.content_verify`（錯誤保守 false）+ 串 goal 文字進 `run_goal`；不新增 subcommand。驗證：5.1 全綠；`cargo test -p codebus-cli` 既有 goal/routing 不回歸。對應 design D6。依賴 4.2。

## 6. GUI goal 串接（codebus-app）（design D6；spec `app-workspace` / Goal Content Verify GUI Wiring）

- [x] 6.1 (RED) `codebus-app` tauri 測試：用 capturing runner 呼 goal-spawn IPC（with-runner），斷言注入 `run_goal` 的輸入在 `goal.content_verify=true` 時帶 `content_verify==true` 且 goal 文字傳入；config 載入錯/缺→`false`。測試先 fail（未串）。對應 design D6 與 spec `app-workspace` / Goal Content Verify GUI Wiring。
- [x] 6.2 (GREEN) `codebus-app/src-tauri/src/ipc/goals.rs`：同源 core loader 解析 `goal.content_verify`（錯誤保守 false）+ 串 goal 文字進 `run_goal`；不新增 IPC、不加 UI 徽章。驗證：6.1 全綠；`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml` 既有不回歸。對應 design D6。依賴 4.2。

## 7. 全域回歸 sweep（design「In scope」/「Out of scope」/「Implementation Contract」）

- [x] 7.1 全域回歸：`cargo test -p codebus-core -p codebus-cli`、`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml`、`cd codebus-app && npx vitest run && npm run typecheck` 彙總 0 failed；確認 **quiz 對外行為逐項不變**（行為保持門檻）、未回歸決定性 lint/fix、plan/generate、quiz；確認 `content_verify=false` CLI/GUI 皆等同今日 run_goal；確認 B1/開放評分/leaked-sensitive/新 subcommand/UI 徽章 未實作（Non-Goals 守住）。對應 design Goals / Non-Goals / In scope / Implementation Contract。依賴 1.2、2.2、3.2、4.2、5.2、6.2。

## Traceability

| Design topic | Tasks |
| --- | --- |
| D1：抽共用 `verb::content_verify` core（quiz 行為不變地重用） | 1.1, 1.2 |
| D2：goal「恰當」契約 = 固定 3 缺陷 | 4.1, 4.2 |
| D3：goal 整合位置與變更頁偵測 | 4.1, 4.2 |
| D4：殘餘 best-effort（鏡像 quiz D4） | 4.1, 4.2 |
| D5：config `goal.content_verify` + SKILL verify mode | 2.1, 2.2, 3.1, 3.2 |
| D6：CLI + GUI 對等（皆於本 change 實作） | 5.1, 5.2, 6.1, 6.2 |
| Implementation Contract | 1.2, 2.2, 3.2, 4.2, 5.2, 6.2, 7.1 |
| Goals | 7.1 |
| Non-Goals | 7.1 |
| In scope | 7.1 |
| Out of scope | 7.1 |

## Spec requirement coverage

| Spec requirement | Tasks |
| --- | --- |
| Goal Content Verification and Repair | 1.1, 1.2, 4.1, 4.2 |
| Goal Content Verify CLI Behavior | 2.1, 2.2, 5.1, 5.2 |
| Codebus-Goal Verify Mode | 3.1, 3.2 |
| Goal Content Verify GUI Wiring | 6.1, 6.2 |
