## Context

`codebus-core/src/agent/claude_cli.rs` 的 `invoke` 透過 `Command::new(claude_bin)` spawn `claude -p` 子 process，目前 0 env 處理——子 process 直接繼承父 shell 全部 env。`codebus-core/src/config/claude_code.rs` 的 `ClaudeCodeConfig` 結構是 `{ goal, query, fix }`，每個 verb 有 `model: Option<String>` 與 `effort: Option<String>`，字串透傳給 Claude CLI 的 `--model` / `--effort` flag。

v2 strategy memo `legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md` §8 已驗證 Claude CLI 接 Azure 走 `ANTHROPIC_BASE_URL` 路可行，但要設 3 個 env（含 undocumented 的 `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`）。v2 §11.2 訂下 architectural principle：「自帶 Server 只影響 codebus 使用而非直接更改系統設定」。

本次 change 把 endpoint 配置內化進 codebus、把 api key 收進 OS keyring、把 env 注入 scoped 化（只對子 process），並把 model 命名分兩條路：system profile 走 codebus 維護的 enum、azure profile 走 user 自填的 deployment name 字串透傳。

## Goals / Non-Goals

**Goals:**

- Endpoint 設定有第一公民 config schema，user 不必手動 export shell env
- API key 存 OS keyring（macOS Keychain / Windows Credential Manager / Linux Secret Service），不落 config 檔
- Spawn 端 scoped env injection——父 shell env 完全不變
- system mode 的 model 命名由 codebus 抽象（4 enum），azure mode 的 model 字串透傳（deployment name）
- profile 模式：切換 endpoint 不丟另一邊配置
- 為 Stage B（Tauri App Settings UI）預備 reusable 的 keyring helper

**Non-Goals:**

- 不支援 AWS Bedrock / Google Vertex / LiteLLM / Ollama 等其他 endpoint（未來可加 sibling profile）
- 不支援 Foundry mode (`CLAUDE_CODE_USE_FOUNDRY=1`)——v2 §8.3 已驗證走不通（Claude Code 內建 deployment 預檢 list 不含 opus 4.6）
- 不做 endpoint reachability health-check 子命令（避免擴 cli spec 子命令數量超出本次必要）
- 不做 App Settings UI（Stage B 另開 change）
- 不自動遷移 user 既有 yaml 檔（偵測舊 schema 僅 stderr 警告 + 顯示新 schema 範例，不改寫）
- 不暴露 `disable_advisor_tool` 給 user（azure 模式強制 true、system 模式不注入）

## Decisions

### Profile 模式 vs discriminated union

採 **profile 模式**：頂層 `active: system | azure` selector，加上 `claude_code.system.*` / `claude_code.azure.*` 兩個 sibling block 共存。切換只動 `active` 一個 key，另一邊配置原封保留。

替代方案：discriminated union（`endpoint.type: system | azure` 只保留 active 那份）扁平但切換要重寫整塊、Stage B 的 App Settings UI 切 endpoint 時會丟舊 form 資料。Profile 模式為「未來加 bedrock / vertex」也預留 sibling profile 結構，新 endpoint type 加進來不動既有 profile。

### system profile 的 model enum

system profile 的 verb model 改為 `SystemModel` enum，每個 variant 以 serde `rename` 顯式註記 kebab-case + 版本後綴。合法值：`opus-4-7` / `opus-4-6` / `haiku-4-5` / `sonnet-4-6`（**全部 versioned**，不接受未版本化的 `opus` / `haiku` / `sonnet`）。codebus-core 維護 `to_cli_flag` 對照表把 enum value 翻譯成 Claude CLI 的 `--model` flag value（如 `opus-4-6` → `claude-opus-4-6`、`haiku-4-5` → `claude-haiku-4-5`）。`opus-4-6` 走 v2 §8 已實機驗證的命名；本次 propose 階段 user 確認在 system mode 下執行 `claude -p --model claude-opus-4-6` 可正常運作（claude CLI 2.1.139）。

預設值：`goal: opus-4-6` 、 `query: haiku-4-5` 、 `fix: sonnet-4-6`（user 在 propose 階段選 `opus-4-6` 作為 goal 預設，理由是 v2 已驗證且明確 pin；apply 階段 user 確認 haiku / sonnet 也要版本化，因此 enum 統一為 versioned 形式）。

### azure profile 的 model 字串透傳

azure profile 的 verb model 是任意非空字串（user 在 Azure Portal 看的 deployment name），codebus 完全不驗證、不翻譯。user 寫 `model: opus-4-6` 也照樣送 `opus-4-6` 給 Azure（user 在 Azure 上若有同名 deployment 則通，否則 Azure 回 404）。Spec 註解明示「填 deployment name 不是 brand name」。

替代方案：azure mode 也走 enum 翻譯到 brand name——被否決，因 Azure deployment 命名是 user 自選，強制翻譯反而誤導；透傳是最少假設、最安全的設計。

### OS keyring 整合：service / account 命名與 fallback chain

採 `keyring` crate（cross-platform 標準）。每個 profile 的 keyring 條目以 `(service, account)` 索引：

- `service` = profile 的 `keyring_service` 欄位（azure profile 必填，user 可改；預設 `codebus-azure`）
- `account` = 固定 `default`（不暴露給 user，避免命名負擔）

Key 讀取 fallback chain（spawn 前）：

1. 嘗試 keyring entry 的 `get_password` 動作
2. Keyring backend 不可用或 entry 不存在 → 讀環境變數 `CODEBUS_AZURE_KEY`（azure profile）
3. 兩者皆缺 → spawn 前回 `EndpointKeyMissing` error，**不啟動子 process**

`codebus config set-key <profile>` 寫 keyring；keyring 不可用時退回 stderr 提示「請改用 `CODEBUS_AZURE_KEY` 環境變數」，**不靜默寫到別處**。

### Scoped env injection 的位置與形狀

新增 `codebus-core/src/agent/env_overrides.rs`，定義 `EnvOverrides` struct（內部包一個有序的 string-string map）。`InvokeAgentOptions` 加欄位 `env: EnvOverrides`；`invoke` 內部把 map 傳給子 process 的 `Command::envs`。Verb command 模組（goal / query / fix）負責**從 config 組出 EnvOverrides**：

- system profile：`EnvOverrides::default()`（空 map，純繼承父 env）
- azure profile：注入三組——`ANTHROPIC_BASE_URL` 帶 `base_url`、`ANTHROPIC_API_KEY` 帶從 keyring 或 env 讀到的 key、`CLAUDE_CODE_DISABLE_ADVISOR_TOOL` 設為字串 `"1"`

替代方案：env 邏輯做進 `claude_cli::invoke` 內部——被否決，因會讓 spawn 層直接依賴 config schema，違反 layer 分離。`EnvOverrides` 是中性 plumbing struct，spawn 層不知道有 profile 概念。

### `codebus config` 子命令動作集

`codebus config` 為新子命令，本次 change 開三個動作：

- `codebus config set-key <profile>`：提示 user 由 stdin 輸入 key（不 echo），寫入 keyring entry（`(keyring_service, "default")`）
- `codebus config get-key <profile> [--show]`：預設只回報「key 已設」或「key 未設」（不印 key 內容）；加 `--show` 才印明文
- `codebus config delete-key <profile>`：移除該 profile 的 keyring entry

`<profile>` 目前只接受 `azure`（system profile 無 key 概念）。未來加 endpoint type 時擴 valid value list。

### 舊 schema 偵測：警告 + 不改寫

`load_claude_code_config` 偵測 yaml 中 `claude_code.goal` / `claude_code.query` / `claude_code.fix` 直接出現（沒被 `system` / `azure` block 包住）時：

- stderr 打印 migration 提示加上新 schema 範例
- **不自動改寫 user yaml 檔**（避免 silent edit；user 看提示後自行調整）
- 仍按舊 default 行為（等同 `active: system` 加上既有 model/effort 沿用）執行該次 invoke

## Implementation Contract

**新增的 user-visible 行為**：

1. `~/.codebus/config.yaml` 接受新 profile schema。頂層 `claude_code` 區塊含：
   - `active`：值為 `system` 或 `azure`
   - `system` block：含 `goal` / `query` / `fix` 三個 verb sub-block，每個 sub-block 有 `model`（SystemModel 列舉值）與 `effort`（字串）
   - `azure` block：含 `base_url`（URL 字串）、`keyring_service`（字串）、`goal` / `query` / `fix` 三個 verb sub-block（每個含 `model` 字串與 `effort` 字串）

   `SystemModel` 合法值：`opus-4-7` / `opus-4-6` / `haiku-4-5` / `sonnet-4-6`（全部 versioned）。違法值（含未版本化的 `haiku` / `sonnet`） → `ConfigLoadError::YamlParse`。Azure profile 缺 `base_url` 或 `keyring_service` → 同樣 parse error。`active` 指的 profile 必須完整；未 active 的 profile 可缺或不完整。

2. `codebus config set-key azure`：互動式提示「Enter API key:」，從 stdin 讀（不 echo），寫入 keyring entry。成功 stdout 印 `key stored`；keyring 不可用 stderr 印 fallback 提示加上非零 exit。

3. `codebus config get-key azure [--show]`：預設只回 `set` 或 `unset`；`--show` 才印 key 明文。

4. `codebus config delete-key azure`：移除 entry；entry 不存在亦 exit 0（idempotent）。

5. `codebus goal` / `codebus query` / `codebus fix` 執行時：
   - 讀 `claude_code.active` 決定 profile
   - system profile：spawn 子 process 不加任何 env，model 經 enum→flag 翻譯後送 `--model` flag
   - azure profile：先嘗試讀 keyring；失敗讀 `CODEBUS_AZURE_KEY`；皆缺則回 `EndpointKeyMissing` error 並 exit 非零，**不 spawn 子 process**。讀到 key 後 spawn 時注入 `ANTHROPIC_BASE_URL` / `ANTHROPIC_API_KEY` / `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1` 三個 env（透過 `Command::env`），父 shell env 不變。Model 字串透傳給 `--model`。

6. `codebus --help` 列出六個子命令：`init` / `goal` / `query` / `lint` / `fix` / `config`。

**Acceptance criteria**：

- `codebus-core/src/config/endpoint.rs` 的 unit test 驗證 profile schema 各種合法或不合法輸入的 parse 行為（含 `active` 指 azure 但 azure block 缺欄位 → error；active 指 system 但 system block 缺 verb → error；舊 schema → 仍 load 成功加上 stderr 警告）
- `codebus-core/src/agent/env_overrides.rs` 的 unit test 驗證 system profile 產生空 map、azure profile 產生 3 個 env 鍵
- `codebus-cli/tests/config_subcommand.rs` 整合測試 `set-key` / `get-key` / `delete-key` 三個動作（用 `keyring` 的 mock backend）
- `codebus-core/tests/keyring_fallback.rs` 驗證 keyring 不可用時 `CODEBUS_AZURE_KEY` env fallback 生效；皆缺則 spawn 前 error
- 手動驗證：照 v2 strategy memo §8.5 的 setup，跑 `codebus config set-key azure` 加上寫好 azure profile config 加上 `codebus query` 對某 vault → 走 Azure endpoint 成功、檢查父 shell `ANTHROPIC_API_KEY` 在 codebus 跑完後仍為空（父 shell 未污染）
- `codebus --help` 列出六個子命令；`codebus config --help` 列出三個子動作

**Scope boundaries**：

- 本次 change 只動 CLI 加上 codebus-core，**不**動 codebus-app（Tauri）— App Settings UI 屬 Stage B
- **不**加新 IPC 命令（codebus-app 既有 `load_global_config` / `save_global_config` 的 round-trip 行為自動 carry profile schema，但本次不加 keyring 相關 IPC）
- **不**改 sandbox flag、stream parser、render、lint、PII、run-log 等其他 spec 對應的行為
- **不**改 `codebus init` 的 vault bootstrap 行為（init 不會建 endpoint config，user 仍需手動編輯 `~/.codebus/config.yaml`）
- **不**做 model alias 跨版本對照（如「以後 4.8 出來時自動 fallback」）—— enum 值只支援當前列舉，未來加新版本 model 再開另一 change

## Risks / Trade-offs

- **Keyring crate cross-platform 行為**：Linux Secret Service 需要 backend daemon（gnome-keyring 或 KWallet）running；headless / Docker / WSL 無 GUI session 時可能 unusable → mitigation: 明確 `CODEBUS_AZURE_KEY` env fallback 加上 spawn 前 clear error
- **新 schema 對既有 user 不向後相容**：除了 harry 自己沒人在用 codebus，影響面極小；偵測舊 schema 時 stderr 警告加上範例對照 → mitigation: 不自動改寫 user 檔，user 看完警告再調整
- **`opus-4-6` enum 值與未來 Anthropic model 版本演化的維護成本**：每出新版要 PR 改 enum；trade-off 是 user 不用記長 alias 字串、命名一致 → mitigation: 未來新增 variant 時開另一 change
- **Azure deployment 命名 user 寫錯（如把 brand name 當 deployment name）**：runtime 才會看到 Azure 回 404 → mitigation: spec 註解明示語意；error message 包含 base_url 讓 user 易追問題
- **`disable_advisor_tool` 強制 true 將來若 Azure 支援該 beta header**：可能變成不必要的繞道 → mitigation: 維持 hardcode；該 beta header 不被外部 endpoint 支援是 Claude Code 設計層面的問題，未來真的廢除再開 change 拿掉
- **Keyring service 預設值 `codebus-azure` 跟未來其他 endpoint type 命名衝突風險**：若加 bedrock 用 `codebus-bedrock` 即可避開；目前 azure 是唯一 endpoint type，無實際衝突 → mitigation: 命名以 endpoint type 為後綴的慣例（spec 註記）
