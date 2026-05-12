## 1. Crate scaffolding

- [x] 1.1 在 codebus-core 與 codebus-cli 的 Cargo.toml 加 `keyring` crate 依賴；新建 stub 檔案 `codebus-core/src/config/endpoint.rs`、`codebus-core/src/config/keyring.rs`、`codebus-core/src/agent/env_overrides.rs`、`codebus-cli/src/commands/config.rs`，並透過 `mod` 宣告掛入 parent module。Behavior：crate compile 通過、新 module 可被既有 `lib.rs` 引用。Verification：`cargo build -p codebus-core -p codebus-cli` 成功且無 unused-module warning。

## 2. Endpoint Profile Schema 與 model alias

- [x] 2.1 在 `codebus-core/src/config/endpoint.rs` 實作 **Endpoint Profile Schema** 與 **System Profile Model Aliases**（落實 design.md「Profile 模式 vs discriminated union」與「system profile 的 model enum」決策）：profile 模式 schema（`active: system | azure` selector、system / azure 兩個 sibling block、active profile 必填驗證、非 active profile 可缺）、`SystemModel` 列舉（`opus-4-7` / `opus-4-6` / `haiku` / `sonnet`，serde kebab-case）、`to_cli_flag` 對照表。TDD：先寫 unit test 涵蓋合法 system / azure 解析、azure 缺 base_url 拒絕、active=system 但 system block 缺 verb 拒絕、非 active profile 缺欄位可接受、SystemModel 違法值拒絕、`to_cli_flag` 四個 variant 對應字串。Verification：`cargo test -p codebus-core --lib config::endpoint` 全綠。
- [x] 2.2 在同一 module 補上 **Azure Profile Model String Passthrough** 解析路徑（落實 design.md「azure profile 的 model 字串透傳」決策）：azure profile 的 verb model 是任意非空字串，codebus 不驗證、不翻譯、不大小寫正規化。TDD：unit test 驗證 azure mode `model: opus-4-6` 解析後值仍是 `opus-4-6`（**不**被翻譯為 `claude-opus-4-6`），任意 deployment name 字串如 `claude-opus-4-6-2026V2` 原值保留。Verification：`cargo test -p codebus-core --lib config::endpoint::azure_passthrough` 綠。

## 3. Legacy schema 處理

- [x] 3.1 在 `load_claude_code_config` 內實作 **Legacy Config Schema Warning Without Rewrite**（落實 design.md「舊 schema 偵測：警告 + 不改寫」決策）：偵測 yaml `claude_code` 區塊直接含 `goal` / `query` / `fix` 時，stderr 打印 migration 警告（含新 schema 範例）、回傳等價 `active: system` 的設定、user yaml 檔 byte-for-byte 不改寫。Verification：integration test `legacy_schema_warns_without_rewrite` 斷言 stderr 含 migration keyword + 檔案 SHA256 hash 與 invocation 前一致 + 回傳 config 的 active profile 為 system。

## 4. Keyring 與 env fallback

- [x] 4.1 在 `codebus-core/src/config/keyring.rs` 實作 **OS Keyring Integration With Env Fallback**（落實 design.md「OS keyring 整合：service / account 命名與 fallback chain」決策）：`store_azure_key(service, value)` / `read_azure_key(service)` / `delete_azure_key(service)` API，`account` 固定 `default`；`read_azure_key` 走 fallback chain（keyring → `CODEBUS_AZURE_KEY` env → `EndpointKeyMissing` error）。TDD：以 `keyring` crate 的 `mock` feature 寫 unit test 驗證 keyring 命中時不讀 env、keyring 不可用時讀 env、皆缺時回 `EndpointKeyMissing` 且 error message 含 service name 與 `CODEBUS_AZURE_KEY` 字樣。Verification：`cargo test -p codebus-core --lib config::keyring` 綠。

## 5. Scoped env injection

- [x] 5.1 在 `codebus-core/src/agent/env_overrides.rs` 定義 `EnvOverrides` struct 與 builder（落實 design.md「Scoped env injection 的位置與形狀」決策）：內含有序 string→string map；`EnvOverrides::for_system()` 回空 map、`EnvOverrides::for_azure(base_url, api_key)` 回含 `ANTHROPIC_BASE_URL` / `ANTHROPIC_API_KEY` / `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1` 三個鍵的 map。TDD：unit test 斷言 system 變體 map 為空、azure 變體 map 鍵集合恰為三個鍵且 `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` 值為字串 `"1"`。Verification：`cargo test -p codebus-core --lib agent::env_overrides` 綠。
- [x] 5.2 修改 `codebus-core/src/agent/claude_cli.rs` 實作 **Scoped Environment Injection At Spawn**：`InvokeAgentOptions` 加 `env: EnvOverrides` 欄位；`invoke` 內以 `Command::envs(opts.env.iter())` 注入；codebus 程式碼路徑 audit 不含 `std::env::set_var` 呼叫。TDD：unit test `invoke_passes_env_overrides_to_command` 用 stub `CODEBUS_CLAUDE_BIN` 指向印出 env 的 helper 腳本，斷言 azure variant 的三個 env 出現在子 process、parent shell `ANTHROPIC_API_KEY` 在 invoke 後維持原值。Verification：既有 `agent::claude_cli` unit test 仍綠 + 新 test 綠。

## 6. `config` 子命令與 CLI 註冊

- [x] 6.1 在 `codebus-cli/src/commands/config.rs` 實作 **Config Subcommand For Keyring Management**（落實 design.md「`codebus config` 子命令動作集」決策）的 `set-key` / `get-key [--show]` / `delete-key` 三動作；`set-key` 從 stdin 不 echo 讀 key、`get-key` 預設只回 `set`/`unset`、`delete-key` idempotent、未知 profile 引數透過 clap 拒絕。TDD：integration test `codebus-cli/tests/config_subcommand.rs` 涵蓋 set→get→delete round-trip、`get-key --show` 印 key 明文、`delete-key` 對不存在 entry 仍 exit 0、`set-key bedrock` 被 clap 拒絕並 exit 非零。Verification：`cargo test -p codebus-cli --test config_subcommand` 綠。
- [x] 6.2 在 `codebus-cli/src/main.rs` 與 `codebus-cli/src/commands/mod.rs` 註冊 `config` 子命令，實作 **Subcommand Registration** 的六命令清單。Verification：擴張既有 `codebus-cli/tests/cli_routing.rs` 加入「六個子命令列出」、「`codebus config --help` 列三動作」、「`codebus mcp` / `codebus randomverb` 仍被拒絕」三個 scenario，`cargo test -p codebus-cli --test cli_routing` 綠。

## 7. Verb 整合

- [x] 7.1 修改 `codebus-cli/src/commands/goal.rs` / `query.rs` / `fix.rs`，依 active profile 組 `EnvOverrides`：system → `EnvOverrides::for_system()` + `SystemModel::to_cli_flag` 翻譯 model；azure → 先呼叫 `keyring::read_azure_key` 解析 key（缺則回 `EndpointKeyMissing` 並 exit 非零、**不** spawn 子 process）、組 `EnvOverrides::for_azure(...)`、model 字串透傳。Verification：integration test `azure_profile_missing_key_aborts_before_spawn` 斷言 stderr 含 `EndpointKeyMissing` 加上 spawn counter 為 0；既有 verb integration tests 維持綠。

## 8. 手動端到端驗證

- [x] 8.1 照 v2 strategy memo §8.5 的 setup（`https://<resource>.cognitiveservices.azure.com/anthropic` + deployment name），跑 `codebus config set-key azure` 寫入 key、寫好 azure profile 的 `~/.codebus/config.yaml`，再對任一既有 vault 跑 `codebus query "ping"`。Verification：人工確認 (a) Azure endpoint 回 200 並串流出正常回應、(b) 父 PowerShell session 在 invocation 結束後 `$env:ANTHROPIC_API_KEY` 仍為 `$null`（父 shell env 未污染）、(c) `~/.codebus/config.yaml` 內容未被 codebus 改寫。
