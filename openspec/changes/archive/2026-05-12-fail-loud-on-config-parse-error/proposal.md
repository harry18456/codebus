## Problem

`codebus` 的所有 verb command（`goal` / `query` / `fix` / `config`）目前在載入 `~/.codebus/config.yaml` 時走「warn-and-fallback」pattern：當 YAML 解析失敗或 schema 驗證失敗（`load_*_config` 回傳 `Err`），helper 函式 stderr 印一行警告後**靜默 fallback 到 `Default::default()`**，invocation 繼續執行。

這個 pattern 在 `claude-code-endpoint-profiles` change 期間造成實際 user 損失：`config_subcommand` 整合測試的 yaml fixture 用了未版本化的 `haiku` / `sonnet`，新 schema 升版後拒收 → 測試呼叫 `codebus config delete-key azure` 時，`read_azure_keyring_service_from_config` 走 `.ok()?` 把 `Err` 轉成 `None` → fallback 到 default `codebus-azure` service → 刪掉了 user 真正寫入 keyring 的 key（即使 yaml 裡明確設了不同的 `keyring_service`）。

同樣 anti-pattern 也存在於 `goal` / `query` / `fix` 的 `load_pii_config_with_warning` / `load_claude_code_config_with_warning` / `load_lint_fix_config` fallback、`codebus-cli/src/run_log.rs::load_log_config_with_warning`。雖然非 keyring section 不會誤刪資料，但 user 寫的設定被靜默忽略仍是 silent footgun。

## Root Cause

`codebus-cli/src/commands/{goal,query,fix}.rs` 與 `codebus-cli/src/commands/config.rs` 與 `codebus-cli/src/run_log.rs` 內的 config 載入 helper 把「parse error」與「合理 fallback 情境（檔案不存在、section 缺漏、未知 key）」視為同一類，全部走 `Default::default()`。`config.rs` 內 `read_azure_keyring_service_from_config()` 還更激進地用 `.ok()?` 連 stderr 警告都不印就 fallback。

設計初衷是 forward-compat：未來新增 yaml key 不應該 break 舊版 binary。但 parse error 不是 forward-compat 場景——是 user 真的寫錯，需要 user 知道並修正，不該被靜默吞掉。

## Proposed Solution

把 config 載入失敗的三種情境分開處理：

| 情境 | 新行為 |
|---|---|
| Config 檔不存在（`io::ErrorKind::NotFound`）| 用 default（不變；first-time setup 友善）|
| Config 載入成功（含「section 缺漏」、「未知 key 被忽略」）| 用 parsed config（不變）|
| **Config 檔存在但解析失敗**（`ConfigLoadError::YamlParse` 或其他 `Err`）| **stderr 印出錯誤位置 + exit code 非零；不啟動任何後續動作** |

實作方式：在所有 `load_*_config_with_warning` helper 內，把 `Err(_) => Default::default()` 分支改成「印錯誤 + 回傳 `Result<T, ExitCode>` 給 caller，由 caller `return ExitCode::from(...)`」。`read_azure_keyring_service_from_config` 同樣處理。

新增一個共用的「config gate」函式語義（每個 verb 入口都呼叫）讓 fail-loud 行為集中、易於 audit。

## Non-Goals

- **不**新增「config 修復」自動工具（如 `codebus config validate` / `codebus config doctor`）；本次只改 fail-loud 行為，diagnostic UX 由 user 自己對照 stderr 提示修 yaml
- **不**改 schema 結構或 default value（保留 `claude-code-endpoint-profiles` 拍板的 profile schema + versioned `SystemModel` enum）
- **不**改 `init` 子命令的 starter writer 行為（init 不讀 config）
- **不**處理 keyring backend 不可用以外的執行期錯誤（那是 spawn / runtime 範疇）

## Success Criteria

- 給定一個 yaml 語法錯誤的 `~/.codebus/config.yaml`，跑 `codebus goal "X"` / `codebus query "X"` / `codebus fix` / `codebus config delete-key azure` 全部 exit 非零，stderr 含解析錯誤位置（行號或 field 名稱），**沒有任何 keyring 寫入 / 刪除動作發生、沒有 claude 子 process 被 spawn、沒有 wiki 檔被改寫**
- 給定 schema 驗證失敗的 yaml（例如 `claude_code.system.goal.model: gpt-4`），同上行為
- 給定不存在的 config 檔，所有 verb 行為**不變**（用 default 繼續跑）
- 給定合法 yaml 加上未知 key（forward-compat 場景），所有 verb 行為**不變**
- 既有 integration tests（goal_flow / query_flow / fix_flow / config_subcommand / cli_routing / scoped_env_injection / azure_key_pre_spawn）維持綠
- 新增 integration test `parse_error_aborts_all_verbs` 驗證上述四個 verb 都 fail-loud

## Impact

- 影響 spec：modify `cli` capability（新增 requirement「Config Parse Failure Aborts Invocation」）。
- 影響程式碼：
  - 修改：
    - codebus-cli/src/commands/goal.rs（`load_pii_config_with_warning` / `load_claude_code_config_with_warning` 改 fail-loud；inline `load_lint_fix_config` 處理改 fail-loud）
    - codebus-cli/src/commands/query.rs（`load_claude_code_config_with_warning` 改 fail-loud）
    - codebus-cli/src/commands/fix.rs（inline `load_claude_code_config` + `load_lint_fix_config` 處理改 fail-loud）
    - codebus-cli/src/commands/config.rs（`read_azure_keyring_service_from_config` 改 fail-loud：parse error 時回傳 `Err(ExitCode)`，三個 sub-action handler 各自 propagate）
    - codebus-cli/src/run_log.rs（`load_log_config_with_warning` 改 fail-loud）
  - 新增：
    - codebus-cli/tests/parse_error_aborts_all_verbs.rs（integration test 涵蓋四個 verb 的 fail-loud 行為）
  - 刪除：無
- 影響使用者：行為變得更嚴格——之前會「警告後用 default」的 invocation，現在會直接 fail。屬於 bugfix 範疇（防止資料破壞）。已知 user 只有 harry 自己，遷移成本零。
