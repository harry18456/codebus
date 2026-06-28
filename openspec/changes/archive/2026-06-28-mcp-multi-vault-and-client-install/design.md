## Context

MCP server v1（`mcp-server-single-vault`，已 ship）是 single-vault：`codebus-cli/src/commands/mcp.rs` 的 `McpArgs { vault: PathBuf }` 在啟動時把 `<vault>/.codebus/wiki/` 釘定為 wiki root，三個 tool（`wiki_list` / `wiki_read` / `wiki_search`）都不收路徑。痛點是多專案要在 client 端各設一條 server entry。

關鍵現況（grounded）：

- `codebus-cli/src/mcp/tools.rs` 的查詢函式（`paginate` / `resolve_page_path` / `search_pages`）本來就收 `wiki_root: &Path`，架構已 multi-vault-ready，核心查詢邏輯零改動。
- registry 來源 `~/.codebus/app-state.json` 的讀取邏輯目前住在 app 端 `codebus-app/src-tauri/src/state/app_state.rs`：`AppState` / `StoredVaultEntry` / `load_app_state` / `app_state_path` / `save_app_state`，是純 fs + serde，無 Tauri 依賴；`AppRuntimeState` / `ActiveRuns` 才是 Tauri runtime state。
- app-shell spec `App-State Persistence` requirement 目前明文「The CLI SHALL NOT read or write `app-state.json`」，且 registry 已正規化 path（add_vault 寫入時 `normalize_path` + strip verbatim）。
- client 偵測既有 seam：`codebus-app/src-tauri/src/ipc/cli_status.rs`（`probe_binary` 跑 `<bin> --version`、Windows 走 `cmd /C` 解 `.cmd` shim）；agent backend bin 解析：`claude_backend.rs`（`CODEBUS_CLAUDE_BIN` → `claude`）/ `codex_backend.rs`（`CODEBUS_CODEX_BIN` → `default_codex_bin()` Windows `codex.cmd`）。
- app bundle 已把 CLI 收進 resource：`tauri.conf.json` 的 `resources` 把 `bin-staging/codebus.exe` 映射成安裝後的 `bin/codebus.exe`，但**目前沒有任何 runtime resolver**去解析它。
- IPC 命令是封閉集：`codebus-app/src-tauri/src/ipc/mod.rs` 的 `REGISTERED_COMMANDS`（29 條）+ 兩個測試（計數 + 名稱集合）+ `generate_ipc_handler!` macro 三者鎖在一起。

## Goals / Non-Goals

**Goals:**

- `codebus mcp` 無 `--vault` 時走 registry 模式，一個進程服務 app-state 已登錄的全部 vault；`--vault` 顯式模式向後相容。
- 新增 `vault_list` tool 讓 agent 自我發現可用 vault；三個 wiki tool 在 registry 模式收 optional `vault`。
- registry 解析邏輯下放 core、CLI 與 app 共用一份，避免漂移。
- MCP 對 registry 唯讀 + vault 白名單，維持「只讀 wiki 子樹、raw/code 不可達」的既有安全姿態。
- app Settings 一鍵把 codebus 註冊 / 移除為 client 的 user-scope MCP server。

**Non-Goals:**

- `wiki_list` 分頁、`wiki_search` 升 RAG / 語意檢索、寫操作、`run_list` tool。
- 首次啟動 onboarding 引導（開關放 Settings）。
- macOS / Linux 實機驗證（命令構造跨平台，實機只驗 Windows）。
- 改 app-state.json 檔案 schema（`schema_version: 1` 不動）。
- 修「純 CLI 建立的 vault 不在 registry」此一已知限制（靠文件 + `--vault` 兜底）。

## Decisions

### D1. 雙啟動模式：registry 預設 + `--vault` pinned

`McpArgs.vault: PathBuf` 改為 `Option<PathBuf>`。`Some(path)` → 沿用 v1 釘定單一 vault；`None` → registry 模式（讀 app-state）。`mcp.rs::run` 依此分流，`mcp/mod.rs::serve` 拆成兩條入口或收一個 `ServeMode { Pinned(PathBuf), Registry }`。**替代方案**：另開 `codebus mcp-multi` 子命令——否決，雙模式同一子命令對使用者更自然、且 cli spec 的 subcommand 封閉集不必動。

### D2. app-state 讀取下放 codebus-core（移動，非複製）

把 `AppState` / `StoredVaultEntry` / `CURRENT_SCHEMA_VERSION` / `AppState::empty` / `app_state_path` / `load_app_state` / `save_app_state` 從 app **移動**到新模組 `codebus-core/src/app_state.rs`（`codebus_core::app_state`），`codebus-core/src/lib.rs` 公開它。app 端 `state/app_state.rs` 改成 `pub use codebus_core::app_state::{...}` 薄包裝，並**保留** `AppRuntimeState` / `AppRuntimeState::new`（用 `super::active_runs::ActiveRuns`，是 Tauri runtime state，不下放）。**必須移動非複製**：否則 CLI 與 app 各一份 registry 解析會漂移。core 已有 `dirs` 依賴（`Cargo.toml`），`app_state_path` 的 `CODEBUS_HOME` 覆寫 + `dirs::home_dir` 直接搬，零新依賴。**替代方案**：CLI 自己另寫一份 app-state 讀取——否決（漂移風險，正是本決策要避免的）。

### D3. vault 以正規化 path 定址（張力 T1）

`vault_list` 回 `[{ vault: <正規化絕對路徑>, name: <display_name> }]`。`vault`（正規化 path）= 呼叫用的穩定 id；`name`（display_name）= 純顯示 label、可重複、不參與定址。理由：id 消費者是 agent（程式）不是人，「可讀」無價值；display_name 去重會集合相依（加同名 vault 改既有 id）且跟 per-call 重讀交互在 long session 會 break；path 100% 穩定唯一、agent 從 `vault_list` 複製貼上零負擔。registry 內 path 已由 app 端 `normalize_path` 正規化。**替代方案**：用 display_name 或一個合成短 id 當定址鍵——否決（不穩定 / 集合相依）。

### D4. vault 參數 optional、省略行為依模式（張力 T3）

統一一套 tool schema，`vault` 為 optional；**省略語意依工具性質而不同**——`wiki_list` / `wiki_search` 是「探索」（省略＝跨所有 present vault 查、每筆標來源 vault），`wiki_read` 是「定位單頁」（slug 跨 vault 可重複，需明確 vault，正常從 search/list 結果帶回）。

| 模式 | present | vault | `wiki_list` / `wiki_search` | `wiki_read` |
| --- | --- | --- | --- | --- |
| registry | 多 | 省略 | 跨所有 present vault（標 vault） | error：read 需指定 vault |
| registry | 1 | 省略 | 那一個 | 那一個 |
| registry | 任意 | in-registry | 限定那個 | 限定那個 |
| registry | 任意 | not-in-registry | error | error |
| registry | 0 | 任意 | error：no vault registered | error：no vault registered |
| pinned | n/a | 省略 | pinned | pinned |
| pinned | n/a | ≠ pinned | **error（P1 fail-loud）** | **error（P1 fail-loud）** |

理由（rationale）：有 global registry 記錄所有 vault，agent 對它的自然用法是「跨全部探索」，不該被迫先選一個再查；`vault_list` 仍保留為 discovery、但非必經。`wiki_read` 因為要回單頁、slug 可能跨 vault 重複，必須明確定位（從 `wiki_list` / `wiki_search` 結果上帶回的 `vault`）。

- 效能：跨所有 present vault grep（registry 通常個位數、`spawn_blocking`、search 全域 cap 20 為跨 vault 合計）。
- 安全：跨全部只 iterate registry present vault（天然落在 D5 白名單內、不會碰 registry 外路徑）；給 vault 仍 canonicalize 驗、`raw/code/` 不可達不變（見 D5）。
- 結果形狀：registry 模式下 `wiki_list` / `wiki_search` 每筆帶 `vault` + `name` 來源標記，讓 agent 能把正確 `vault` 帶進 `wiki_read`。
- P1（pinned mismatch）：pinned 模式收到 ≠ 釘定的 `vault` → MCP error（fail-loud），不 silent ignore。

**替代方案**：(a) 多 vault 省略時報錯逼先選一個——否決（違反 global registry「跨全部探索」的直覺）；(b) registry 模式強制必填 vault——否決（單一 vault 時多此一舉，傷 UX）。

### D5. registry 唯讀 + vault 白名單（canonicalize 比對）

MCP 只**讀** app-state.json，不寫（寫是 app `add_vault` / `remove_vault` 的職責）。tool 傳入的 `vault` 必須是 registry 成員：對傳入值與每個 registry entry 都 `canonicalize` 後比對（吸收 verbatim prefix / 大小寫 / trailing slash 差異），命中才放行；清單外（如 `~/.ssh`）一律拒（MCP `invalid_params`）。`is_missing`（path 不存在 / canonicalize 失敗）的 entry 從 `vault_list` skip、被顯式傳入也拒。命中後沿用既有 `resolve_page_path` 的 canonicalize 落點檢查，`raw/code/` 仍不可達。白名單比對邏輯住在新檔 `codebus-cli/src/mcp/registry.rs`（composes `codebus_core::app_state::load_app_state` + canonicalize gate），保持 core 純讀。**替代方案**：信任傳入 path 直接開——否決（等於把任意目錄讀取面開給 client agent）。

### D6. registry 每次 vault 解析時重讀（非啟動快照）

MCP 長駐期間每次解析 vault 都重讀 app-state.json（檔小、`spawn_blocking`），GUI 新增 vault 即時可見、不必重啟。對齊 app-shell spec 既有「External vault add refreshes Lobby」的 watcher 語意（外部 process append app-state.json）。**替代方案**：啟動時快照一次——否決（GUI 加 vault 後 server 看不到，違反一鍵接入的順手感）。

### D7. mcp-server tool 查詢核心邏輯零改動

`tools.rs`（`paginate` / `resolve_page_path` / `search_pages`）不改——它們本就收 `wiki_root: &Path`。改動集中在 `server.rs`（tool handler 加 vault 參數 + 新 `vault_list` + instructions 文案）、`mod.rs`（serve 分流）、`commands/mcp.rs`（args + dispatch）、新 `registry.rs`。`wiki_list` 目前無參數 struct，需新增一個含 optional `vault` 的 args struct。

### D8. 一鍵接入＝shell out client 原生 CLI（張力 T2 / 決策 5）

app 不解析 / merge client 設定檔，只 shell out client 原生 CLI、用 argv array（非 shell string）：

- claude 安裝：`claude mcp add --scope user codebus -- <CLI絕對路徑> mcp`（**`--scope` 預設是 local＝只當前 project，要 global 必須明確 `--scope user`**，真陷阱）。移除：`claude mcp remove --scope user codebus`。狀態：`claude mcp list` / `claude mcp get codebus`。
- codex 安裝：`codex mcp add codebus -- <CLI絕對路徑> mcp`（codex config 單一 global、無 scope）。移除：`codex mcp remove codebus`。狀態：`codex mcp list`。

**替代方案**：app 自己讀寫 client 的 `.mcp.json` / TOML——否決（耦合 client 內部格式、易隨 client 版本破、要處理 merge 衝突）。

### D9. client 偵測重用 cli_status、bin 解析重用 agent backend

- **偵測 client 是否安裝**：重用 `cli_status::probe_binary`（`<bin> --version`）——這就是 Settings 既有「CLI status」那一列的機制；偵測不到回 `client_missing`，UI disable + 友善提示，不報錯。
- **解析要呼叫的 client bin**：重用 agent backend 規則（`CODEBUS_CLAUDE_BIN` → `claude` / `CODEBUS_CODEX_BIN` → `default_codex_bin()`，Windows `codex.cmd`），以 `Command::new(<bin>).args([argv...])` 直接帶 argv（Rust 在 Windows 會經 cmd 跑 `.cmd` shim 並自行 quoting），**不**用 `cmd /C` 拼字串（`--` + 含空格路徑用 `cmd /C` 拼接易碎）。Windows 隱藏 console 沿用 `codebus_core::win_console::hide_console`。

### D10. bundle CLI 絕對路徑解析（Tauri resource）

一鍵接入寫 CLI 絕對路徑（非裸 `codebus`、不靠 PATH）。透過 Tauri path API 解析 bundle resource `bin/codebus.exe`（對應 `tauri.conf.json` 的 `resources` 映射）。**注意**：此 resource 只在 packaged build 存在，`npm run tauri dev` 下不存在 → dev 需 fallback（解析 `target/debug/codebus.exe` 或 PATH 上的 `codebus`），否則 dev 期間一鍵接入無法實測。此 fallback 是 dev-only，不進 packaged 行為。

## Implementation Contract

### Phase 1 — multi-vault server

**Behavior（使用者 / 呼叫端可觀察）**

- `codebus mcp`（無 `--vault`）啟動 registry 模式 server；stderr 印 registry 模式啟動訊息；stdout 純 JSON-RPC。
- `codebus mcp --vault <path>` pinned 模式，與 v1 同查詢語意（result 形狀為加欄位的向後相容超集）。
- registry 模式 `tools/list` 列**四**個 tool：`vault_list` + `wiki_list` / `wiki_read` / `wiki_search`。pinned 模式同樣四個（`vault_list` 在 pinned 模式回那一個釘定 vault）。
- registry 多 vault 省略 `vault`：`wiki_list` / `wiki_search` 跨所有 present vault 查、每筆標 `vault` + `name`（search 全域 cap 20 跨 vault 合計）；`wiki_read` 省略 + 多 vault → error（需指定 vault）。
- 無任何寫 tool；`wiki_*` 的 `vault` 不是任意路徑入口，只能選 registry 成員（或省略時跨 present vault 探索）。

**Interface / data shape**

- `McpArgs { vault: Option<PathBuf> }`（`codebus-cli/src/commands/mcp.rs`）。
- `vault_list()` → JSON 陣列 `[{ "vault": "<abs-normalized-path>", "name": "<display_name>" }]`；空 registry → `[]`（success，非 error）。
- `wiki_list` / `wiki_read` / `wiki_search` 各新增 optional `vault: Option<String>`（值＝`vault_list` 給的 path id）；registry 模式下 `wiki_list` / `wiki_search` 每筆結果帶 `vault` + `name` 來源標記、`wiki_read` 多 vault 省略則報錯；分頁 / snippet 等核心語意不變（`wiki_search` cap 改為跨 vault 全域 20）。
- `codebus_core::app_state` 公開：`AppState`、`StoredVaultEntry`、`CURRENT_SCHEMA_VERSION`、`app_state_path() -> Option<PathBuf>`、`load_app_state(&Path) -> AppState`、`save_app_state(&Path, &AppState) -> io::Result<()>`、`AppState::empty()`。
- `codebus-cli/src/mcp/registry.rs`：給定 optional vault 字串 + 模式，回 `Result<PathBuf /* wiki_root */, McpResolveError>`；`McpResolveError` 映射成 MCP `invalid_params`。

**Failure modes**

- registry 多 vault + 省略 vault：`wiki_list` / `wiki_search` 跨所有 present vault 查（非錯誤、每筆標來源）；`wiki_read` → `invalid_params`「read 需指定 vault」。
- pinned 模式 + 傳入 ≠ 釘定 vault → `invalid_params`「vault mismatch」（P1 fail-loud，不 silent ignore）。
- 傳入 vault 不在 registry（canonicalize 後無命中）或為 `is_missing` → `invalid_params`「vault 不在 registry 白名單」。
- registry 模式但 app-state.json 不存在 / 空清單：`vault_list` 回 `[]`；`wiki_*` 省略或傳 vault 皆 `invalid_params`「尚無已登錄 vault」。
- 真實 fs 錯誤仍回 MCP `ErrorData`（不吞成空 success），沿用既有 error-vs-empty 語意。

**Acceptance criteria**

- 新整合測試 `codebus-cli/tests/mcp_multi_vault.rs`：registry 模式啟動、`vault_list` 輸出形狀、白名單命中 / 拒絕、D4 省略行為矩陣（search/list 跨全部、read 多省略報錯、pinned mismatch 報錯）、raw/code 在 registry 模式仍不可達（traversal slug + registry-外 path 雙向）。
- 既有 `codebus-cli/tests/mcp_server.rs` pinned 模式斷言維持綠（向後相容）。
- `cargo test -p codebus-core` 涵蓋下放後的 `app_state` 模組；app 端 `vault_list.rs` 測試零改動維持綠（re-export 保 API surface）。

**Scope boundaries**：In — D1–D7 全部 + spec / README / security.md 更新。Out — Phase 2、Non-Goals 所列。

### Phase 2 — app 一鍵接入

**Behavior**：Settings 出現 MCP 整合區塊，**claude / codex 各一列獨立開關**（不隨 active_provider，P3）：偵測到該 client → 顯示開關 + 目前是否已註冊；切 ON → codebus 加進該 client 的 user-scope MCP server；切 OFF → 移除；偵測不到 → 該列 disable + 提示（不影響另一列）。

**Interface / data shape（新增三個 IPC 命令）**

- `mcp_client_status(provider: String) -> McpClientStatus`：`McpClientStatus`（`serde(tag="kind", rename_all="snake_case")`）變體 `installed` / `not_registered` / `client_missing`。
- `mcp_client_install(provider: String) -> Result<(), AppError>`：解析 bundle CLI 絕對路徑 + 構造 argv（D8）+ shell out（D9）。
- `mcp_client_remove(provider: String) -> Result<(), AppError>`：shell out 對應 remove 命令。
- `provider` 合法值 `"claude_code"` / `"codex"`（沿用 `binary_for_provider` 既有對應）；非法值 → `client_missing`（不報錯）。

**Failure modes**：client 未安裝 → `client_missing`（status）/ install 命令回前明確化；shell out 非零退出 → `AppError::Io`（帶 stderr 末段）；dev 模式無 bundle resource → 走 dev fallback 路徑（D10）。

**Acceptance criteria**：`mcp_install` 命令構造單元測試（claude 帶 `--scope user`、codex 不帶、argv array 形式、絕對路徑）；`REGISTERED_COMMANDS` 計數 + 名稱集合測試更新並綠；Settings 兩列（claude / codex）獨立開關 GUI smoke（偵測到 / 偵測不到兩態、互不影響）。

**Scope boundaries**：In — 三個 IPC + Settings 區塊 + i18n + app-shell IPC registry 同步。Out — 自動 merge client 設定、onboarding。

## Pre-apply 校準

apply 第一步先做這些同步點校準，別憑記憶（呼應「閉集 scenario 全對齊」教訓）：

1. **mcp-server tool 數 3 → 4**（加 `vault_list`）。同步點：`server.rs` tool handler、`mcp_server.rs` 整合測試的 `tools/list` 斷言、spec `mcp-server` requirement「tools surface」列舉、`README.md` MCP 段的 tool 清單、`docs/security.md` §7 的「只暴露三個 tool」文案。grep `wiki_list` / `wiki_search` 找出全部列舉點。
2. **wiki tool 加 optional `vault` 參數**。同步點：`WikiReadArgs` / `WikiSearchArgs` + 新增 `wiki_list` 的 args struct、整合測試的 input schema 斷言（v1 曾斷言「不含 vault/path」，現在要反過來）、spec scenario、security.md「tool 不收路徑」文案改成「收 registry 白名單內的 vault」。
3. **IPC 命令封閉集 29 → 32**（+`mcp_client_status` / `mcp_client_install` / `mcp_client_remove`）。同步點：`ipc/mod.rs` 的 `REGISTERED_COMMANDS`、`generate_ipc_handler!` macro、兩個測試（`exactly_twenty_nine_commands_are_registered` 計數與函式名要一起改、`command_names_match_spec` 名稱集合）、app-shell spec `IPC Command Registry` MODIFIED、`lib/ipc.ts` binding。
4. **app_state.rs 測試落點（一個判斷題，建議先拍板）**：建議把 app `app_state.rs` 現有 5 個檔案持久化單元測試**隨 impl 一起移動**到 `codebus_core::app_state`（move-not-copy 的自然延伸，`cargo test -p codebus-core` 才直接覆蓋）；`vault_list.rs` 測試維持零改動（這是「re-export 不破 API」的承重保證）。替代：app_state.rs 測試原樣留 app、透過 re-export 測 core（字面「零改動」更貼，但 core 在 `-p codebus-core` 無直接覆蓋）。apply 時二選一並一致。
5. **app-shell spec 兩處 MODIFIED 確認**：`App-State Persistence`（放寬 CLI 唯讀 registry）；`IPC Command Registry`（開頭已改委派式表述＝基礎命令列舉 + 各 capability 委派 + 總數 thirty-two，P2）。code 端 `REGISTERED_COMMANDS` 計數測試需對齊 32、不補列無關命令、不擴 scope。
6. **cli spec 不動**：已 grounded 確認 subcommand 封閉集（含 `mcp`）不變、`--vault` optional 純屬 mcp-server capability，cli spec 無需 MODIFIED；apply 不要誤加 cli delta。

## Risks / Trade-offs

- [registry 白名單若只比字串會被 verbatim / 大小寫繞過或誤拒] → 兩側都 canonicalize 後比對（D5）。
- [純 CLI 建立的 vault 不在 registry，registry 模式看不到] → 已知限制，文件寫明 + `--vault` 覆寫兜底；不在本 change 修。
- [Phase 2 bundle CLI 在 `tauri dev` 不存在 → 一鍵接入 dev 期間無法實測] → D10 dev fallback；packaged 行為以 build 產物驗。
- [下放 app_state 若改成「複製」會雙份漂移] → 強制移動 + app 薄包裝 re-export（D2）。
- [client CLI 的 `mcp add/remove/list` 子命令形式隨 client 版本演變] → 只構造 + shell out + 解析退出碼 / 條目存在與否，不解析其內部設定格式，破面最小；shell out 失敗回 `AppError::Io` 讓使用者看得到。
- [`vault_list` 把已登錄 vault 的絕對路徑清單暴露給連線的 client agent] → 這些 client 本就是使用者自己的 agent、且只給 path（不給內容）；security.md §7 補一句說明此暴露面。

## Migration Plan

- Phase 1 先 apply：`--vault` pinned 路徑查詢語意等價（result 形狀為向後相容加欄位超集）、registry 模式為純新增，無資料遷移。回退＝revert CLI/core 變更，app `state/app_state.rs` 還原為自有定義（re-export 還原成原本 struct/fn）。
- Phase 2 後 apply：純新增 IPC + UI，回退＝移除三個命令 + 區塊並還原 registry 計數。
- app-state.json schema 不變，新舊 server 互不破壞既有檔案。

## Open Questions

- pinned 模式傳入 ≠ 釘定 `vault` 怎麼處理？**已定稿（P1）**：MCP error / fail-loud，不 silent ignore（見 D4 矩陣末列 + spec scenario「Pinned mode rejects a mismatched vault」）。
- Phase 2 Settings 一個開關隨 `active_provider` 還是各一列？**已定稿（P3）**：claude / codex 各一列獨立開關、不隨 `active_provider`（backend IPC 本就 per-provider；見 mcp-client-install spec「Each supported client has an independent control」+ task 6.1）。
- 目前無未決問題。
