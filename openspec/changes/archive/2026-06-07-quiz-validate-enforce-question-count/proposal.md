## Why

使用者在 Quiz 把題數設為 5，產出的 quiz 卻有 9 題（再驗一次設 5 出 10）。**真正的 root cause 經 live events 還原確認：不是 LLM 超量，是輸出被複製。** claude 的 Mode B 自驗流程讓 agent 先寫一份草稿 quiz body、跑 `codebus quiz validate`、再寫一份「最終」body；`run_quiz_generate` 的 `run_spawn` 把 agent 的**每一段** assistant 文字（`StreamEvent::Thought`）串接成 `gen_text`，而 `strip_preamble_before_first_question` 只砍「第一個 `## Q` 之前」→ 草稿與最終**兩份都留下**。live 實證：agent 自驗的 heredoc 剛好 5 題、validator 回「0 issues」（5==5 正確），但持久化檔是 Q1–Q5 接 Q1–Q5 完全相同的兩份 = 10 題。使用者最初的「9」是草稿/最終其中一份不完整的同款複製。

附帶發現：deterministic 驗證器 `validate_quiz_body` 從不檢查題數（沒拿到 N），所以複製出的多餘題目連 `validation: failed` 都不會標、frontmatter 直接 `validation: ok` 放行。

## What Changes

- **（主修法，deterministic）`run_quiz_generate` 只取最終一份 quiz body**：新增 `strip_to_final_quiz_body`，從**最後一個 `## Q1.`**（最終版重新編號的起點）取，丟棄自驗前的草稿複本；無 `## Q1.` 時 fallback 現行 `strip_preamble_before_first_question`（單份輸出與 codex 路徑行為不變）。generate 與 repair 兩條 strip 路徑都改用它。此修法與 LLM 是否聽話無關、deterministic，直接消除複製。
- **（安全網）deterministic 驗證器新增題數 finding**：`validate_quiz_body` 接受 optional「期望題數」；提供且實際 `## Q<n>.` 區塊數 ≠ 期望時，產生 `error` 題數 finding（與既有 finding 同形狀）。未提供時行為不變。這是去複製後的第二道防線——任何殘留的題數異常（含未來其他來源）仍會被標記。
- **（安全網）`codebus quiz validate` 子動作新增 `--count <N>` 旗標 + claude SKILL Mode B 自驗帶 `--count <N>`**：讓 agent 自驗也能反映題數，subcommand 註冊行簽章同步；codex 路徑維持既有 no-validate marker。
- **（安全網）`run_quiz_generate` final-verify 傳入期望題數**：去複製後 final-verify 以 `Some(question_count)` 複查，題數仍異常則誠實標 `validation: failed`。維持 best-effort：不丟題、verb 不失敗。

## Non-Goals

- **不採用 verb 端硬截斷「真實超量」到 N**（選項 A）：去複製（`strip_to_final_quiz_body`）只移除 agent 重述的**重複**草稿，保留 agent 實際的最終那一份；它**不是**把一份「真的有 7 道不同題」的 body 裁成 5。若 agent 真的最終輸出了 ≠ N 道不同題（去複製後仍 ≠ N），本 change 不在 verb 端刪題/補題——改由安全網（驗證器題數 finding + agent 自驗 + final-verify `validation: failed` 標記）處理，best-effort、不保證恰好 N。硬截斷真實超量列為 design Risks 的未採選項。
- **不改 `quiz.content_verify`（模型內容驗證）路徑**：題數是結構問題，屬 deterministic 驗證器範疇，與內容 verify 互不重疊。
- **不改 `getDefaultLength()` / 題數來源**：題數 5 的解析本來就正確，問題在輸出端無強制。
- **不改固定的 token / 語言 / 模型設定。**

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `quiz`: **（主修法）`run_quiz_generate` 只取最終一份 quiz body（`strip_to_final_quiz_body`），消除自驗草稿與最終的複製**；deterministic 驗證器（`Quiz Output Validation and Repair`）新增第三類 finding（題數不符，安全網）；final-verify 傳入期望題數。
- `cli`: `Quiz Validate Sub-Action Behavior` 新增 `--count <N>` 旗標；subcommand 註冊行簽章同步。
- `skill-bundles`: claude 路徑 quiz SKILL `generate:` Mode B 自驗呼叫改帶 `--count <N>`；codex 路徑不變。

## Impact

- Affected specs: `quiz`, `cli`, `skill-bundles`
- Affected code:
  - Modified:
    - codebus-core/src/verb/quiz_validate.rs
    - codebus-core/src/verb/quiz.rs
    - codebus-cli/src/commands/quiz.rs
    - codebus-core/src/skill_bundle/mod.rs
  - New: (none)
  - Removed: (none)
