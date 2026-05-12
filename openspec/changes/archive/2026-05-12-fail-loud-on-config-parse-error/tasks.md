## 1. Helper 改 fail-loud

- [x] 1.1 修改 `codebus-cli/src/commands/goal.rs` 內的 `load_pii_config_with_warning` 與 `load_claude_code_config_with_warning`，以及 `run` 函式內 inline `load_lint_fix_config` 處理：落實 **Config Parse Failure Aborts Invocation** requirement — 當 loader 回傳 `Err` 時 stderr 印「section 名稱 + 錯誤明細」、`return ExitCode::from(<非零碼>)`，**不** fallback 到 `Default::default()`。Behavior：`codebus goal` 在 yaml 解析錯誤時 exit 非零、stderr 含 section + 錯誤位置、無 claude 子 process spawn、無 wiki 寫入。Verification：integration test 涵蓋 goal 路徑（見 task 3.1）綠 + 既有 `goal_flow` tests 維持綠。
- [x] 1.2 修改 `codebus-cli/src/commands/query.rs` 的 `load_claude_code_config_with_warning`：同樣 fail-loud pattern。Behavior：`codebus query` 在解析錯誤時 exit 非零、stderr 含錯誤、無 claude 子 process spawn。Verification：integration test 涵蓋 query 路徑（見 task 3.1）綠 + 既有 `query_flow` tests 維持綠。
- [x] 1.3 修改 `codebus-cli/src/commands/fix.rs` 的 inline `load_claude_code_config` 與 `load_lint_fix_config` 處理：同樣 fail-loud pattern。Behavior：`codebus fix` 在解析錯誤時 exit 非零、無 claude 子 process spawn、無 auto-commit。Verification：integration test 涵蓋 fix 路徑（見 task 3.1）綠 + 既有 `fix_flow` tests 維持綠。
- [x] 1.4 修改 `codebus-cli/src/run_log.rs` 的 `load_log_config_with_warning`：同樣 fail-loud pattern。Behavior：`log:` section 解析失敗時 caller 取得 error 並 exit 非零、不寫入 jsonl run-log。Verification：integration test 在 `log:` section 製造解析錯誤後跑 goal/query/fix，確認三者都 fail-loud（屬於 task 6.1 一部分）。

## 2. `config` 子命令 fail-loud

- [x] 2.1 修改 `codebus-cli/src/commands/config.rs` 的 `read_azure_keyring_service_from_config`：把現行 `load_claude_code_config(&path).ok()?` 改成顯式 match — 檔案不存在 → `Ok(None)`（caller 用 default service）；`Err(ConfigLoadError::YamlParse | ...)` → 回傳 `Err(ExitCode)` 並 stderr 印錯誤；`Ok(cfg)` 取 `cfg.azure.as_ref().map(...)`。`run_set_key` / `run_get_key` / `run_delete_key` 各自 propagate error。Behavior：`codebus config set-key azure` / `get-key azure` / `delete-key azure` 在 yaml 解析錯誤時 exit 非零、stderr 含 `claude_code` 區塊解析錯誤明細、keyring 完全沒被觸碰（無 `Entry::set_password` / `delete_credential` / `get_password` 呼叫）。Verification：integration test `parse_error_aborts_all_verbs` 中 config 三動作的 subtest 綠。

## 3. Integration test

- [x] 3.1 新增 `codebus-cli/tests/parse_error_aborts_all_verbs.rs`，涵蓋：(a) yaml 語法錯誤（如 `pii` key 漏冒號）→ `goal` / `query` / `fix` / `config delete-key azure` 全 exit 非零；(b) `claude_code.system.goal.model: gpt-4` schema 違反 → 四個 verb 全 exit 非零；(c) 對 (a) 的場景斷言 mock-claude **沒** 被 spawn（CODEBUS_CLAUDE_BIN 指向 mock-claude + mock log file 不應存在）；(d) 對 (a) 的 `config delete-key azure` 場景，預先用 unique service name 寫入 keyring 一條 entry，跑 delete-key 後該 entry 仍存在；(e) 反向 sanity：合法 yaml + 未知 key 跑 `goal` 不會 fail-loud。Behavior：四個 verb 的 fail-loud 行為被 end-to-end 驗證。Verification：`cargo test -p codebus-cli --test parse_error_aborts_all_verbs` 全綠。

## 4. 回歸驗證

- [x] 4.1 跑 `cargo test -p codebus-core -p codebus-cli` 全套，斷言既有 `goal_flow` / `query_flow` / `fix_flow` / `config_subcommand` / `cli_routing` / `scoped_env_injection` / `azure_key_pre_spawn` / `endpoint_config_load` 全部維持綠（fail-loud 改動 SHALL NOT 影響合法 yaml 的既有行為）。Behavior：所有既有 integration tests 不需要修改即通過。Verification：`cargo test -p codebus-core -p codebus-cli 2>&1 | grep -E "^test result|FAILED"` 只看到 `ok` 行、無 `FAILED`。
