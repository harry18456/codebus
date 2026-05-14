## Why

`codebus init` 目前對每個 verb 寫兩份 byte-identical SKILL.md（vault-internal `<repo>/.codebus/.claude/skills/` + repo-root `<repo>/.claude/skills/`），但實際 codebus app + CLI 的 agent spawn cwd 都鎖在 `<repo>/.codebus/`（見 `codebus-core/src/agent/claude_cli.rs:123` `cmd.current_dir(&opts.vault_root)`）——repo-root copy 只有「user 在 source repo root 直接跑純 `claude` CLI 並打 `/codebus-<verb>` slash command」這種 niche workflow 會用到。對 80% 走 app / CLI 的使用者來說，那份 copy 從不被觸發，純粹是 cosmetic 多餘 + 有 write-if-missing drift 風險（user 編輯一邊另一邊不會同步）。本 change 把 repo-root copy 改成 **opt-in**，default 只寫 vault-internal copy；power user 需要的話用 `--with-repo-root-skills` flag 顯式打開。

## What Changes

- `codebus init` default 行為改成 **只寫 vault-internal copy**（4 個 verb 共 4 個 SKILL.md，落在 `<repo>/.codebus/.claude/skills/codebus-{goal,query,fix,chat}/`）
- 新增 CLI flag `--with-repo-root-skills`（init subcommand）——傳入時恢復既有「同時寫 repo-root copy」行為，並把 4 個 repo-root 路徑加進 source repo 的 `.gitignore`
- `codebus-core::skill_bundle::write_bundles_if_missing()` 簽名加 `write_repo_root: bool` 參數
- `codebus-core::vault::init::run_init` 端 plumb 該 flag（透過 `InitOptions`，default false）
- `codebus-core::vault::source_gitignore` mutation step 改成 conditional——只在 `write_repo_root: true` 時加 repo-root patterns
- spec MODIFIED `skill-bundles`：
  - `Skill Bundle Layout` requirement 從「SHALL create at BOTH locations」改成「SHALL create at vault-internal location; MAY create at repo-root location when caller requests it」
  - 既有「byte-identical between locations」scenario 改成 conditional——只在兩份都寫的情況下適用
  - 既有「Init adds repo-root skill bundle directories to source gitignore」scenario 改成 conditional
- 既有 vault 重跑 init **不會** 動 repo-root copy（write-if-missing 不變、單邊 preserve）

## Non-Goals

- **不刪除既有 repo-root copy**——已 init 過的 vault 那份保留不動，避免 user 已客製化的內容被清掉
- **不改 vault-internal copy 行為**——既有 4 個 verb 的 vault-internal 寫入規則、write-if-missing 語意完全不動
- **不改 agent spawn 的 cwd**——app + CLI 仍 spawn 在 `<repo>/.codebus/`，repo-root copy 從來就不是它們的 discovery path
- **不擴 SKILL.md 內容**——本 change 純 location toggle，bundle content / frontmatter / workflow body 不動
- **不擴 init 其他 step**——本 change 只動 skill_bundle write + source_gitignore mutation 兩處
- **不擴 `~/.claude/skills/` user-global location**——既有 spec 禁止這條，本 change 維持

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `skill-bundles`：dual-write 從 mandatory 改成 default vault-only + opt-in repo-root；既有 byte-identical / gitignore mutation scenarios 改成 conditional

## Impact

- Affected specs: `skill-bundles` (modified)
- Affected code:
  - Modified:
    - codebus-core/src/skill_bundle/mod.rs
    - codebus-core/src/vault/init.rs
    - codebus-core/src/vault/source_gitignore.rs
    - codebus-cli/src/commands/init.rs
  - New: (none)
  - Removed: (none)
