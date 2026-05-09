## 1. core::git 模組 carry

- [x] 1.1 在 `codebus-core/src/git/mod.rs` 建立模組入口，宣告 `pub mod nested_repo;` 並 re-export `pub use nested_repo::{init_nested_repo, auto_commit};`，呼應 design.md「nested repo author hardcode `codebus@local` / `codebus`」決策的 module surface
- [x] 1.2 在 `codebus-core/src/git/nested_repo.rs` 實作 `pub fn init_nested_repo(vault_root: &Path) -> io::Result<()>`：若 `vault_root.join(".git")` 已存在 return Ok(())（idempotent no-op）；否則跑 `git init -b main -q` 後 `git config user.email codebus@local` + `git config user.name codebus`，全用 `std::process::Command::new("git").current_dir(vault_root)`；落實 Nested Git Repository Initialization requirement 的「First init creates nested git repo with codebus identity」+「Re-init does not overwrite user-modified local git config」scenarios
- [x] 1.3 在 `codebus-core/src/git/nested_repo.rs` 實作 `pub fn auto_commit(vault_root: &Path, message: &str) -> io::Result<String>`：先 `git add -A`，再 `git status --porcelain` 判斷 working tree 是否 clean；clean 則回現有 HEAD sha（`git rev-parse HEAD`）；dirty 則 `git commit -m <message> -q` 後回新 HEAD sha；落實 Initial Auto-Commit On Init requirement 的 working-tree-clean 子句
- [x] 1.4 在 `codebus-core/src/git/nested_repo.rs` 補 unit tests：`init_creates_dot_git_with_codebus_identity`（assert `.git/` 存在 + email/name 對）、`init_is_idempotent`（重跑不 panic）、`auto_commit_writes_changes`（dirty 路徑回 40-char sha + working tree clean）、`auto_commit_returns_existing_head_when_clean`（clean 路徑連續兩次 sha 相同）、`auto_commit_message_appears_in_log`（`git log --pretty=%s -1` 對得上）；以 v2 carry test set 為基底
- [x] 1.5 在 `codebus-core/src/lib.rs` 加 `pub mod git;` export 模組

## 2. init.rs 接 nested git + 收尾 auto_commit

- [x] 2.1 在 `codebus-cli/src/commands/init.rs` 新增 const `INTERNAL_GITIGNORE_LINES: &[&str] = &[".lock", "raw/code/", "**/.obsidian/", "logs/"];`，並新增 helper `fn merge_internal_gitignore(vault_root: &Path) -> io::Result<()>`：file 不存在則建 4 行；存在則 append 缺的（保留 user 已有 line 順序與 user-added 額外 line），落實 Internal Vault .gitignore Content requirement
- [x] 2.2 在 `codebus-cli/src/commands/init.rs::run` 函數內 wire 進 design.md 規定的「init 流程順序：raw_sync → internal gitignore → nested repo init → 後續產物 → 收尾 auto_commit」決策步驟：raw_sync 之後呼叫 `merge_internal_gitignore(&paths.root)`，再呼叫 `codebus_core::git::init_nested_repo(&paths.root)`，再走原有 source gitignore / schema / manifest / skill bundles / obsidian register 步驟，最後在所有步驟成功後呼叫 `codebus_core::git::auto_commit(&paths.root, "init: codebus vault")`；nested git init 或 auto_commit 失敗則 eprintln + `ExitCode::from(1)`，落實 design.md「`auto_commit` 失敗 = init 失敗」決策與 Initial Auto-Commit On Init requirements 的「Auto-commit failure surfaces as init non-zero exit」scenario
- [x] 2.3 在 `codebus-cli/src/commands/init.rs` 加 stdout progress lines：`✓ vault git: nested repo initialized`（init_nested_repo 成功時印；no-op case 印 `✓ vault git: already initialized`）跟 `✓ vault git: committed <sha7> "init: codebus vault"`（auto_commit 成功時印 7-char sha 前綴）

## 3. vault layout test 對齊

- [x] 3.1 在 `codebus-core/src/vault/layout.rs` 的 unit test `create_vault_layout_does_not_create_v2_legacy_paths` 內移除 `assert!(!p.root.join(".git").exists(), "nested .git/ must not exist")` 那行；改為斷言 `output/` 跟 `goals.jsonl` 仍不存在（保留 v3-init 的 V2 legacy paths 排除子集），呼應 Vault Layout requirement 反轉子句

## 4. integration test for nested git + first commit

- [x] 4.1 在 `codebus-cli/tests/cli_routing.rs` 新增 helper `run_full_init_via_cli(repo_path: &Path)`（spawn release binary + `--no-obsidian-register` + capture stdout/stderr/exit）作為 integration 入口；呼應 design.md 整體 init 流程驗證
- [x] 4.2 在 `codebus-cli/tests/cli_routing.rs` 新增整合 test `nested_git_repo_present_with_codebus_identity_after_init`：跑 init、斷言 `.codebus/.git/` 存在 + `git config --get user.email` 為 `codebus@local` + `user.name` 為 `codebus`，落實 Nested Git Repository Initialization requirement 的 First init scenario
- [x] 4.3 在 `codebus-cli/tests/cli_routing.rs` 新增整合 test `init_produces_canonical_init_commit`：跑 init、斷言 `git log --pretty=%s -1` 輸出 `init: codebus vault`、`git status --porcelain` 為空、`git ls-tree -r HEAD --name-only` 含 `CLAUDE.md` 跟 `manifest.yaml`、不含任何 `raw/code/` 開頭的 path；落實 Initial Auto-Commit On Init requirement 的「canonical init message」「captures CLAUDE.md and manifest」「excludes raw/code」三條 scenario
- [x] 4.4 在 `codebus-cli/tests/cli_routing.rs` 新增整合 test `re_init_preserves_user_modified_git_config`：先跑一次 init、手動 `git -C .codebus config user.email alice@example.com`、再跑一次 init、斷言 user.email 仍為 alice@example.com；落實 Nested Git Repository Initialization requirement 的「Re-init does not overwrite user-modified local git config」scenario，並驗證 design.md「既有 .codebus/ 重跑 init 自動 promote 進 nested tracking」決策的「不破壞 user 修改過的 local config」邊界
- [x] 4.5 在 `codebus-cli/tests/cli_routing.rs` 新增整合 test `internal_gitignore_appends_missing_required_lines`：先建 vault layout、手寫 `.codebus/.gitignore` 含 `.lock\nnotes/\n`、跑 init、斷言檔案內含原順序的 `.lock`+`notes/` 後接著 append 的 `raw/code/`+`**/.obsidian/`+`logs/`；落實 Internal Vault .gitignore Content requirement 的 append-missing scenario

## 5. workspace 全綠

- [x] 5.1 跑 `cargo test --workspace` 全綠，含本 change 新增 unit + integration tests 與既有 v3-init / v3-pii test 全部通過
