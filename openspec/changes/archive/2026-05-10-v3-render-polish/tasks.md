## 1. Render module 骨架與 Banner enum

- [x] 1.1 [P] 在 `codebus-core/src/render/options.rs` 實作 `RenderOptions` struct（`use_emoji` / `use_color` / `use_hyperlinks` / `vault_id`）與三個建構器 `RenderOptions::detect()` / `detect_with_vault_id(Option<String>)` / `no_styling()`，遵守 Environment-Aware Output Styling 契約：`use_emoji = std::io::IsTerminal`、`use_color = use_emoji && !env!("NO_COLOR").is_some()`、`use_hyperlinks = use_color && supports_hyperlinks::on(stdout)`；遵守「RenderOptions 普通 struct，靜態初始化一次」決定（無 trait、無 factory） — 由 `options::tests::detect_in_isolated_env_returns_expected_flags` / `no_styling_returns_all_false` unit test 驗證
- [x] 1.2 [P] 在 `codebus-core/src/render/banner.rs` 實作 `Banner<'a>` enum 共 10 變體（Start / Goal / SyncStart / SyncDone / PiiSummary / LintStart / LintDone / CommitDone / Done / Hint）與 `format_banner(banner, &opts) -> String` / `print_banner(banner, &opts)`，遵守「Banner 採 enum + free function，不採 trait」決定與 Banner Output for Verb Commands 契約 — 由 `banner::tests::format_each_variant_emoji_on` / `format_each_variant_emoji_off` unit tests 驗證每個變體 byte-equal expected string（emoji vs symbol fallback）
- [x] 1.3 在 `codebus-core/src/render/mod.rs` 把 `banner` / `options` / `lint_text` submodule 串起來、re-export `Banner` / `RenderOptions` / `format_banner` / `print_banner` / `format_lint_text`；在 `codebus-core/src/lib.rs` `pub mod render;`；`cargo check -p codebus-core` 通過
- [x] 1.4 在 `codebus-core/Cargo.toml` 加 `supports-hyperlinks` dependency；確認 `cargo build -p codebus-core` 拉得到

## 2. lookup_vault_id port + render::lint_text

- [x] 2.1 在 `codebus-core/src/vault/obsidian_register.rs` 新增 `pub fn lookup_vault_id(wiki_path: &Path) -> io::Result<Option<String>>`，遵守「`lookup_vault_id` port 自 v2」決定：讀 obsidian.json、normalize abs path 比對、回傳 vault id；找不到回 `Ok(None)`、parse fail 回 `Err`；reference impl 在 `legacy/v2-rust/codebus-core/src/obsidian/registry.rs` — 由 `obsidian_register::tests::lookup_returns_none_when_obsidian_missing` / `lookup_returns_some_when_path_in_vaults` / `lookup_returns_none_when_path_not_in_vaults` unit tests 驗證
- [x] 2.2 在 `codebus-core/src/render/lint_text.rs` 實作 `format_lint_text(result, &opts, wiki_root) -> String`，遵守「lint text 重構：分離 format 與 styling」決定與 Lint Output Formats 契約：emoji header 切換（`✅`/`ok`、`🔍`/`#`）、issue lead 切換（`✗`/`x`、`⚠`/`!`）、ANSI color 包 `error:`（紅）/ `warn: `（黃）、OSC 8 包 `wiki/<rel-path>` URL 為 `obsidian://open?vault=<percent-encoded>&file=<percent-encoded>` — 由 `lint_text::tests::clean_emoji_on` / `error_with_color` / `osc8_wraps_path_when_vault_id_some` / `osc8_omitted_when_vault_id_none` / `url_encodes_spaces_in_vault_id_and_path` unit tests 驗證
- [x] 2.3 在 `codebus-core/src/wiki/lint/output.rs` 新增 `pub fn format_text_with_opts(result, &opts, wiki_root) -> String`，內部 delegate 至 `crate::render::lint_text::format_lint_text`；既有 `format_text(result)` 簽章不破壞性改、內部呼叫 `format_text_with_opts(..., RenderOptions::no_styling(), Path::new(""))` 確保 byte-equal 既有測試 — 由既有 `format_text` unit tests 仍綠 + 新增 `output::tests::with_opts_emoji_path` 驗證

## 3. CLI 入口接入 RenderOptions

- [x] 3.1 在 `codebus-cli/src/main.rs` 進入 dispatch 前一次呼叫 `RenderOptions::detect()` 並傳給每個 verb command（簽章加 `render_opts: &RenderOptions`），遵守「Detection runs once per process」scenario — 由 `cli_routing::tests` 既有 startup tests 仍綠驗證（不破壞 entry behavior）

## 4. init 改用 banner 序列

- [x] 4.1 重寫 `codebus-cli/src/commands/init.rs::run`：default 模式按順序呼叫 `print_banner(Banner::Start{...}, &opts)`、執行 raw_sync 量 `elapsed_ms` 後呼叫 `Banner::SyncDone`、根據 `summary.pii_matches` / `pii_skipped_files` / `pii_masked_matches` 與 scanner 名稱呼叫 `Banner::PiiSummary`、auto_commit 後呼叫 `Banner::CommitDone`、結尾呼叫 `Banner::Done`；obsidian register 成功時加 `Banner::Hint`；既有 11 行 `✓ <step>` lines 全用 `if debug { println!("✓ ...") }` 包起來，遵守「Init progress 的 11 → 5 行轉換」與「debug mode 共存契約」決定，遵守 Init Subcommand Behavior MODIFIED 與 Debug Flag Output requirement — 由 `cli_routing::tests::init_default_mode_emits_banners_only` / `init_debug_mode_emits_banners_and_progress_lines` integration test 驗證

## 5. goal / query / fix / lint 接 banner

- [x] 5.1 [P] 修改 `codebus-cli/src/commands/goal.rs`：開頭印 `Banner::Start` + `Banner::Goal`、re-sync 時印 `Banner::SyncStart` + `Banner::SyncDone`、fix phase 入口印 `Banner::LintStart` + `Banner::LintDone`、commit 後印 `Banner::CommitDone` + `Banner::Done`；既有 `[debug]` lines 與 `✓ raw mirror: ...` 細節行用 debug guard 包 — 由 `goal_flow::tests::goal_default_emits_start_goal_done_banners` integration test 驗證
- [x] 5.2 [P] 修改 `codebus-cli/src/commands/query.rs`：開頭印 `Banner::Start`，agent 結束無 commit 直接收尾（query 無 `Banner::Done` — 因為沒寫 wiki，避免「下車」訊息誤導；可選輕量 `Banner::Hint` 提示 wiki 位置）— 由 `query_flow::tests::query_default_emits_start_banner` integration test 驗證
- [x] 5.3 [P] 修改 `codebus-cli/src/commands/fix.rs`：開頭印 `Banner::Start`、agent 跑後印 `Banner::LintStart` + `Banner::LintDone` 反映 final lint state、commit 後印 `Banner::CommitDone` — 由 `fix_flow::tests::fix_default_emits_lint_done_banner` integration test 驗證
- [x] 5.4 修改 `codebus-cli/src/commands/lint.rs`：構造 `RenderOptions::detect_with_vault_id(lookup_vault_id(&paths.wiki).unwrap_or(None))` 後 call `format_text_with_opts`；JSON format 不變仍走 `format_json`；非 TTY 時 `RenderOptions.use_emoji=false` → text 走 ASCII fallback — 由 `lint_flow::tests::lint_text_emoji_on_in_tty` / `lint_text_ascii_when_redirected` integration test 驗證

## 6. 整合 + 環境契約測試

- [x] 6.1 [P] 在 `cli_routing.rs` 新增 `init_no_color_disables_ansi_keeps_emoji` test：`NO_COLOR=1` env 下 init 輸出含 `🚌` 但不含任何 `\x1b[`；`init_pipe_disables_emoji_and_color` test：stdout 重新導向到檔案時，輸出不含 `🚌` / `🎉` / `\x1b[` — 遵守 Environment-Aware Output Styling 契約
- [x] 6.2 [P] 在 `lint_flow.rs` 新增 `lint_text_with_vault_id_emits_osc8` test：先 init（registers vault）→ 製造 lint issue → 跑 `codebus lint`（TTY 模式 mock 透過 `IsTerminal` 不易，用 `format_text_with_opts` 直呼覆蓋或 force flag）→ 輸出含 OSC 8 escape；`lint_text_no_vault_id_omits_osc8` test：跑 init `--no-obsidian-register` → lint → 不含 OSC 8 escape — 遵守 Lint Output Formats MODIFIED scenarios
- [x] 6.3 在 `cli_routing.rs` 新增 `emoji_flag_rejected_by_clap` test：`codebus init --emoji on` 應回非零 exit；`no_emoji_env_silently_ignored` test：`NO_EMOJI=1` 下仍輸出 emoji（前提：TTY） — 遵守 Environment-Aware Output Styling 兩條 scenario
- [x] 6.4 跑 `cargo test --workspace` 全綠；確認 v3-config / v3-fix-trust-agent / 既有 vault_init / goal_flow / query_flow / fix_flow / lint_flow 整合測試在 banner 替換後仍通過（必要時更新斷言期待從「`✓ vault layout:`」改為「`🚌`」）；跑 `spectra validate v3-render-polish` 與 `spectra analyze v3-render-polish --json` 確認無 Critical / Warning finding
