<!--
每個 task：(1) 交付的行為或契約 + (2) 完成驗證目標。
File path 僅供 locator context；task 描述須含 spec requirement name 與行為的 substring。
Locale zh-tw；symbol / 路徑 / requirement 名稱保英文。
[P] 標示與群組內其他 [P] 不共享 file 且互不依賴，可平行執行。
本 tasks.md 覆蓋 proposal § What Changes 範圍與 spec MODIFIED 全部 scenarios。
-->

## 1. Core API — InitOptions + write_bundles_if_missing 加 opt-in flag

- [x] 1.1 `codebus-core/src/vault/init.rs` 為 `InitOptions` 加 `pub with_repo_root_skills: bool` 欄位（預設 `false`，既有 callers `Default::default()` / 顯式構造若無此欄則 fallback 到 false）— 落實 spec § Skill Bundle Layout「caller explicitly requests it via InitOptions」。**驗證**：unit test `init_options_default_disables_repo_root_skills`（構造 `InitOptions::default()`、assert `with_repo_root_skills == false`）。
- [x] 1.2 `codebus-core/src/skill_bundle/mod.rs` 為 `write_bundles_if_missing` 加 `write_repo_root: bool` 參數 — true 時雙寫（既有 6 outcomes 行為）、false 時只寫 vault-internal（回 4 outcomes Vec）— 落實 spec § Skill Bundle Layout「creates only vault-internal by default」與「creates at both locations when caller requests it」。**驗證**：unit test `write_bundles_default_vault_only_returns_four_outcomes`（call with `write_repo_root: false`、assert outcomes.len()==4 + 4 個 verb 的 vault-internal SKILL.md 存在 + repo-root 4 個路徑全不存在）；unit test `write_bundles_with_repo_root_returns_eight_outcomes_byte_identical`（call with `write_repo_root: true`、assert outcomes.len()==8 + 4 verb 兩處 SKILL.md bytes 互等）。
- [x] 1.3 翻新 `codebus-core/src/skill_bundle/mod.rs` 既有 test `skill_bundles_creates_eight_outcomes_no_lint_at_either_location` — 拆成「default 4 outcomes」+「opt-in 8 outcomes」兩條，並保留「no lint bundle at either location」斷言在兩條內各驗一次。**驗證**：`cargo test -p codebus-core --lib skill_bundle` 既有 4 條 tests（slug_replaces / name_stable / first_write / five_writes / parent_directory / three_variants_all_persist — 對 EventsJsonlSink 而言；對 skill_bundle 是 `skill_bundles_*` 系列）全綠且包含新的兩條 default / opt-in 分支 test。

## 2. Vault init 連線 flag

- [x] 2.1 `codebus-core/src/vault/init.rs` `run_init` 內把 `options.with_repo_root_skills` 傳給 `skill_bundle::write_bundles_if_missing` 與 source-gitignore mutation step — 落實 spec § Skill Bundle Layout 連結 InitOptions ↔ skill_bundle write 行為。**驗證**：integration test `run_init_default_writes_only_vault_internal_skills`（呼叫 `run_init` with `InitOptions::default()`、assert vault-internal 4 個 SKILL.md 存在、repo-root 4 個路徑全部 `.exists() == false`）；integration test `run_init_with_repo_root_skills_writes_both_locations`（同上但 `InitOptions { with_repo_root_skills: true, ..Default::default() }`、assert 8 個 SKILL.md 存在）。
- [x] 2.2 [P] `codebus-core/src/vault/source_gitignore.rs`（或 init.rs 內 gitignore mutation 區塊，視當前實作位置）改為 conditional — `with_repo_root_skills: false` 時跳過加入 `.claude/skills/codebus-*/` 4 條 patterns；`true` 時加入既有 4 條 patterns — 落實 spec § Skill Bundle Layout「.gitignore mutation step adds patterns only when repo-root skills written」。**驗證**：integration test `run_init_default_does_not_add_repo_root_skill_gitignore_patterns`（fresh repo、`InitOptions::default()`、assert source `.gitignore` 不含 `.claude/skills/codebus-` 子串）；test `run_init_with_repo_root_skills_adds_gitignore_patterns`（同上但 opt-in、assert 4 條 patterns 全部加入）。

## 3. Re-init preserve semantics

- [x] 3.1 [P] 新增 integration test `re_init_default_preserves_existing_repo_root_bundles`（在 codebus-core/tests/vault_init.rs 或 skill_bundle tests file）— pre-seed `<repo>/.claude/skills/codebus-goal/SKILL.md` 含自訂內容、再跑 `run_init` with `InitOptions::default()`（無 opt-in）；assert 該 file bytes 完全未變、且本次 invocation 不會嘗試刪或改 repo-root copy — 落實 spec § Skill Bundle Layout「Existing repo-root bundles are preserved across re-init even without opt-in」scenario。

## 4. CLI flag — codebus init --with-repo-root-skills

- [x] 4.1 `codebus-cli/src/commands/init.rs` 加 clap derive flag `--with-repo-root-skills`（bool、default false、help 文字英文 single line）、傳入 `InitOptions { with_repo_root_skills: <flag>, ..既有 }` — 落實 spec § Skill Bundle Layout「caller requests via `codebus init --with-repo-root-skills`」。**驗證**：cli_routing test `init_subcommand_accepts_with_repo_root_skills_flag`（用 `assert_cmd` 或既有 clap parse test 工具、assert `--with-repo-root-skills` 不報 unknown argument、解析後 InitOptions 對應 field 為 true）；既有 `codebus init <path>` 無 flag invocation 仍 work（既有 `init_*` cli tests 全綠不破）。
- [x] 4.2 [P] `codebus-cli/tests/cli_routing.rs` 或 `vault_init.rs` 新增 end-to-end test `init_with_repo_root_skills_writes_both_locations_via_cli`（spawn `codebus init <tmp> --with-repo-root-skills`、assert 8 個 SKILL.md 存在、`<tmp>/.gitignore` 含 4 條 `.claude/skills/codebus-*/` patterns）；test `init_without_flag_writes_only_vault_internal_via_cli`（同上但無 flag、assert 4 個 vault-internal + 0 repo-root + `.gitignore` 不含對應 patterns）。

## 5. Verification 收尾

- [x] 5.1 `cargo test --workspace` 跑完 0 failure — 對齊 spec § Skill Bundle Layout 所有 scenarios 與 既有 vault_init / cli_routing / skill_bundle tests 全綠不破。**驗證**：test runner summary `0 failed` for codebus-core + codebus-cli + codebus-app-tauri 三 package。
- [x] 5.2 [P] `cargo build --workspace` 通過 — 無 warnings 上 fatal、`InitOptions` 新欄位之 default、`write_bundles_if_missing` 簽名變更不破 既有 import / callers。**驗證**：build log exit 0、無 `error[` 行。
- [x] 5.3 [P] `spectra validate v3-skill-bundles-vault-only` 與 `spectra analyze v3-skill-bundles-vault-only --json` 輸出 0 Critical / 0 Warning finding — 兌現 spec MODIFIED scenarios 全部與 proposal § What Changes 對齊。**驗證**：兩 command exit 0、analyze JSON `findings` array 內 severity 為 Critical / Warning 的 count 為 0。
