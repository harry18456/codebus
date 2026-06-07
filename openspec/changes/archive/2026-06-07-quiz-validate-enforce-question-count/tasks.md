## 1. core: 驗證器題數規則（quiz: Quiz Output Validation and Repair）

- [x] [P] 1.1 為 design 決策「驗證器接受期望題數並新增題數 finding」在 `codebus-core/src/verb/quiz_validate.rs` 測試模組新增失敗測試（RED）：`Some(n)` 且題數不符 → 回一個 `quiz-question-count` finding；`Some(n)` 且相符 → 無題數 finding；`None` → 無論題數皆不報。驗證：`cargo test -p codebus-core quiz_question_count` 先 RED。
- [x] 1.2 實作 design 決策「驗證器接受期望題數並新增題數 finding」，更新 spec requirement「Quiz Output Validation and Repair」：`validate_quiz_body` 簽章新增 `expected_count: Option<u8>`，`Some(n)` 且 `## Q<n>.` 區塊數 `!= n` 時追加 body 級 `quiz-question-count` error finding（message 載明 expected/actual），`None` 不檢查；同步更新 `quiz_validate.rs` 內 7 個既有測試呼叫點為三參數（傳 `None`）。驗證：1.1 轉綠 + `cargo test -p codebus-core quiz` 全綠。
- [x] 1.3 實作 design 決策「final-verify 傳入期望題數（誠實標記、不丟題）」，落實 spec requirement「Quiz Output Validation and Repair」：`run_quiz_generate` 的 final-verify 改呼叫 `validate_quiz_body(&quiz_md, &paths.wiki, Some(options.question_count))`，題數不符 → `validation: failed`、不丟題、verb 不失敗；content-repair 後若有 validate 呼叫一併對齊三參數。驗證：`cargo test -p codebus-core` 全綠、`cargo test -p codebus-cli` quiz 整合測試綠。

## 2. cli: quiz validate --count 旗標（cli: Quiz Validate Sub-Action Behavior + Subcommand Registration）

- [x] [P] 2.1 為 design 決策「CLI quiz validate 新增 --count 旗標」在 `codebus-cli` 測試新增失敗測試（RED）：`codebus quiz validate --count 5` 對 9 題 body → exit 1 並列出 question-count finding；省略 `--count` → 不檢查題數。驗證：`cargo test -p codebus-cli quiz` 先 RED。
- [x] 2.2 實作 design 決策「CLI quiz validate 新增 --count 旗標」，落實 spec requirement「Quiz Validate Sub-Action Behavior」與「Subcommand Registration」：`QuizValidateArgs` 新增 `count: Option<u8>`、原樣傳給 `validate_quiz_body` 的 `expected_count`；help/簽章顯示 `[--count <N>]`。驗證：2.1 轉綠 + `codebus quiz --help` 文檔含 validate 的 `--count`。

## 3. skill: claude 自驗帶 --count（skill-bundles: Quiz Skill Bundle Content）

- [x] 3.1 實作 design 決策「claude SKILL Mode B 自驗帶 --count N（codex 不變）」，落實 spec requirement「Quiz Skill Bundle Content」：在 `codebus-core/src/skill_bundle/mod.rs` 把 claude 路徑 quiz `generate:` 自驗呼叫改為 `codebus quiz validate --count <N>`（`<N>` 取自 prompt 的 `count=<N>`），仍只引用驗證器不重述規則；codex 路徑 Mode B 維持既有 no-validate marker。驗證：grep 確認 claude body 含 `codebus quiz validate --count`、codex body 不含 validate 呼叫。
- [x] [P] 3.2 確認 codex 衍生不漂移：`CODEX_BODY_TRANSLATIONS` drift-guard 與 SKILL materialization 測試維持綠（claude body 含 `codebus quiz validate --count`、codex body 不含 validate）。驗證：`cargo test -p codebus-core skill` / drift-guard 相關測試全綠。

## 4. 整合驗證與範圍確認

- [x] 4.1 全 Rust 回歸綠：`cargo test -p codebus-core`、`cargo test -p codebus-cli`、src-tauri crate `cargo test --lib` 全綠，且 `cargo clippy --workspace`（或受影響 crate `--lib`）無新增警告。驗證：上述指令逐一通過。
- [x] 4.2 落實 design 決策「不採 verb 端硬截斷（B over A 的權衡）」：確認 `run_quiz_generate` 無刪題/補題碼（題數縮減僅由 claude agent 自驗迴圈達成）、codex 路徑題數僅由 final-verify 標 `validation: failed` 不縮減（既有 codex 限制不退步）。驗證：grep `run_quiz_generate` 無 truncate/drop 題目邏輯、codex quiz 相關測試行為不變。
- [x] 4.3 手動 live 驗（claude 路徑）：spawn 一個設定 N 題的 quiz，確認產出檔 `## Q` 區塊數等於 N、frontmatter `validation: ok`。驗證：實機觀察 `codebus quiz --count 5` 產出剛好 5 題、`lint: 0 errors`、`validation: ok`（去重修法生效）。

## 5. core: 去複製（主修法）（quiz: Final Quiz Body Extraction Deduplicates Self-Validate Drafts）

- [x] [P] 5.1 為 design 決策「只取最終一份 quiz body 消除草稿複製（主修法）」在 `codebus-core/src/verb/quiz.rs` 測試模組新增失敗測試（RED）：草稿+最終兩份 → 只留最終；單份不變；草稿較短時取最終；無 `## Q1.` 時 fallback。驗證：`cargo test -p codebus-core final_body` 先 RED。
- [x] 5.2 實作 design 決策「只取最終一份 quiz body 消除草稿複製（主修法）」，落實 spec requirement「Final Quiz Body Extraction Deduplicates Self-Validate Drafts」：新增 `strip_to_final_quiz_body`（取最後一個 `## Q1.` 起；無則 fallback `strip_preamble_before_first_question`），`run_quiz_generate` 的 generate 與 repair 兩條 strip 路徑改用它。驗證：5.1 轉綠 + `cargo test -p codebus-core verb::quiz` 全綠 + live `codebus quiz --count 5` 得剛好 5 題。
