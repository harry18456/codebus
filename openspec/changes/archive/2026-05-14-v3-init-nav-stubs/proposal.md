## Why

`codebus init` 預建 5 個 wiki 子資料夾（`wiki/{concepts,entities,modules,processes,synthesis}/`），卻沒預建 `wiki/index.md` 與 `wiki/log.md`。但 wiki lint 的 `nav-missing` rule 會把兩者列為 missing，導致**每個 vault 第一次 `codebus goal`** 不論 goal 是什麼，fix phase 必觸發 sub-agent 跑 ~3 次 `codebus lint --format json` + 寫兩份 nav stub + 偶爾修 broken-wikilink 誤判（log.md 範本內的 `[[slug1]]` 註解被誤判）— 約佔 first goal 總 wall-time / token 成本 10-30%，且對 user 觀感是「我問 goal、為什麼 codebus 又跑 fix-loop 在補 nav」。實際上 fix agent 補出來的也只是 stub（沒實際 goal context），等同 init 寫一份空 stub。把 stub 預建到 init step 既省 first-goal cost，又跟 5 個 taxonomy folders 預建邏輯一致。

## What Changes

- `vault::init::run_init` 新增「write nav stubs if missing」step（位置：write_skill_bundles 之後、write_settings 之前）
- 新 module 或 helper：`codebus-core/src/vault/nav_stubs.rs`（或 layout.rs 內擴），寫 `<vault>/wiki/index.md` + `<vault>/wiki/log.md`，內容 minimal frontmatter（title / type=`synthesis` / sources=`[]` / goals=`[]` / created+updated=今日 UTC YYYY-MM-DD / related=`[]` / stale=`false`）+ 一行 placeholder body（不含 `[[wikilink]]` 等 lint 會誤判的 token）
- Write-if-missing 語意：兩檔獨立判斷，存在就 preserve（user 已 customized 不覆寫）
- 加 `InitEvent::NavStubsDone { vault_root, written, preserved }` 給 CLI / app banner observer
- spec MODIFIED `vault` § Vault Layout requirement：加「init 預建 index.md + log.md 含 minimal frontmatter」normative + 新 scenarios
- 既有 `nav-missing` lint rule 維持（防 user 手動刪檔的 safety net），但 fresh init 後不再 fire — fix verb 內 nav-missing 修補步驟自然 short-circuit（一旦兩檔存在 lint 0 errors）

## Non-Goals

- **不改 lint rule `nav-missing` 本身**：rule 仍 fire when files actually missing，只是 fresh init 後不會 fire；rule 是 safety net，本 change 不縮 scope
- **不改 fix verb 行為**：fix SKILL.md 內 nav-missing 修補指示維持；只是新的 fresh vault 不會 trigger 該 path
- **不寫 placeholder 內容到 nav 預建檔以外**：5 個 taxonomy folder 仍是空 dir、不預建任何 stub `.md`（taxonomy page 是 content，由 goal agent 寫；nav 是 structure，由 init 寫）
- **不改 SKILL.md GOAL_WORKFLOW**：既有 schema `Page Conflict Rules`「page exists → append `## from goal:` section」已 cover nav files；agent 看到 index/log 已存在會自然 append、不會 overwrite
- **不改 `wiki/concepts/`, `wiki/entities/`, etc. 預建行為**：folder 預建不動
- **不引入新 lint rule**：本 change 純 init-side 改動 + spec MODIFY，不擴 lint
- **不改 source-signal manifest schema**：nav stubs 跟 manifest source-signal 算法解耦，不影響 drift detection
- **不改 `nav-missing` 在 chat / query verb 內行為**：那些 verb 不跑 fix-loop，本來就不會 auto-fix nav

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `vault`：Vault Layout requirement 擴 init 預建 nav files；加對應 scenarios

## Impact

- Affected specs: `vault` (modified)
- Affected code:
  - Modified:
    - codebus-core/src/vault/init.rs
    - codebus-core/src/vault/mod.rs
  - New:
    - codebus-core/src/vault/nav_stubs.rs
  - Removed: (none)
