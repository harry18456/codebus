## Why

prompt surface deep review（見 `docs/2026-05-23-prompt-surface-inventory.md` §8）在 Layer 1 抓到 19 個 finding，含 1 個 🔴 CRITICAL：

- **F1 (🔴)**：`§0 Language Policy` 不存在但被 5 處 SKILL workflow 引用（`codebus-core/src/skill_bundle/mod.rs:273, 316, 569, 867, 885`），agent 找不到時 fallback 模型內建 heuristic 猜語言。多語言「幸運能跑」靠 LLM mirror 本能，**契約上是壞的**。
- 其餘 18 個（F2-F10, F12-F18a, F11a）：schema 規則描述模糊 / taxonomy 列兩次 / frontmatter 欄位語意未定義 / wikilink anchor 不支援未聲明 / PII boundary 描述太弱 / page body 範例缺 / `CODEX_AGENTS_SOFT_CONSTRAINT` 內容自我削弱（"soft constraint"）等。

19 個 finding 全屬同層、集中於同檔案（`codebus-core/src/schema/neutral.md`）外加 1 個 Rust const string（`skill_bundle/mod.rs` 內 `CODEX_AGENTS_SOFT_CONSTRAINT`），單批改最小化 review 噪音與 commit 散落。

## What Changes

- 在 `codebus-core/src/schema/neutral.md` **§1 之前補 §0 Language Policy**（修 F1）。內容草稿已備於 inventory §8 F1（line ~825）。
- 整理 `neutral.md` §1-§9 共 18 處措辭 / 結構 finding（F2-F10, F12-F18a），包括：
  - **F2**：§1 把 `wiki/` 標 READ-only 但實際 RW → 修正
  - **F6**：§2 taxonomy 重複列兩次 → 合併
  - **F8**：§7 step budget 30 是憑空數字 → 補 rationale 或改 soft target 字眼
  - **F10**：§6 source code citation 只示範 Python → 補多語言範例
  - **F12**：§5 wikilink 沒說 heading anchor 不支援 → 補一行明示
  - **F13**：§4 `stale: false` 生命週期未定義 → 補語意
  - **F15**：PII boundary 描述太弱 → 補強
  - **F16**：§8 Out-of-scope 範例 CJK-heavy → 加 EN 範例
  - **F18a**：§4 frontmatter 沒明示 required vs optional → 加標註
  - 其他 F3 / F4 / F5 / F7 / F9 / F14 / F17 / F18 同類措辭調整
- 改寫 `codebus-core/src/skill_bundle/mod.rs:156-164` `CODEX_AGENTS_SOFT_CONSTRAINT` 內容（修 F11a 四子項）。對照版已備於 inventory §8 F11a（line ~976）。修法重點：
  - heading 改規則式（去除 "vs" 對比框架）
  - 移除「claude path 怎麼做」meta-info
  - 移除「soft constraint / self-discipline」自我削弱字眼
  - 收緊「proactively」模糊副詞，加「even if the user prompt names them」
  - 加 fallback「refuse and explain the scope」
  - 補 `~/.config/` credential subdir（gh / azure CLI / token 常見位置）

## Non-Goals (optional)

- **F11（`CODEX_AGENTS_SOFT_CONSTRAINT` 位置 append → templated insert）**：留 backlog 另議。該 finding 要動 `codebus-core/src/vault/init.rs:329` 插入邏輯 + 新測試，與此批字串編輯性質不同；F11 是 MEDIUM（非 CRITICAL），不阻塞 Phase 2/3/4。
- **Layer 2（5 verb SKILL body）**：Phase 2 SKILL split 處理，不在此批。
- **Layer 3（10 spawn prompt 模板）**：Phase 3 SpawnSpec 重構處理，不在此批。
- **不改 `schema_neutrality` test 既有斷言**：forbidden tokens / 5 folder names / >1000 chars 全部 still pass。
- **僅 F1 進 spec 層**：F1 §0 Language Policy 是 SKILL 5 處引用的契約點，加入後讓引用變實，因此進 `skill-bundles` spec delta（ADDED Requirement）。其他 18 個 finding 是內容澄清 / 字串收緊，仍在現有實作 boundary 內，公開 boundary 由既有 `schema_neutrality` test 守。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `skill-bundles`：在 schema rules（`NEUTRAL_RULES`，materialized as vault `CLAUDE.md` / `AGENTS.md`）加入 Language Policy 規則段（§0）契約點，constrain agent 輸出語言。SKILL workflow body 5 處已引用此契約點，加入後引用變實、不再 dangling。其他 18 個 finding 屬同一 capability 的 implementation 強化（schema 內容澄清 + `CODEX_AGENTS_SOFT_CONSTRAINT` 內容收緊），不另立 requirement。

## Impact

- Affected code:
  - Modified: `codebus-core/src/schema/neutral.md`、`codebus-core/src/skill_bundle/mod.rs`
- Tests:
  - `codebus-core/tests/schema_neutrality.rs` 三個斷言 still pass（forbidden tokens / 5 folder names / >1000 chars）
  - `codebus-core/tests/vault_init.rs` line 254-263 涉 `NEUTRAL_RULES` 寫入 + lower-case 檢查，實作時需確認斷言不依賴特定行
- 下游影響：`skill_bundle/mod.rs:273, 316, 569, 867, 885` 共 5 處 SKILL 引用「§0 Language Policy in cwd CLAUDE.md」於 F1 修法後自動 valid，不用動引用方
