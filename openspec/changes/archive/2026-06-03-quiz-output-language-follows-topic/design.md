## Context

同仁回報：被考的 wiki 頁面是中文，但 quiz 卻考出英文。三條根因（已 grep 複驗）：

1. **§0 Language Policy**（`codebus-core/src/schema/neutral.md` 的 `## 0. Language Policy` 段，materialize 成 vault `CLAUDE.md` / `AGENTS.md`）規定 agent 輸出語言跟隨 prompt context（user 的 goal/query/chat 文字），且明確「不跟隨 wiki 頁面語言」——但只列 goal/query/chat，**沒涵蓋 quiz**。
2. quiz SKILL 的 `## Language Override`（`codebus-core/src/skill_bundle/mod.rs` 的 `QUIZ_SKILL_CONTENT` const）反著做：題幹/選項/解釋「follow the language of the quizzed wiki pages（auto-detect；混雜時取 dominant）」。技術頁面夾雜大量英文（slug、frontmatter、code identifier、`[[wikilink]]`），dominant 常判成英文。
3. quiz generate spawn 的 `input`（`codebus-core/src/verb/quiz.rs` 的 `run_quiz_generate`）只組 `pages=[...] count=N`，**沒帶 topic**，generate agent 連語言訊號都沒有。

關鍵前提：`QuizGenerateOptions.topic: Option<String>` 已 plumb 到底，兩條 caller（CLI / GUI IPC）都已填妥，目前僅用於 content-verify 的 off-topic 檢查。本 change 只「複用」此值，不動傳遞層。

## Goals / Non-Goals

**Goals:**

- quiz 輸出語言跟隨 quiz topic（Goal flow）；無 topic（Page flow / wiki-preview「Quiz me on this」）fallback 到被考頁面語言 auto-detect。
- §0 與 quiz SKILL 語言政策對齊（集中政策到 §0，SKILL Override 對齊之）。
- structural token（`[CODEBUS_QUIZ_*]`、`## Answer:`、`## Explanation:`）恆英文不變。

**Non-Goals:**

- 不改成固定宣告語言（不 hard-code 中/英）。
- 不新增全域語言設定旋鈕（另一個更大的決定，明確 out of scope）。
- 不動 IPC / CLI 的 `topic` 傳遞層。
- 不改 marker / structural token 的英文恆定規則。
- 不引入語言偵測函式庫——語言判斷仍由 agent 在 spawn 內完成。

## Decisions

**D1：採「動態語言模型」而非固定宣告語言。** topic 是 user 的自然語言意圖，最貼近 prompt context 哲學；fixed declaration 需新增設定面與決策，已被討論排除。

**D2：政策集中於 §0，SKILL Override 對齊。** §0 是 NEUTRAL_RULES 唯一語言權威，SKILL body 多處 cite「§0 Language Policy in cwd CLAUDE.md」。在 §0 新增 quiz 子句（rule 3），SKILL `## Language Override` 改成同義且指回 §0，避免兩處政策再度 drift（本 bug 正是 drift 造成）。

**D3：複用既有 `topic` 欄位作語言訊號，不新增傳遞層。** `QuizGenerateOptions.topic` 已到底，只在 spawn `input` 組裝點併入 `topic=<...>` 即可。`Some` 帶、`None` 不帶——`None`／不帶即等同 Page flow 的舊行為，零回歸風險。

**D4：repair（regenerate）spawn 同步帶 topic。** content-verify 開啟時的 repair 也是 `sub_mode=generate` 的 quiz body 產出（`run_quiz_generate` 內 `repair` closure）。雖然 repair 指示「只改被 flag 的題、其餘 verbatim」，但被改寫的題仍可能語言飄移。為與 D3 一致、杜絕修復回合的語言缺口，repair `input` 在 `topic` 為 `Some` 時同樣帶 `topic=<...>`。這是 D3 的同檔同函式延伸，非額外子系統。

**D5：抽 `compose_generate_input` helper，比照 `compose_verify_input`。** 既有 `compose_verify_input(topic, pages, body)` 已是同檔的 compose 範式。新增 `compose_generate_input(topic: Option<&str>, pages, count) -> String`，generate 與 repair 兩處共用前綴組裝，純函式、單元可測。topic 段放在最前（與 verify input `topic=...` 開頭一致）。

**D6：codex drift guard 處理。** quiz SKILL body 改動若新增需 codex 對應的字串，須同步維護 `CODEX_BODY_TRANSLATIONS`（drift guard 測試 `every_codex_translation_from_appears_in_a_claude_body` / `drift_guard_detects_*` 會紅）。本次新文字（topic-follows / page-fallback）是 provider-neutral 純政策敘述，預期**不需**新增 translation；apply 時以 `cargo test -p codebus-core` 跑 drift guard 確認，若紅再補 table。

## Implementation Contract

**Behavior（觀察得到的結果）:**

- Goal flow：中文 topic → 中文題目/選項/解釋；英文 topic → 英文。
- Page flow（`topic=None`）：fallback 被考頁面語言 auto-detect，行為與改動前一致。
- 任何 flow：`## Answer:`、`## Explanation:`、`[CODEBUS_QUIZ_*]` marker 恆英文。

**Interface / data shape:**

- §0（`neutral.md`）`## 0. Language Policy` 段新增一條 quiz 子句：有 topic 跟 topic、無 topic fallback 頁面；quiz structural token 恆英文。
- quiz SKILL `## Language Override`（`QUIZ_SKILL_CONTENT`）改寫：刪「follow the quizzed wiki pages（dominant）」，改為「有 `topic=` 跟 topic、無 `topic=` fallback 頁面」。
- quiz SKILL Mode B 契約字串：`generate: pages=[<path1>,...] count=<N>` 新增可選 `topic=<...>` 欄位說明（Goal flow 帶、Page flow 不帶）。
- `run_quiz_generate` 的 generate spawn `input`：`topic` 為 `Some` 時含 `topic=<topic>` 段 + `pages=[...] count=<N>`；`None` 時維持 `pages=[...] count=<N>`。
- repair spawn `input`：`topic` 為 `Some` 時同含 `topic=<topic>` 段。
- 新 helper `compose_generate_input(topic, pages, count)` 純函式。

**Failure modes:**

- `topic=None` 是正常 Page flow 路徑，非錯誤；不帶段、不警告。
- topic 字串本身語言由 agent 判讀；codebus 不做語言偵測、不在無法判讀時報錯。

**Acceptance criteria:**

- `cargo test -p codebus-core` 全綠，含：drift guard 測試、`schema_neutrality.rs`（§0 斷言）、`quiz.rs` 內新增的 generate input compose 單元測試（Some 帶 `topic=`、None 不帶、repair 帶）。
- `cargo test -p codebus-cli` 不回歸。
- `cargo clippy --workspace` 無新增 warning。
- spec scenario 列舉：中文 topic→中文、英文 topic→英文、Page flow→fallback、structural token 恆英文。

**Scope boundaries:**

- **In scope:** `neutral.md` §0 一條子句；`mod.rs` quiz SKILL `## Language Override` + Mode B 契約字串（必要時 `CODEX_BODY_TRANSLATIONS`）；`quiz.rs` generate + repair spawn input compose（含新 helper 與單元測試）；連動測試更新。
- **Out of scope:** IPC / CLI 的 topic 傳遞層；plan spawn；fixed-language 宣告；全域語言設定；其他 verb（goal/query/fix/chat）的語言政策（rule 1/2 不動）。

## Risks / Trade-offs

- **R1（低）：agent 對「topic 語言」判讀。** 短 topic（如純英文技術詞 `JWT`）語言可能偏英文——但這正是 user 輸入的語言意圖，符合動態模型；可接受。
- **R2（低）：drift guard 連鎖。** 改 claude quiz body 可能要動 translation table；以 D6 的 test-driven 確認兜底。
- **R3（極低）：§0 文字計數斷言。** `schema_neutrality.rs` 只驗 §0 存在 + 含 `agent output`/`structural tokens` substring，新增子句不破壞；仍以測試確認。
- **Trade-off：** repair 也帶 topic（D4）略超出原始口頭 scope（只提 generate），但同檔同函式、消除語言缺口、零額外子系統——選工程最正確解。
