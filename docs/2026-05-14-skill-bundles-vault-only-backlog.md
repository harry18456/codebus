# Backlog: skill bundles 預設只寫 vault-internal copy

**Date:** 2026-05-14
**Surfaced during:** v3-app-workspace-goal apply（manual happy path 觀察）
**Severity:** design smell, no functional impact
**Owner:** harry
**Status:** parked — 等 v3-app-workspace-goal archive 後接著做

---

## 問題

`codebus init` 目前對每個 verb 寫 **兩份 byte-identical SKILL.md**：

```
<repo>/.codebus/.claude/skills/codebus-{goal,query,fix,chat}/SKILL.md   ← vault-internal
<repo>/.claude/skills/codebus-{goal,query,fix,chat}/SKILL.md            ← repo-root
```

兩處內容完全一樣（spec `Skill Bundle Dual Layout` 明定 byte-identical）。

## 為何寫兩份（spec rationale）

Claude Code 發現 skill 的規則是「掃 cwd 內 `.claude/skills/`」，cwd 不同看不到：

- codebus CLI / codebus app spawn agent 時 cwd = `<repo>/.codebus/` → 看 vault-internal copy
- 「user 自己在 source repo root 開純 `claude` 跑 `/codebus-goal`」→ 看 repo-root copy

兩個入口都要能 trigger，所以兩處都寫。

## 為何是 design smell

repo-root copy 是 **niche use case**——只有「不透過 codebus binary 直接用 Claude Code」的 power user 會用。對 80%+ codebus 用戶（走 app / CLI）這份 copy 從不被觸發：

- agent::invoke (`codebus-core/src/agent/claude_cli.rs:123`) 寫死 `cmd.current_dir(vault_root)` = `.codebus/`
- 所以 CLI + app 都只看 vault-internal copy

結果：

- 80% 用戶拿到一份永遠用不到的副本
- 兩份 drift 風險（write-if-missing 是兩處各自獨立判斷，user 改一邊另一邊不會同步）
- source repo 多一個 `.claude/skills/` 目錄（雖已加 `.gitignore`，cosmetic 上仍多餘）

## Proposed fix

新提一條 change：`v3-skill-bundles-vault-only`

### Spec 變動（MODIFIED `skill-bundles`）

- 預設 **只寫 vault-internal copy**（`<repo>/.codebus/.claude/skills/`）
- repo-root copy 改 **opt-in**：
  - CLI flag：`codebus init --with-repo-root-skills`
  - 或 config：`~/.codebus/config.yaml` 加 `skill_bundles.write_repo_root: true`
- spec normative 改：「by default only vault-internal copy is written」
- 既有 byte-identical scenario 改 conditional：「when repo-root copy is written, it SHALL be byte-identical to vault-internal」

### Migration

- 既有 vault 已有的 repo-root copy **不動**（preserve user customization、不破壞）
- 既有 `.gitignore` 規則保留（後續 init 不會 re-add 因為不再寫該位置，但既有規則無害）

### Tasks（粗估）

1. spec MODIFIED `skill-bundles`：寫法、scenarios 翻新
2. `codebus-core/src/skill_bundle/mod.rs`：`write_bundles_if_missing` 改接 `write_repo_root: bool` 參數
3. `codebus-core/src/vault/init.rs`：default false；CLI parse flag 傳入
4. `codebus-cli/src/commands/init.rs`：加 `--with-repo-root-skills` clap flag
5. 既有 tests 翻新（單 location vs dual location 兩條 path）

工程量：1–2 個半天。

## Out of scope

- 不刪除既有 repo-root copy（migration safety）
- 不改 vault-internal copy 行為（CLI / app 仍 work as-is）
- 不調整 `<vault>/.codebus/` agent cwd（Tauri app + CLI 都繼續用同一條 path）

## 何時動

v3-app-workspace-goal archive 完之後（即將）。或接著 D `v3-app-chat-cmdk` 之前處理也行——這條 change 跟 GUI 改動正交、可以獨立 ship。

## 替代：cosmetic 解決

若不想開新 change，僅手動清掉 + 加 gitignore 蓋住：

```bash
rm -rf <repo>/.claude/skills
echo '.claude/skills/codebus-*/' >> <repo>/.gitignore
```

但每次 `codebus init` (e.g., add_vault) 還是會重建——不是根治。
