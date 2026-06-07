## Context

Quiz 產出題數由 `getDefaultLength()`（config `quiz.default_length`，fallback 5，clamp 3..10）決定，經 `compose_generate_input(topic, pages, count)` 寫成 prompt 的 `count=<N>` 段，SKILL Mode B 明確要求「Exactly `<N>` `## Q<i>.` sections」。但輸出端零強制：

- deterministic 驗證器 `validate_quiz_body(quiz_md, wiki_root)`（`verb/quiz_validate.rs`）只解析 `## Q<n>.` 區塊驗 schema（4 選項、Answer、Explanation）與 `[[slug]]` 引用存在性；**不接受、也不檢查期望題數**。
- `run_quiz_generate`（`verb/quiz.rs`）的 final-verify 呼叫 `validate_quiz_body(&quiz_md, &paths.wiki)` 後，依 findings 是否為空寫 `validation: ok|failed`；政策為 best-effort（持久化、不丟題、verb 不失敗）。
- claude 路徑 SKILL Mode B 自驗呼叫 `codebus quiz validate - <<heredoc`（無 `--count`），故自驗也抓不到題數。
- CLI `codebus quiz validate <file|->` 子動作目前簽章只有 `[--json]`。

結果：agent 多生（觀測案例設 5 出 9）時，自驗與 final-verify 都不報、`validation: ok` 放行，使用者拿到 9 題。

使用者選擇以「驗證器認 N」（選項 B）為強制機制，而非 verb 端硬截斷（選項 A）。

## Goals / Non-Goals

**Goals:**

- deterministic 驗證器在獲知期望題數時，能對「實際題數 ≠ 期望」產生一個 `error` finding，與既有 finding 同形狀、流經同一 self-repair 回饋路徑。
- claude 路徑的 agent 自驗→自修迴圈據此把題數修正到 N（在既有上限 3 次內）。
- `codebus quiz validate` 可由 caller 指定期望題數（`--count N`），未指定時行為不變。
- 持久化的 `validation:` 標記在題數不符時誠實為 `failed`。

**Non-Goals:**

- 不在 verb 端刪題／補題（不採選項 A 的硬截斷）。
- 不改 `quiz.content_verify` 模型內容驗證路徑。
- 不改題數來源（`getDefaultLength` / config）。
- 不為 codex 路徑新增可跑的自驗（codex 沙箱無法安全跑單一 `codebus quiz validate`，沿用既有 no-validate marker；codex 路徑題數僅由 final-verify 標記、不縮減）。

## Decisions

### 只取最終一份 quiz body 消除草稿複製（主修法）

真正的 root cause（live events 還原）：claude Mode B 自驗流程讓 agent 先寫草稿 quiz body、跑 `codebus quiz validate`、再寫最終 body；`run_quiz_generate` 的 `run_spawn` 把 agent 的每一段 `StreamEvent::Thought` 文字串接成 `gen_text`，`strip_preamble_before_first_question` 只砍第一個 `## Q` 之前，於是草稿與最終兩份都留下（live 實證：正確 5 題持久化成 10 題＝兩份完全相同的 Q1–Q5）。

修法：新增 `strip_to_final_quiz_body`，取**最後一個 `## Q1.`**（每份完整輸出都從 Q1 重新編號，最後一份即最終版）起的內容；無 `## Q1.` 時 fallback 既有 `strip_preamble_before_first_question`（單份輸出、codex 路徑行為不變）。`run_quiz_generate` 的 generate 與 repair 兩條 strip 路徑都改用它。此修法 deterministic、與 LLM 是否聽話無關。

替代方案：(a) 在 `run_spawn` 串接時就辨識草稿/最終只留最終——否決，stream 階段難可靠分辨且侵入性高；(b) 去重相同題組——否決，「取最後一份」更簡單且涵蓋「草稿不完整＋最終完整」的情形。此決策**取代**了原本對 root cause 的誤判（誤以為 LLM 超量、需題數強制）；題數驗證器（下列各項）保留為**安全網**而非主修法。

### 驗證器接受期望題數並新增題數 finding

`validate_quiz_body` 簽章新增結尾參數 `expected_count: Option<u8>`。當為 `Some(n)` 且解析到的 `## Q<n>.` 區塊數 `!= n` 時，附加一個 body 級 `error` finding（新 `rule_id` = `quiz-question-count`，識別碼用 body 級字串如 `quiz` 而非 `Q<n>`，message 載明「expected n, found m」）。`None` 時完全不檢查題數（向後相容既有 CLI 無 `--count` 與測試呼叫）。題數 finding 與 schema/wikilink finding 共用 `LintIssue` 形狀，因此自然流經既有的 agent self-repair 與 final-verify 通道。

替代方案：另開一個獨立的 count-check 函式——否決，因為 self-repair 與 final-verify 都已消費 `validate_quiz_body` 的 findings，複用同一函式才能讓題數修正走既有迴圈而非新增平行路徑。

### CLI quiz validate 新增 --count 旗標

`codebus quiz validate <file|-> [--count N] [--json]`：`QuizValidateArgs` 新增 `count: Option<u8>`，原樣傳給 `validate_quiz_body` 的 `expected_count`。省略時為 `None`（不檢查題數）。人類輸出與 `--json` 的 finding 列舉沿用既有格式（題數 finding 也以同樣形狀列出）。exit-code 契約不變（有 finding → 1）。subcommand 註冊描述（八子命令那條）的 validate 簽章一併由 `[--json]` 改為 `[--count N] [--json]`。

### claude SKILL Mode B 自驗帶 --count N（codex 不變）

claude 路徑 quiz SKILL 的 `generate:` Mode B 自驗步驟，把 `codebus quiz validate - <<heredoc` 改為 `codebus quiz validate --count <N> - <<heredoc`，其中 `<N>` 取自 agent 自己 prompt 中的 `count=<N>` 段。SKILL 仍只「引用驗證器、依 findings 行動」，不重述規則（維持 no-schema-double-delivery）。PreToolUse hook 的 Bash 白名單為 `Bash(codebus quiz validate *)`，`--count` 落在萬用後綴內，無需改 hook。codex 路徑 Mode B 維持既有 `[CODEBUS_QUIZ_NO_VALIDATE]` marker、不跑 validate（沿用 `skill-bundles` 既有 codex 限制）。

實作注意：SKILL body 為 `skill_bundle/mod.rs` 內 source 常數；codex body 由 `CODEX_BODY_TRANSLATIONS` 的 `str.replace` 衍生並有 drift-guard 測試守恆。本次只動 claude self-validate 文字，須確保 drift-guard 仍綠（必要時補一條 translation 條目）。

### final-verify 傳入期望題數（誠實標記、不丟題）

`run_quiz_generate` 的 final-verify 呼叫改為 `validate_quiz_body(&quiz_md, &paths.wiki, Some(options.question_count))`。題數不符會新增一個 finding → `validation: failed`。維持既有 best-effort 政策：仍持久化、不丟任何題、`run_quiz_generate` 不因此回傳 `VerbError`。content-verify 的 repair 路徑同樣已注入 count、不受影響。

### 不採 verb 端硬截斷（B over A 的權衡）

選項 A（verb 端把 >N 截斷到前 N 題）能「保證」恰好 N，但使用者選 B。B 的強制點在 claude agent 自驗迴圈：迴圈達上限 3 次後若仍未修正、或 codex 路徑（不自驗），題數可能仍 ≠ N，此時 final-verify 標 `validation: failed` 但不丟題。這是 B 相對 A 的已知取捨，刻意接受。

## Implementation Contract

**行為**：當 caller 對 `codebus quiz validate` 提供 `--count N`、或 `run_quiz_generate` 以 `Some(question_count)` 呼叫驗證器時，驗證器對「實際題數 ≠ N」回報一個 `error` finding。claude 路徑 generate 的 agent 自驗會因此 finding 在上限內把題數修正到 N；最終持久化 quiz 的 `## Q` 區塊數在 claude 路徑正常情況下等於設定題數，且 `validation: ok`。

**介面 / 資料形狀**：
- `validate_quiz_body(quiz_md: &str, wiki_root: &Path, expected_count: Option<u8>) -> Vec<LintIssue>`。`Some(n)` 且題數 `!= n` → 追加一個 `LintIssue { rule_id: "quiz-question-count", severity: Error, .. }`；`None` → 不檢查題數。
- CLI：`codebus quiz validate <file|-> [--count N] [--json]`；`QuizValidateArgs.count: Option<u8>`。
- SKILL：claude 路徑 Mode B 自驗指令含 `codebus quiz validate --count <N>`（`<N>` = prompt 的 count）。
- verb：final-verify 傳 `Some(options.question_count)`。

**失敗模式**：題數不符在 claude 路徑經 agent 自修；達上限未修正或 codex 路徑 → `validation: failed`、不丟題、verb 不失敗（best-effort，沿用既有政策）。`--count` 省略 → 不檢查題數（向後相容）。

**驗收**：
- 新增 Rust 單元測試：`validate_quiz_body` 在 `Some(n)` 且題數不符時回 `quiz-question-count` finding；`Some(n)` 且相符 → 無題數 finding；`None` → 無論題數皆不報。
- CLI 測試：`codebus quiz validate --count N`（透過 mock body / 既有 quiz CLI 測試）對題數不符 exit 1 並列出 finding；無 `--count` 行為不變。
- drift-guard 測試（`skill_bundle`）維持綠；claude 路徑 quiz SKILL body 含 `codebus quiz validate --count`，codex 路徑不含 validate 呼叫。
- 既有 `cargo test -p codebus-core` / `cargo test -p codebus-cli` 全綠（含既有 7 個 `validate_quiz_body` 測試呼叫點更新為三參數）。

**範圍邊界**：
- In scope：`validate_quiz_body` 簽章+題數規則、CLI `--count`、claude SKILL Mode B 自驗、verb final-verify 傳 count、相關 spec、測試。
- Out of scope：verb 端截斷/補題；codex 自驗；`quiz.content_verify`；題數來源/設定。

## Risks / Trade-offs

- [B 不保證恰好 N] → claude 自驗達上限或 codex 路徑時題數可能仍 ≠ N；以 `validation: failed` 誠實標記、不丟題；若日後要硬保證，再評估選項 A（verb 截斷）。記於 Non-Goals。
- [改 `validate_quiz_body` 公開簽章] → 7 個既有測試呼叫點 + verb + CLI 共 9 處需更新為三參數；`Option` 結尾參數使既有語意以 `None` 保持不變，最小破壞面。
- [SKILL body 改動觸發 codex drift-guard] → 只動 claude self-validate 文字；若 translation 表受影響則補一條對應條目，靠既有 drift-guard 測試把關。
- [codex 路徑題數仍不縮減] → 為既有 codex 沙箱限制的延伸（Mode B 本就不自驗），非本 change 退步；final-verify 標記提供 observability。

## Open Questions

(none)
