## Why

同仁回報：被考的 wiki 頁面是中文，但 quiz 題目卻考出英文。根因是 quiz SKILL 的語言政策與全域 §0 Language Policy 互相矛盾——§0 規定 agent 輸出語言要跟隨「prompt context 語言」（user 的 goal/query/chat 文字），明確不跟隨 wiki 頁面語言；但 quiz SKILL 的 `## Language Override` 反向要求題幹/選項/解釋跟隨「被考 wiki 頁面語言 auto-detect（混雜時取 dominant）」。技術頁面夾雜大量英文（slug、frontmatter、code identifier、`[[wikilink]]`），auto-detect 常把 dominant 判成英文，於是中文 wiki 考出英文題。再加上 quiz generate spawn 的 input 根本沒帶 topic，generate agent 連一個可跟隨的語言訊號都沒有。

## What Changes

- **§0 Language Policy（NEUTRAL_RULES）** 擴一條涵蓋 quiz：題幹/選項/解釋的語言在「有 quiz topic」時跟隨 **quiz topic 語言**（Goal flow）；無 topic 時（Page flow / wiki-preview「Quiz me on this」）fallback 到被考頁面語言 auto-detect。structural token（`[CODEBUS_QUIZ_*]`、`## Answer:`、`## Explanation:`）恆英文不變。
- **quiz SKILL `## Language Override`** 改寫：刪掉「follow the language of the quizzed wiki pages」那條，改為「有 topic 時跟隨 quiz topic 語言；無 topic 時 fallback 頁面 auto-detect」，與 §0 對齊。
- **quiz SKILL Mode B input 契約**（`generate: pages=[...] count=<N>`）新增可選 `topic=<...>` 欄位，作為語言訊號的承載。
- **quiz.rs `run_quiz_generate`**：當 `options.topic`（既有欄位，已 plumb 到底）為 `Some` 時，把它併進 generate spawn 的 input 字串；`topic=None`（Page flow）維持現狀不帶。content-verify repair regenerate spawn 同步比照（同一函式內、同一語言訊號缺口），以免修復回合把語言改飄。

## Non-Goals

- 不改成固定宣告語言（不 hard-code 中/英）。
- 不新增全域語言設定旋鈕——那是另一個更大的決定，明確 out of scope。
- 不動 IPC / CLI 的 topic 傳遞層（`QuizGenerateOptions.topic` 已被兩 caller 填妥，本 change 只「複用」）。
- 不改 structural token / marker 的英文恆定規則。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `skill-bundles`: §0 Language Policy 擴涵 quiz topic-follows 規則；Quiz Skill Bundle Content 的 `## Language Override` 改為跟隨 topic（有則跟、無則 fallback 頁面），Mode B input 契約新增可選 `topic=<...>`。
- `quiz`: `run_quiz_generate` 在 `topic` 為 `Some` 時把 topic 併進 generate（及 repair regenerate）spawn 的 input，作為語言訊號；`None` 時行為不變。

## Impact

- Affected specs: `skill-bundles`, `quiz`
- Affected code:
  - Modified:
    - codebus-core/src/schema/neutral.md（§0 Language Policy 擴一條 quiz 規則）
    - codebus-core/src/skill_bundle/mod.rs（`QUIZ_SKILL_CONTENT` 的 `## Language Override` 與 Mode B input 契約；如新增字串需 codex 對應，維護 `CODEX_BODY_TRANSLATIONS`）
    - codebus-core/src/verb/quiz.rs（`run_quiz_generate` 的 generate 與 repair regenerate input compose；新增 compose helper 比照 `compose_verify_input` 風格）
  - Tests（連動可能需更新／新增）:
    - codebus-core/tests/schema_neutrality.rs（§0 斷言）
    - codebus-core/src/skill_bundle/mod.rs 內 drift guard 測試（`every_codex_translation_from_appears_in_a_claude_body` 等）
    - codebus-core/src/verb/quiz.rs 內 generate input compose 單元測試
