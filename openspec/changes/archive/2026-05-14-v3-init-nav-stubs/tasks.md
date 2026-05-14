<!--
每個 task：(1) 交付的行為或契約 + (2) 完成驗證目標。
File path 僅供 locator context；task 描述須含 spec requirement name 與行為的 substring。
Locale zh-tw；symbol / 路徑 / requirement 名稱保英文。
[P] 標示與群組內其他 [P] 不共享 file 且互不依賴，可平行執行。
本 tasks.md 覆蓋 proposal § What Changes 範圍與 spec MODIFIED 全部 scenarios。
-->

## 1. Nav stubs module

- [x] 1.1 新增 `codebus-core/src/vault/nav_stubs.rs` 定義 `pub fn write_nav_stubs_if_missing(vault_root: &Path, today_utc: &str) -> io::Result<(usize, usize)>` 與 helper `nav_stub_content(name: &str, today_utc: &str) -> String` — 落實 spec § Vault Layout「Init materializes both nav files at the wiki root」+「Nav placeholder body contains no wikilink syntax」+「Nav write-if-missing preserves existing files」normative；function 對 `wiki/index.md` 與 `wiki/log.md` 各自獨立判斷 missing，missing 時寫入 frontmatter（title / type=`synthesis` / sources=`[]` / goals=`[]` / created=`today_utc` / updated=`today_utc` / related=`[]` / stale=`false`）+ 一行 placeholder body（不含 `[[` 或 `]]` 字元）；回傳 `(written_count, preserved_count)` 兩個 usize；vault_root/wiki/ parent 不存在時 `fs::create_dir_all` 補建。**驗證**：unit test `nav_stub_content_index_has_required_frontmatter_keys`（call helper、assert body starts with `---\n` + 含 `title:` / `type: synthesis` / `sources:` / `goals:` / `created:` / `updated:` / `related:` / `stale:` 全 8 keys + 一行 placeholder text）；test `nav_stub_content_body_has_no_wikilink_syntax`（call helper for both `index` 跟 `log`、assert returned string 不含 `[[` 與 `]]` substring）；test `write_nav_stubs_first_run_writes_both`（temp vault root、call write_nav_stubs_if_missing、assert outcomes==(2, 0) + 兩 file exist）；test `write_nav_stubs_preserves_existing_index`（pre-seed `wiki/index.md` 含 custom content、call write_nav_stubs_if_missing、assert outcomes==(1, 1)、index bytes 完全未變、log.md 新建存在）。
- [x] 1.2 [P] 將 `nav_stubs` 加入 `codebus-core/src/vault/mod.rs` `pub mod` 列表 — 讓 `vault::init::run_init` 與外部 caller 可呼叫；module 同 `vault/skill_bundle` 等 sibling 公開。**驗證**：`cargo build -p codebus-core` 通過 + `use codebus_core::vault::nav_stubs;` 在 integration test 內可解析（task 2.1 / 2.2 同 integration test 內 import 驗證）。

## 2. Wire 進 run_init

- [x] 2.1 `codebus-core/src/vault/init.rs` 加 `InitEvent::NavStubsDone { vault_root: &'a Path, written: usize, preserved: usize }` 新 variant；既有 callers（CLI init handler、verb::goal 自動 init 路徑）對 InitEvent match 為 non-exhaustive 或顯式列舉 — 加新 case 編譯不破壞既有 callers。**驗證**：`cargo build --workspace` 通過、無 `non-exhaustive` warning 變 error 的 callsite。
- [x] 2.2 `codebus-core/src/vault/init.rs` `run_init` body 內：(a) 算 `today_utc = chrono::Utc::now().format("%Y-%m-%d").to_string()`；(b) 在 `write_skill_bundles` 步驟之後、`write_settings_if_missing` 之前 call `nav_stubs::write_nav_stubs_if_missing(&paths.root, &today_utc)` map_err 為 `InitError::Layout`（或新加 `InitError::NavStubs` 同 pattern as SkillBundles，視 既有 enum 擴充慣例選一致路徑）；(c) emit `InitEvent::NavStubsDone { vault_root: &paths.root, written, preserved }` — 落實 spec § Vault Layout「Init materializes both nav files at the wiki root」normative + 「Re-running init leaves nav files untouched」idempotency。**驗證**：integration test `run_init_writes_both_nav_stubs_on_fresh_vault`（fresh repo + `InitOptions::default()`、跑 run_init、assert `<vault>/wiki/index.md` 與 `<vault>/wiki/log.md` 兩檔存在、各 starts with `---\n`、各含 `type: synthesis`）；test `re_init_preserves_existing_nav_index`（先跑 run_init、改寫 index.md 加自訂行、第二次跑 run_init、assert index.md bytes 與改寫後一致 + log.md 也未變）；test `lint_on_freshly_inited_vault_reports_no_nav_missing`（run_init 後對 vault 跑 lint factory 全 rules、assert 結果 `LintIssue` array 內無 `rule_id == "nav-missing"`）。

## 3. CLI handler 更新

- [x] 3.1 [P] `codebus-cli/src/commands/init.rs` `handle_event` 內加 `InitEvent::NavStubsDone { written, preserved, .. }` 分支 — debug 模式時印 `[debug] nav stubs: written={written}, preserved={preserved}`（同 既有 SkillBundlesDone debug 行格式）；non-debug 模式不額外輸出（避免汙染 banner sequence）。**驗證**：cli_routing test `init_emits_nav_stubs_done_event_in_debug_mode`（spawn `codebus --debug init <tmp>`、assert stderr 含 `[debug] nav stubs:` substring + `written=2` substring on fresh vault）；既有 bare invocation banner tests（既有 `bare_invocation_routes_to_init_handler_*` 等）仍綠不破。

## 4. Integration verification

- [x] 4.1 [P] `codebus-cli/tests/cli_routing.rs` 新增 end-to-end test `bare_init_creates_nav_stubs_so_subsequent_lint_is_clean`（spawn `codebus init <tmp>` 預設 flag、assert `<tmp>/.codebus/wiki/index.md` 與 `<tmp>/.codebus/wiki/log.md` 存在、再 spawn `codebus lint --repo <tmp> --format json`、parse stdout JSON、assert `issues` array 內無 `rule_id == "nav-missing"`、`error_count` + `warn_count` 為 0）— 落實 spec § Vault Layout「Lint on a freshly-inited vault does not report nav-missing」end-to-end。

## 5. Verification 收尾

- [x] 5.1 `cargo test --workspace` 跑完 0 failure — 對齊 spec § Vault Layout 所有 scenarios 與既有 vault_init / cli_routing / skill_bundle / lint_flow tests 全綠不破。**驗證**：test runner summary `0 failed` for codebus-core + codebus-cli + codebus-app-tauri 三 package。
- [x] 5.2 [P] `cargo build --workspace` 通過 — 無 fatal error；新增 `InitEvent::NavStubsDone` variant 不破既有 callers（match 範圍未覆蓋警告若有 應於 task 2.1 對應 handler 加分支已修）。**驗證**：build log exit 0。
- [x] 5.3 [P] `spectra validate v3-init-nav-stubs` 與 `spectra analyze v3-init-nav-stubs --json` 輸出 0 Critical / 0 Warning finding。**驗證**：兩 command exit 0、analyze JSON `findings` array 內 severity 為 Critical / Warning 的 count 為 0。
