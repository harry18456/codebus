## 1. PII Configuration Schema 載入

- [x] 1.1 [P] 在 `codebus-core/src/config/pii.rs` 實作 `PiiConfig` struct（含 `scanner: PiiScannerKind`、`patterns_extra: Vec<String>`、`on_hit: OnHit`）與 `Default` impl，使空輸入回 `{ regex_basic, [], mask }`，遵守「`OnHit::Mask` default」契約 — 由 `pii::config::tests::default_when_file_missing` 與 `default_when_pii_section_absent` unit test 驗證
- [x] 1.2 [P] 在同檔實作 `load_pii_config(path)` loader，遵守 PII Configuration Schema 與「Tolerance 範圍：5 條（不做 type-mismatch）」決定 — missing file / missing section / missing field 走 default、unknown key 靜默忽略、unknown discriminator parse fail；type-mismatch 由 serde_yaml 預設 parse fail 處理；由 `partial_config_fills_missing_fields_with_defaults` / `unknown_pii_subkey_silently_ignored` / `unknown_on_hit_value_returns_parse_err` unit test 驗證
- [x] 1.3 在 `codebus-core/src/config/mod.rs` re-export `PiiConfig` / `PiiScannerKind` / `load_pii_config`；`cargo check -p codebus-core` 通過且 `mod.rs` 內含 `pub use pii::*`

## 2. Claude Code Configuration Schema 載入

- [x] 2.1 [P] 在 `codebus-core/src/config/claude_code.rs` 實作 `ClaudeCodeConfig` / `VerbAgentConfig` struct 與 `Default` impl，使空輸入回 `{ goal: {opus, high}, query: {haiku, low}, fix: {sonnet, medium} }`，遵守 Claude Code Configuration Schema 與「Per-verb claude_code config（非全域單組）」決定 — 三 verb 算力差異反映在 default 上；`model` / `effort` 為 `Option<String>` 放行任意字串、不硬列舉，遵守「`model` / `effort` 字串放行不硬列舉」契約 — 由 `default_when_file_missing` 與 `arbitrary_model_string_is_accepted` unit test 驗證
- [x] 2.2 [P] 在同檔實作 `load_claude_code_config(path)` loader，per-verb override 應只影響該 verb 不污染其他 default；由 `per_verb_override_applies_only_to_that_verb` 與 `unknown_subkey_silently_ignored` unit test 驗證
- [x] 2.3 在 `codebus-core/src/config/mod.rs` re-export `ClaudeCodeConfig` / `VerbAgentConfig` / `load_claude_code_config`；`cargo check -p codebus-core` 通過

## 3. Global Config Starter Writer

- [x] 3.1 [P] 在 `codebus-core/src/config/global_starter.rs` 實作 `write_starter_config_if_missing(path) -> io::Result<StarterOutcome>`，遵守「Starter writer primitive 在 core，orchestration 在 cli」契約：path.exists 短路回 `AlreadyPresent`、parent 不存在則 `create_dir_all`、寫入硬編碼 starter（含 pii / claude_code / lint.fix 全部 default 加 inline comment） — 由 `writes_when_missing` / `noop_when_present` / `creates_parent_dir` unit test 驗證
- [x] 3.2 starter 字串輸出 round-trip 過 `load_pii_config` 與 `load_claude_code_config` 後皆等於 `Default::default()`，由 `starter_round_trips_to_defaults` unit test 驗證
- [x] 3.3 在 `codebus-core/src/config/mod.rs` re-export `StarterOutcome` / `write_starter_config_if_missing`

## 4. Raw Sync OnHit 行為實作

- [x] 4.1 在 `codebus-core/src/vault/raw_sync.rs` 把 `sync_with_scanner_into` signature 加上 `on_hit: OnHit` 參數（破壞性改動）；同步更新 `sync_with_scanner` thin wrapper（內部硬填某 default 即可，由 caller 顯式選擇）；`SyncSummary` 新增 `pii_skipped_files: usize` 與 `pii_masked_matches: usize` 欄位，遵守「`OnHit::{Skip, Mask}` 由 raw_sync 接 OnHit 參數實作」契約 — 由 `cargo check -p codebus-core` + 既有測試以 `OnHit::Warn` 顯式傳入後仍通過驗證
- [x] 4.2 實作 `OnHit::Warn` 分支保留現行行為（`fs::copy` + warn line），遵守 On-Hit Policy Default 契約 — 由 `codebus-core/tests/raw_sync_onhit.rs::warn_mode_copies_file_and_emits_warn` integration test 驗證
- [x] 4.3 實作 `OnHit::Skip` 分支：scan 結果非空時不執行 `fs::copy` 但仍寫 warn line、`pii_skipped_files += 1`，遵守 Mirror Skip Behavior 契約 — 由 `codebus-core/tests/raw_sync_onhit.rs::skip_mode_omits_matched_file` 與 `skip_mode_records_skipped_count` integration test 驗證
- [x] 4.4 實作 `OnHit::Mask` 分支：UTF-8 內容由後往前替換 `matched_text` 為 `[REDACTED:<pattern_name>]`、寫 `fs::write`、warn line 仍寫、`pii_masked_matches += matches.len()`，遵守 Mirror Mask Behavior 契約 — 由 `mask_mode_replaces_single_match` / `mask_mode_replaces_multiple_matches_in_descending_order` / `mask_mode_falls_through_to_copy_for_non_utf8` / `mask_mode_records_masked_count` integration test 驗證

## 5. claude_cli invoke 接 model / effort

- [x] 5.1 在 `codebus-core/src/agent/claude_cli.rs` 為 `InvokeAgentOptions` 加 `model: Option<String>` / `effort: Option<String>` 欄位；`invoke()` 在 `Some` 時 append `--model <X>` / `--effort <Y>` 到 argv、`None` 時不加，遵守 Agent Spawn Model and Effort Forwarding 契約 — 由 `claude_cli::tests::invoke_appends_model_and_effort_when_some` 與 `invoke_omits_flags_when_none` unit test 驗證（透過 `CODEBUS_CLAUDE_BIN` 指 echo wrapper 攔截 argv）

## 6. CLI Verb 串接 PII + claude_code config

- [x] 6.1 修改 `codebus-cli/src/commands/init.rs` 使 raw_sync 階段先 `load_pii_config(default_config_path()?)`、依 `PiiConfig.scanner` 走 dispatch 建構 `Box<dyn PiiScanner>`、把 `on_hit` 顯式傳給 `sync_with_scanner_into`，遵守 Scanner Selection from Config 契約並驗證「`pii.scanner` 直接用既存 NullScanner 當第二 impl」決定 — 由 `codebus-cli/tests/cli_routing.rs::init_with_null_scanner_config_skips_pii_warnings` integration test 驗證（寫一份 `~/.codebus/config.yaml` 含 `pii.scanner: none` 後跑 init 應 0 警告）
- [x] 6.2 在 `init.rs` orchestration 末尾插入 global config starter write 步驟：呼叫 `write_starter_config_if_missing(default_config_path()?)`、印「✓ global config: wrote ~/.codebus/config.yaml」或「✓ global config: ~/.codebus/config.yaml already present」、寫失敗印 stderr `warning: global config` 但不 abort，遵守 Init Subcommand Behavior 新增 scenario — 由 `cli_routing.rs::init_writes_global_config_starter_when_missing` 與 `init_does_not_overwrite_existing_global_config` integration test 驗證
- [x] 6.3 修改 `codebus-cli/src/commands/goal.rs` 使其在 `InvokeAgentOptions` 建構處 `load_claude_code_config(...)` 後填入 `goal.model` / `goal.effort`；source-signal drift detection re-sync 階段同樣依 `PiiConfig` dispatch scanner — 由 `codebus-cli/tests/goal_flow.rs::goal_spawn_includes_configured_model_and_effort` integration test 驗證 spawn argv 含 `--model opus --effort high`（default）或 user override 值
- [x] 6.4 修改 `codebus-cli/src/commands/query.rs` 使其在 `InvokeAgentOptions` 建構處填入 `query.model` / `query.effort` — 由 `codebus-cli/tests/query_flow.rs::query_spawn_includes_configured_model_and_effort` integration test 驗證 spawn argv 含 `--model haiku --effort low`
- [x] 6.5 修改 `codebus-cli/src/commands/fix.rs` 與 `codebus-core/src/wiki/fix/mod.rs::run_fix_loop`（簽名加 `model: Option<String>` / `effort: Option<String>`），讓 fix 命令把 `claude_code.fix` 設定一路傳到 `InvokeAgentOptions` — 由 `codebus-cli/tests/fix_flow.rs::fix_spawn_includes_configured_model_and_effort` integration test 驗證 spawn argv 含 `--model sonnet --effort medium`
- [x] 6.6 在 PII regex 編譯失敗時，goal.rs / init.rs 須印 stderr `warning: pii config` 後降級為「無 patterns_extra」的 `RegexBasicScanner`（不 fallback NullScanner），遵守 Scanner Selection from Config 的「patterns_extra regex compile failure falls back to built-in only」scenario — 由 `cli_routing.rs::init_with_bad_patterns_extra_falls_back_to_builtin` integration test 驗證

## 7. 整合測試與 spec 對齊

- [x] 7.1 在 `codebus-cli/tests/cli_routing.rs` 新增 `init_writes_default_parseable_global_config` test：跑 init 後 `~/.codebus/config.yaml` 存在、parse 結果等於 `PiiConfig::default()` 與 `ClaudeCodeConfig::default()`
- [x] 7.2 跑 `cargo test --workspace` 全綠；確認既有 `vault_init.rs` / `goal_flow.rs` / `query_flow.rs` / `fix_flow.rs` 整合測試在 OnHit 改 default 為 Mask 後仍通過（必要時更新 fixture 預期值，遵守 Migration Plan 的 BREAKING 註解）
- [x] 7.3 跑 `spectra validate v3-config` 與 `spectra analyze v3-config --json` 確認 spec ↔ tasks ↔ design 互相 reference 一致；無 Critical / Warning finding
