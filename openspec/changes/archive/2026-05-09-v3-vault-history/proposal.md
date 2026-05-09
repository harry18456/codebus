## Why

v3-init #2 archive 故意排除 `.codebus/` 內 nested git，把 vault diff 歷史 deferred。但 #5 v3-goal 跟 #8 v3-fix 的 spawn 收尾都需要 `auto_commit "wiki: ..."` 把 wiki 變動寫進 vault repo —— 這要求 nested repo 在 #5/#8 開始前就已存在且有 first commit。v2 carry 的做法是 init 階段直接建 nested repo + 第一個 `auto_commit "init: codebus vault"`（[v2 init.rs:34, 44](file:///D:/side_project/codebus/legacy/v2-rust/codebus-cli/src/commands/init.rs)）；v3 採同樣 timing，把 vault-history 從原本「#5 後 follow-up」拉到主序列 #4。

## What Changes

- 修改 capability `vault`：
  - 反轉 Vault Layout requirement —— 從「SHALL NOT create `.codebus/.git/`」改為「SHALL create nested git repository at `.codebus/`」
  - 新增 Internal `.codebus/.gitignore` Content requirement：vault-internal `.gitignore` 含 `.lock` / `raw/code/` / `**/.obsidian/` / `logs/` 4 行（v2 carry）
  - 新增 Nested Git Repository Initialization requirement：init 階段執行 `git init -b main` + 設 local `user.email=codebus@local` / `user.name=codebus`（不依賴 user 全域 git config）
  - 新增 Initial Auto-Commit On Init requirement：init 結尾呼叫 `auto_commit` 帶 message `init: codebus vault`，把所有 init 階段產物（vault layout + raw mirror + CLAUDE.md + manifest + skill bundles）一次 commit 進 nested repo
- 新增 Rust 模組 `codebus-core/src/git/nested_repo.rs`（v2 carry）：
  - `pub fn init_nested_repo(vault_root: &Path) -> io::Result<()>`：idempotent；存在 `.git/` 即 no-op
  - `pub fn auto_commit(vault_root: &Path, message: &str) -> io::Result<String>`：`git add -A` + `git commit -m`；無變動時回現有 HEAD sha；回 40-char sha
- 修改 init 流程順序：raw_sync → 內部 .gitignore 寫入 → init nested repo → source gitignore mutation → schema → manifest → skill bundles → obsidian register → **第一個 auto_commit 收尾**

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `vault`: Vault Layout requirement 反轉（移除 nested `.git/` 禁建子句，改為 SHALL build）；新增 3 個 init 階段 nested git 相關 requirements

## Impact

- Affected specs:
  - Modified: `vault`
- Affected code:
  - New:
    - codebus-core/src/git/mod.rs
    - codebus-core/src/git/nested_repo.rs
  - Modified:
    - codebus-core/src/lib.rs
    - codebus-core/src/vault/layout.rs
    - codebus-cli/src/commands/init.rs
    - codebus-core/tests/vault_init.rs
  - Removed: 無
