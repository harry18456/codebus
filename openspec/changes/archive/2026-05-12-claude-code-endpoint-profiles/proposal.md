## Why

v2 strategy memo §8 已實機驗證 Claude CLI 接 Azure AI Foundry 的 Anthropic-compatible endpoint 可行，但目前 v3 codebus spawn `claude -p` 完全繼承父 shell env——user 想走 Azure 只能污染整個 shell（export `ANTHROPIC_BASE_URL` / `ANTHROPIC_API_KEY` / 還有 undocumented 的 `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`），違反 v2 §11.2 「自帶 Server 只影響 codebus 使用而非直接更改系統設定」原則。同時 `claude_code.{goal,query,fix}.model` 目前是 free string pass-through，未來切換 endpoint 時 system mode 跟 azure mode 的命名規則差異（system 走 brand alias、azure 走 deployment name）容易讓 user 誤用。

Stage A 把 endpoint 設定內化進 codebus config（profile 模式：system / azure），把 api key 收進 OS keyring（不落 config 檔），並讓 spawn 端做 scoped env injection——只對子 process 設 env，父 shell 完全不變。

## What Changes

- `claude_code` config schema 重構為 **profile 模式**：頂層加 `active: system | azure` selector，profile 內容分別放在 `claude_code.system.*` 與 `claude_code.azure.*` 兩個 sibling block。切換 endpoint 只動 `active` 一個 key，另一邊配置原封保留。
- **system profile** 的 verb model 改為 enum：`opus-4-7` / `opus-4-6` / `haiku` / `sonnet`。codebus 內部維護 alias → Claude CLI `--model` flag 對照表（如 `opus-4-6` → `claude-opus-4-6`）。預設 model 為 `opus-4-6`（goal）/ `haiku`（query）/ `sonnet`（fix）。
- **azure profile** 必填 `base_url` 與 `keyring_service`；verb model 為任意字串（deployment name），codebus 不驗證、不翻譯、字串透傳。
- `disable_advisor_tool` 不暴露給 user；azure profile 啟用時 codebus 強制注入 `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`。
- 新增 `codebus config` 子命令（`set-key` / `get-key` / `delete-key`），把 api key 寫入 OS keyring（macOS Keychain / Windows Credential Manager / Linux Secret Service）。**BREAKING**：`cli` capability 的 `Subcommand Registration` requirement 從 `exactly five` 改為包含 `init` / `goal` / `query` / `lint` / `fix` / `config` 六項。
- Spawn 端 (`agent/claude_cli.rs`) 加 `EnvOverrides` 機制：verb command 模組從 config 組出 endpoint 對應的 env map，spawn 時透過 `Command::env(...)` scoped 注入子 process，父 shell 完全不污染。
- Keyring 不可用（headless / CI / 缺 backend）時 fallback：spawn 端可讀 `CODEBUS_AZURE_KEY` 環境變數作為 key 來源；keyring 與 env fallback 都缺則 spawn 前明確失敗。
- **BREAKING**：既有 `~/.codebus/config.yaml` 內的 `claude_code.{goal,query,fix}.{model,effort}` 結構需遷移為 `claude_code.system.{goal,query,fix}.{model,effort}` + `active: system`。codebus 偵測舊 schema 時 stderr 打印 migration 提示，仍按舊 default 行為跑（不自動改寫 user 檔案）。

## Capabilities

### New Capabilities

- `claude-code-config`：codebus spawn Claude CLI 子 process 時所需的 endpoint profile 設定與 scoped env injection 行為。涵蓋 (a) `claude_code.*` config schema（`active` selector + `system` / `azure` 兩個 profile block 的欄位與驗證規則）、(b) system profile 的 model enum 與 Claude CLI flag 對照表、(c) azure profile 字串透傳規則、(d) OS keyring 整合（service / account 命名、`CODEBUS_AZURE_KEY` env fallback、缺 backend 行為）、(e) `Command::env` scoped 注入規則（system 模式不注入、azure 模式注入 `ANTHROPIC_BASE_URL` / `ANTHROPIC_API_KEY` / `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`）。

### Modified Capabilities

- `cli`：`Subcommand Registration` requirement 從 `exactly five` 鬆綁為 `exactly six`，加入 `config` 子命令的契約。`config` 子命令 SHALL 提供 `set-key <profile>` / `get-key <profile> [--show]` / `delete-key <profile>` 三個動作，操作 OS keyring。

## Impact

- 影響 spec：新建 `openspec/specs/claude-code-config/spec.md`；修改 `openspec/specs/cli/spec.md`（subcommand list）。
- 影響程式碼：
  - 新增：
    - codebus-core/src/config/endpoint.rs（profile schema + Deserialize）
    - codebus-core/src/config/keyring.rs（keyring get/set/delete + env fallback）
    - codebus-core/src/agent/env_overrides.rs（EnvOverrides struct + builder）
    - codebus-cli/src/commands/config.rs（config subcommand + key 動作）
    - codebus-cli/tests/config_subcommand.rs
    - codebus-core/tests/endpoint_config_load.rs
    - codebus-core/tests/keyring_fallback.rs
  - 修改：
    - codebus-core/src/config/claude_code.rs（refactor 成 profile 模式 + system enum）
    - codebus-core/src/config/mod.rs（re-export）
    - codebus-core/src/agent/claude_cli.rs（InvokeAgentOptions 加 env: EnvOverrides 欄位；spawn 時 cmd.envs）
    - codebus-core/src/agent/mod.rs
    - codebus-core/src/lib.rs
    - codebus-cli/src/main.rs（clap 註冊 config subcommand）
    - codebus-cli/src/commands/mod.rs
    - codebus-cli/src/commands/goal.rs / query.rs / fix.rs（從 config 組 EnvOverrides 傳入 invoke）
    - codebus-cli/Cargo.toml（加 keyring dependency）
    - codebus-core/Cargo.toml（加 keyring dependency）
  - 刪除：無
- 影響使用者：既有 `~/.codebus/config.yaml` 內舊 schema 需手動遷移到 profile 模式。codebus 偵測舊 schema 時於 stderr 打印 migration 提示，但**不自動改寫 user 檔案**（避免 silent edit）。
- 不影響：codebus-app（Tauri）— Stage B 才處理 App Settings UI；既有 IPC 命令清單不變（`load_global_config` / `save_global_config` 既有 round-trip 行為足以讓 `claude_code.*` 透傳，stage B 加 UI 時才討論是否新增 keyring IPC）。
