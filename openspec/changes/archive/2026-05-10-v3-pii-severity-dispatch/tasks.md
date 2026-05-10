## 1. Config 層 — default 改 Warn

- [x] 1.1 在 `codebus-core/src/config/pii.rs::PiiConfig::default()` 把 `on_hit` 從 `OnHit::Mask` 改回 `OnHit::Warn`，遵守 On-Hit Policy Default MODIFIED 段；既有 unit test `default_when_file_missing` / `default_when_pii_section_absent` / `partial_config_fills_missing_fields_with_defaults` 對 default 值的 assertion 同步從 `OnHit::Mask` 改 `OnHit::Warn` — 由 `cargo test -p codebus-core --lib config::pii::tests` 全綠驗證
- [x] 1.2 在 `codebus-core/src/config/global_starter.rs::STARTER_CONFIG` 把 `on_hit: mask` 改 `on_hit: warn`，註解更新說明「`on_hit` 只控 Warn-severity；Critical-severity（AWS / Anthropic key）永遠 mask 不可關」；`starter_round_trips_to_defaults` test 仍綠（PiiConfig::default() 的 on_hit 已改 Warn 同步）

## 2. Raw Sync — Critical 永遠 mask

- [x] 2.1 在 `codebus-core/src/vault/raw_sync.rs::sync_with_scanner_into` 把 match-handling block 改成兩段：(a) 先把 matches 切成 `critical_matches` 和 `warn_matches`；(b) 對 critical_matches **永遠** apply mask（即使 `on_hit` 是 Warn / Skip）— 用 `mask_matches` helper 處理；(c) 對 warn_matches 依 `on_hit` 走原本 Warn / Skip / Mask 分支。當 critical_matches 非空時，destination 永遠寫 masked 內容（不管 on_hit）；warn-only file 才依 `on_hit` 決定 — 遵守 Mirror Skip Behavior MODIFIED 與 Mirror Mask Behavior MODIFIED 契約
- [x] 2.2 在 `codebus-core/src/vault/raw_sync.rs` `#[cfg(test)]` 區新增 4 條 unit test：(a) `critical_match_under_warn_policy_is_masked` — file 含 1 條 AWS key + on_hit=Warn → 檔案被 mask 寫入（不只 warn line）；(b) `critical_match_under_skip_policy_is_masked_not_skipped` — file 含 AWS key + on_hit=Skip → 檔案 mirror 含 `[REDACTED:aws-access-key]`、不被 skip；(c) `mixed_severity_under_warn_policy_only_critical_masked` — file 含 AWS key + email + on_hit=Warn → 檔案 mirror 中 AWS key 被 mask、email 保留原文、warn sink 兩條 line 都印；(d) `warn_only_file_under_warn_policy_unchanged` — file 含 email + on_hit=Warn → 檔案 byte-identical 復制、warn line 印

## 3. Init Banner — PiiSummary action 兩段式

- [x] 3.1 在 `codebus-cli/src/commands/init.rs::on_hit_label` 改成回傳格式 `critical=<X>, warn=<Y>` 字串（X 永遠 `mask`、Y 是 user 設的 OnHit lowercase 名）；`Banner::PiiSummary` 的 `action` field 接收這個格式 — 遵守 Banner Output for Verb Commands MODIFIED 段「PiiSummary action 兩段式格式」契約
- [x] 3.2 在 `codebus-cli/tests/cli_routing.rs` 既有 `init_progress_line_includes_zero_pii_count_for_clean_repo` / `init_progress_line_reports_nonzero_pii_count_for_repo_with_secrets` / `init_with_null_scanner_config_skips_pii_warnings` / `init_with_bad_patterns_extra_falls_back_to_builtin` 4 條 test 中，PII summary line 的 `action` 字串斷言改成期待 `critical=mask, warn=warn`（default）或 `critical=mask, warn=mask`（user override）— 確保 banner 格式變動已 propagate

## 4. 整合驗證

- [x] 4.1 跑 `cargo test --workspace` 全綠
- [x] 4.2 跑 `spectra validate v3-pii-severity-dispatch` + `spectra analyze --json` 無 Critical / Warning finding
- [x] 4.3 release build + 手動 CLI 驗證 — 用 `D:/side_project/uv` 跑：(a) 清舊 vault 後 init，PiiSummary banner 含 `action critical=mask, warn=warn`；(b) raw mirror 內 `CONTRIBUTING.md` 仍含原文 `127.0.0.1`（不再 redacted）；(c) 植入 `AKIAIOSFODNN7EXAMPLE` 到一個 source 檔，再 init，raw mirror 該檔含 `[REDACTED:aws-access-key]`（強制 mask）；(d) 設 `on_hit: mask` 到 `~/.codebus/config.yaml`，重 init，PiiSummary 變 `critical=mask, warn=mask`、CONTRIBUTING.md raw mirror 變 redacted（user 仍可 opt-in 全 mask）— 寫進 `docs/v3-uv-verification-2026-05-10.md` 附錄
