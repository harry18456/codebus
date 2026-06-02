# Tasks

## 1. §0 Language Policy 擴涵 quiz

- [x] 1.1 [P] 實作 spec requirement **NEUTRAL_RULES Language Policy**：在 `codebus-core/src/schema/neutral.md` 的 `## 0. Language Policy` 段新增 quiz 子句：題幹/選項/解釋有 topic 時跟隨 topic 語言、無 topic 時 fallback 被考頁面 auto-detect、quiz structural token（`[CODEBUS_QUIZ_*]`、`## Answer:`、`## Explanation:`）恆英文；不得宣稱「跟隨 dominant 頁面語言」。**驗證**：`cargo test -p codebus-core` 中 `tests/schema_neutrality.rs` 仍綠（§0 存在且含 `agent output` / `structural tokens` substring、§0 在 §1 之前）。

## 2. quiz SKILL body 對齊（mod.rs）

- [x] 2.1 [P] 實作 spec requirement **Quiz Skill Bundle Content**：改寫 `codebus-core/src/skill_bundle/mod.rs` 中 `QUIZ_SKILL_CONTENT` 的 `## Language Override` 段：刪掉「follow the language of the quizzed wiki pages（auto-detect / dominant）」，改為「`generate:` 提供 `topic=<...>` 時跟隨 topic 語言；未提供時 fallback 被考頁面 auto-detect」，並明示對齊 §0、structural token 恆英文。**驗證**：新增/更新一條斷言（比照既有 `mod.rs` 內 quiz body 測試風格）確認 body 含 topic-follows 文字且不含舊「dominant language」措辭。
- [x] 2.2 [P] 在同 const 的 Mode B 契約（`### Mode B — generate: pages=[...] count=<N>`）說明新增可選 `topic=<...>` 欄位：Goal flow 帶、Page flow 不帶，作為語言訊號。**驗證**：`mod.rs` quiz body 測試斷言契約段含 `topic=` 欄位描述。
- [x] 2.3 跑 drift guard：若 2.1/2.2 新增的字串需 codex 對應，於 `CODEX_BODY_TRANSLATIONS` 補對應項；新文字若為 provider-neutral 純政策敘述則免補。**驗證**：`cargo test -p codebus-core` 中 `every_codex_translation_from_appears_in_a_claude_body`、`drift_guard_detects_unmatched_from`、`drift_guard_detects_leaked_claude_token` 全綠。（依賴 2.1、2.2）

## 3. generate / repair spawn input 帶 topic（quiz.rs）

- [x] 3.1 [P] 實作 spec requirement **Quiz Generate Spawn Carries Topic Language Signal**：在 `codebus-core/src/verb/quiz.rs` 新增純函式 `compose_generate_input(topic: Option<&str>, pages, count) -> String`，比照既有 `compose_verify_input` 風格：`topic` 為 `Some` 時輸出 `topic=<topic>` 段 + `pages=[...] count=<N>`、`None` 時僅 `pages=[...] count=<N>`。**驗證**：新增單元測試覆蓋 Some（含 `topic=`）、None（不含 `topic=`、與舊 shape 等同）兩列舉。
- [x] 3.2 `run_quiz_generate` 的 generate spawn 改用 `compose_generate_input(options.topic.as_deref(), &options.pages, options.question_count)` 組 `input`，取代原本只組 `pages=[...] count=N` 的 `format!`。**驗證**：單元測試斷言 `topic: Some(...)` 時送進 spawn 的 `input` 含 `topic=<...>`、`topic: None` 時不含。（依賴 3.1）
- [x] 3.3 `run_quiz_generate` 內 content-verify 的 repair（regenerate）closure 同樣以 `compose_generate_input` 組前綴（`topic` 為 `Some` 時帶 `topic=<...>`），其餘 defect-revision 指示與 PREVIOUS QUIZ / CONTENT DEFECTS 區塊不變。**驗證**：單元測試或既有 content-verify 測試斷言 repair `input` 在 topic=Some 時含 `topic=<...>` 段。（依賴 3.1）

## 4. 全量驗證與回歸

- [x] 4.1 跑 `cargo test -p codebus-core` 與 `cargo test -p codebus-cli` 全綠，確認 quiz / skill_bundle / schema_neutrality 無回歸。（依賴 1.1、2.x、3.x）
- [x] 4.2 跑 `cargo clippy --workspace`，確認無新增 warning（baseline 既有 warning 不算）。（依賴 1.1、2.x、3.x）
