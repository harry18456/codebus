<!--
Traceability：每 task 標交付行為、驗證目標、對應 design 決策與 spec requirement（逐字）。TDD：(RED) 先寫失敗測試，(GREEN) 才實作到綠。audit：config 缺省/容錯安全、`content_review` 缺值不得當 ok、verify 失敗非致命保守標 flagged。檔案重疊（verb/quiz.rs、config、skill_bundle、cli、mock_claude）且有資料依賴（CLI 串 topic 依賴 run_quiz_generate 介面、整合依賴 config+SKILL），故序列、無 [P]。
-->

## 1. `quiz.content_verify` config 鍵（codebus-core config）（design D5；spec `quiz` / Quiz Content Verification and Repair）

- [x] 1.1 (RED) 在 codebus-core quiz config 單元測試新增：缺鍵 → `content_verify == false`；`true`/`false` 正確解析；型別錯誤/未知值走既有 graceful 容錯不 panic。測試先 fail（欄位未存在）。對應 design D5 與 spec `quiz` / Quiz Content Verification and Repair。
- [x] 1.2 (GREEN) quiz config struct/loader 加 `content_verify: bool`（serde 缺省 false，沿用既有 tolerance）。驗證：1.1 全綠；`cargo test -p codebus-core` 不回歸。對應 design D5。依賴 1.1。

## 2. codebus-quiz SKILL `verify:` mode（codebus-core skill bundle）（design D2/D7；spec `skill-bundles` / Quiz Skill Bundle Content）

- [x] 2.1 (RED) skill-bundle materialization 測試：`codebus-quiz/SKILL.md` 含第三 mode `verify:`，定義 5 種缺陷（answer-wrong / out-of-scope / not-exactly-one-correct / degenerate-distractor / off-topic）、要求逐題輸出「題號 + 缺陷類型 + 修正建議」、off-topic 僅在有 topic 時判、且不重述決定性 validator 規則；既有 plan/generate/自驗 loop 行為不回歸。測試先 fail。對應 design D2/D7 與 spec `skill-bundles` / Quiz Skill Bundle Content。
- [x] 2.2 (GREEN) `QUIZ_SKILL_CONTENT` 加 `verify:` mode 段（5 缺陷契約 + 輸出格式 + 與決定性 validate 分離），保留既有 plan/generate/violation/語言/自驗規則。驗證：2.1 全綠；既有 skill-bundle 測試不回歸。對應 design D2/D7。依賴 1.2。

## 3. run_quiz_generate verify+repair 整合（codebus-core/src/verb/quiz.rs）（design D1/D3/D4/D6；spec `quiz` / Quiz Content Verification and Repair）

- [x] 3.1 (RED) mock-spawn 測試（codebus-cli `quiz_flow` + `bins/mock_claude.rs` 新增 `quiz-verify-clean` / `quiz-verify-flag` behaviors）：`content_verify=false` → 不跑 verify、落檔無 `content_review`；`=true` 且 verify 回 0 缺陷 → 落檔 `content_review: ok`、無 warning；verify 點名 Q3 → 經 Stage-1 trust-agent 路徑 ≤cap 輪 repair → 最終 `content_review: ok`（清）或 `flagged` 列題號（殘留）+ 非致命 warning + 題數不減 + exit 0；Page flow（無 topic）→ off-topic 不判、其餘 4 項仍判；verify spawn 失敗/不可解析 → warning + `content_review: flagged`（不當 ok）+ 不失敗；一次 run 仍一筆 RunLog。測試先 fail。對應 design D1/D3/D4/D6 與 spec `quiz` / Quiz Content Verification and Repair。
- [x] 3.2 (GREEN) `run_quiz_generate` 加 originating-topic `Option<String>` 輸入；`content_verify=true` 時於決定性 final-verify 後 reuse `run_spawn` 跑獨立 read-only verify spawn（沿用 generate model/effort、無 Bash），缺陷以既有 finding 形狀經 `fan_out` emit 並走 Stage-1 trust-agent repair 路徑（cap 同 Stage-1、到頂出最佳）；`QuizReport` 加 content-review 狀態；`persist_quiz` caller frontmatter 寫 `content_review: ok|flagged`（flagged 列題號、缺值不得當 ok）；殘餘/verify 失敗皆 best-effort 非致命。驗證：3.1 全綠；`cargo test -p codebus-core -p codebus-cli` 不回歸。對應 design D1/D3/D4/D6 與 spec `quiz` / Quiz Content Verification and Repair。依賴 1.2、2.2。

## 4. CLI topic 串接（codebus-cli）（design D6；spec `cli` / Quiz Content Verify CLI Behavior）

- [x] 4.1 (RED) `codebus-cli` 測試：`codebus quiz "<topic>"` 在 `content_verify=true` 時把 `<topic>` 傳進 `run_quiz_generate`（off-topic 可判）、落檔含 `content_review`；`content_verify` 缺省 false 時行為與決定性流程一致、無 `content_review`；`codebus --help` subcommand 列表不變（無新增）。測試先 fail（topic 未串、無 content_review）。對應 design D6 與 spec `cli` / Quiz Content Verify CLI Behavior。
- [x] 4.2 (GREEN) `codebus-cli` quiz command 把 Goal-flow `args.topic` 經新介面傳入 `run_quiz_generate`；不新增 subcommand/flag（config-only）。驗證：4.1 全綠；`cargo test -p codebus-cli` 既有 quiz/routing 不回歸。對應 design D6 與 spec `cli` / Quiz Content Verify CLI Behavior。依賴 3.2。

## 5. GUI 行為對等：`spawn_quiz_generate` 接 config + 串 topic（codebus-app）（design D8；spec `app-workspace` / Quiz Content Verify GUI Wiring）

- [x] 5.1 (RED) `codebus-app` tauri 測試：用 capturing runner 呼 `spawn_quiz_generate_with_runner`，斷言注入 `run_quiz_generate` 的 `QuizGenerateOptions` 在 `content_verify=true` + `AiPlanned{topic}` trigger 時帶 `content_verify==true` 且 `topic==Some(topic)`；`WikiPreview` trigger 時 `topic==None`；`content_verify=false` 時 `content_verify==false`。測試先 fail（literal 寫死 `false/None`）。對應 design D8 與 spec `app-workspace` / Quiz Content Verify GUI Wiring。
- [x] 5.2 (GREEN) `spawn_quiz_generate`（`#[tauri::command]`）以 core `default_config_path`+`load_quiz_config` 解析 `quiz.content_verify`（錯誤保守 false）傳入 `spawn_quiz_generate_with_runner`；後者由 `trigger` 導出 topic（`AiPlanned`→`Some`、`WikiPreview`→`None`）建 `QuizGenerateOptions{content_verify,topic,..}`（取代寫死 false/None）；不新增 IPC、不加 UI 徽章。驗證：5.1 全綠；`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml` 既有不回歸。對應 design D8 與 spec `app-workspace` / Quiz Content Verify GUI Wiring。依賴 3.2。

## 6. 全域回歸 sweep（design「In scope」/「Out of scope」/「Implementation Contract」）

- [x] 6.1 全域回歸：`cargo test -p codebus-core -p codebus-cli`、`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml`、`cd codebus-app && npx vitest run && npm run typecheck` 彙總 0 failed；確認未回歸 Stage-1 決定性 validate/自修、plan/generate、quiz 持久化/sidecar、決定性 `codebus quiz validate` 與 hook；確認 `content_verify=false` 時 CLI 與 GUI 行為皆與 Stage-1 一致；確認 B1/goal 通用化/品質評分/新 subcommand/UI 徽章 確實未實作（Non-Goals 守住）。對應 design Goals / Non-Goals / In scope / Implementation Contract。依賴 1.2、2.2、3.2、4.2、5.2。

## Traceability

| Design topic | Tasks |
| --- | --- |
| D1：獨立 verify spawn（B2），跑在 run_quiz_generate 內、config 閘控 | 3.1, 3.2 |
| D2：「內容 ok」契約 = 固定 5 項逐題缺陷 | 2.1, 2.2, 3.1 |
| D3：有界 caller 編排 verify→repair 迴圈（新增機制；修正 archived D6 的誤述） | 3.1, 3.2 |
| D4：殘餘 best-effort（鏡像 Stage-1 D4） | 3.1, 3.2 |
| D5：config 閘 `quiz.content_verify`，預設 false，config-only | 1.1, 1.2 |
| D6：topic 串接 | 3.2, 4.1, 4.2 |
| D7：SKILL 新增 `verify:` mode；決定性 validator 不動 | 2.1, 2.2 |
| D8：GUI 行為對等——`spawn_quiz_generate` IPC 接 config + 串 topic | 5.1, 5.2 |
| Implementation Contract | 1.2, 2.2, 3.2, 4.2, 5.2, 6.1 |
| Goals | 6.1 |
| Non-Goals | 6.1 |
| In scope | 6.1 |
| Out of scope | 6.1 |

## Spec requirement coverage

| Spec requirement | Tasks |
| --- | --- |
| Quiz Content Verification and Repair | 1.1, 1.2, 3.1, 3.2 |
| Quiz Content Verify CLI Behavior | 4.1, 4.2 |
| Quiz Skill Bundle Content | 2.1, 2.2 |
| Quiz Content Verify GUI Wiring | 5.1, 5.2 |
