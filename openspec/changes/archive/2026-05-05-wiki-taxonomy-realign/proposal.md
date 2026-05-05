## Why

對照 Karpathy 真本（`https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f`）盤點 codebus 的 wiki taxonomy，發現幾個 phase 1 自我加碼的強制規定脫離原意又有具體後遺症（uv repo spike 暴露），需要回收簡化：

1. **`wiki/goals/<slug>.md` per-goal reading guide** — Karpathy 沒此概念。spike 4 個 goal 都產出但從沒被 user 回讀過；narrative 內容跟 `log.md` 對應 entry 大量重疊；re-run detection schema 自己標註「fragile」。
2. **`wiki/overview.md` 強制 rewrite each run** — Karpathy 沒把 overview 當 named root file，只把 overview/synthesis 當一種 page type。spike 觀察 goal 4 的 overview 完全覆蓋 goal 1 寫的版本（不是累積）—「rewrite each run」是錯的設計。
3. **5 type folder 強制 + folder/type mismatch lint warn** — Karpathy 是 flat `wiki/` + 軟分類「entities, concepts, sources 等」。codebus 強制 5 folder + 用 lint warn 推斷 type/folder 一致性，spike 中 false positive 已干擾正常 markdown（inline code、table escape 已記在另一 backlog）。

`module` 跟 `process` 兩個 type 是 codebus 對 Karpathy 合理的 code domain 延伸（spike 100%/50% 觸發率驗證），這次保留 type 但移除 folder hard-enforce。

## What Changes

- **BREAKING（schema）**：移除 `wiki/goals/<slug>.md` 的「per-goal reading guide」要求，原本 schema §4.1 step 7 的 narrative 收進 step 6 (log.md) 變更詳細的 chronological entry
- **BREAKING（schema）**：`wiki/overview.md` 不再是 wiki/ 根目錄強制 named file，也不再要求每 goal rewrite。Overview 性質的 page 改成 `wiki/synthesis/<slug>.md`（用既有 5 type taxonomy 中的 synthesis），由 agent 在累積足夠 page 後自決時機建立
- **BREAKING（lint）**：移除 folder/type mismatch warn — frontmatter `type` 仍須是 5 enum 之一（concept / entity / module / process / synthesis），但檔放哪個 folder 不再被 lint 當 mismatch 警告。folder 仍是 organizational hint（init 仍預先建 5 個目錄供 Obsidian sidebar 排列）
- **BREAKING（lint）**：`SPECIAL_FILES` 從 `[overview.md, index.md, log.md]` 縮為 `[index.md, log.md]`。missing overview.md 不再 warn
- **BREAKING（lint）**：移除 wiki/goals/ 的 wikilink catalog 來源跟 body scan — slug catalog 改只含 5 type folder + index/log 兩個 special
- **BREAKING（init）**：`runInit` 不再 mkdir `wiki/goals/`；對應的 `wikiGoals` layout path 從 `VaultPaths` 介面移除
- **不動的**：5 type folder 仍由 init 預先建立；frontmatter type enum 仍含 5 種；page-merge / stale-detect / sandbox / auto-lint / `--check` 行為不變

## Non-Goals

- **不寫 vault migration 工具**：existing 用戶 `.codebus/` 裡舊的 `goals/` 目錄、`overview.md`、folder/type mismatch 檔案維持原狀，不自動清理。新規定只對未來 goal 生效。Lint 看到舊 `goals/<slug>.md` 不會 special-cased（變成 wiki/ 根的 unknown .md → warn），user 自行決定要不要刪
- **不改 page-merge 邏輯**：本 change 不解 spike REPORT backlog 第 3 條「page-merge bias too weak」。那個跟本 change 同家族但解法不同（schema 語言修改 vs taxonomy 縮編），分開做
- **不解決 lint markdown false positive**：spike 報的 inline code + table escape 是另一個 lint regex bug，獨立 change（建議名 `lint-markdown-aware-scan`）
- **不重新設計 `--query` system prompt**：query 仍讀 index.md 為 nav，但 prompt 文字會跟 schema 同步去除 overview / goals 的提及；不另外設計 query 用的新 nav 機制
- **不改 frontmatter schema**：`goals[]` array、`sources[]`、`type` enum 等欄位都不動

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `vault-init`: 移除 `wiki/goals/` 目錄建立要求；Initialize 場景的目錄列表更新
- `wiki-lint`: 移除 folder/type mismatch warning；`SPECIAL_FILES` 從 3 縮為 2（去掉 overview.md）；移除 goal guides 作為 wikilink catalog 來源跟 body scan 標的；`navFilesScanned` 計數定義對應調整

**Note on schema-only changes**: 本 change 也會修 agent 系統 prompt 模板（schema 模組）— 移除 §4.1 step 7、§3 overview-as-named-file、5-type 措辭軟化等。但這些變更不對應任何 spec-level requirement（schema 內容由對應 schema test 鎖住關鍵 phrase，不是 normative spec scenario）。因此本 change 只列上述兩個 capability 為 Modified；schema content 變更走 code 層，於 Impact 段中明列檔案路徑。

## Impact

- Affected specs:
  - Modified: `openspec/specs/vault-init/spec.md`
  - Modified: `openspec/specs/wiki-lint/spec.md`
- Affected code:
  - Modified: `src/schema/claude-md.ts` (remove §4.1 step 7, expand step 6, drop overview.md from §3 named files, drop goals/ from §3, soften 5-type folder language to "organizational hint")
  - Modified: `src/commands/init.ts` (remove the wikiGoals mkdir line)
  - Modified: `src/core/vault/layout.ts` (remove `wikiGoals` from `VaultPaths` interface and from return shape)
  - Modified: `src/core/wiki/lint.ts` (remove §3 folder/type mismatch emission, remove `'overview.md'` from `SPECIAL_FILES`, remove §1c goal-guides catalog source, remove §6 goal-guides body scan loop)
  - Modified: `tests/core/wiki/lint.test.ts` (drop folder/type mismatch test, drop overview-related missing/body tests, drop goal-guide resolution and broken-wikilink tests, update navFilesScanned counting tests)
  - Modified: `tests/core/vault/layout.test.ts` (drop `wikiGoals` path expectation)
  - Modified: `tests/commands/init.test.ts` (drop wiki/goals/ creation expectation)
  - Modified: `tests/commands/goal.test.ts` (rework SabotageGoalsProvider — original sabotaged `wiki/goals/`; replace with another lintWiki throw vector or remove the lint:null fallback test if no equivalent vector remains)
  - Modified: `tests/schema/claude-md.test.ts` (drop assertions on overview-related and goal-guide-step phrases)
