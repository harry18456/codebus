## Why

codebus 的 MCP server v1（change `mcp-server-single-vault`，已 ship）是 single-vault：`codebus mcp --vault <path>` 一個進程綁定一個 vault、tool 不收路徑。要讓多個專案被外部 agent 查詢，使用者必須在 claude / codex 的 MCP 設定檔裡為每個 vault 各手寫一條 server entry，難用且容易過時。本 change 把 server 升級為 multi-vault（讀 app-state registry，一條 entry 即服務全部已登錄 vault），並加上 app 端「一鍵接入」，把手動設定 client 的負擔拿掉。

## What Changes

分兩 Phase，可分階段 apply：**Phase 1 可獨立 apply + 驗證後，再做 Phase 2**（Phase 2 依賴 Phase 1 的 registry 模式能動）。

**Phase 1 — multi-vault server（CLI / core）**

- `codebus mcp` 雙啟動模式並存：省略 `--vault` 時走 registry 模式，讀 `~/.codebus/app-state.json` 的 vault 清單、一個進程服務全部已登錄 vault；`codebus mcp --vault <path>` 顯式單一模式保留、向後相容（**非 BREAKING**）。
- 三個既有 tool（`wiki_list` / `wiki_read` / `wiki_search`）在 registry 模式新增 optional `vault` 參數；新增第四個 tool `vault_list`，回傳已登錄 vault 的 `{ vault: <正規化絕對路徑>, name: <display_name> }` 清單。
- app-state 讀取邏輯**下放**（移動，非複製）codebus-core（`AppState` / `StoredVaultEntry` / `load_app_state` / `app_state_path` 等純 fs + serde 邏輯），app 端改薄包裝 re-export，CLI 與 app 共用同一份 registry 解析，避免兩份邏輯漂移。
- 安全邊界：MCP 對 registry **唯讀**；tool 傳入的 `vault` 必須是 registry 成員（canonicalize 後比對白名單）才放行，清單外路徑（如 `~/.ssh`）一律拒；`is_missing` 的 vault skip；沿用既有 `resolve_page_path` 的 canonicalize 防穿越，`<vault>/.codebus/raw/code/`（PII 去識別化鏡像）仍不可達。
- registry 每次 vault 解析時重讀（非啟動快照），GUI 期間新增 vault 即時可見、不必重啟 server。

**Phase 2 — app 一鍵接入（app / UI）**

- Settings 新增 MCP 整合開關：偵測 client（claude / codex）是否安裝，開啟時 shell out client 原生 CLI 把 codebus 註冊為 user-scope MCP server，關閉時移除；開關狀態由查詢 client 已登錄條目得出。
- 一鍵接入寫的是 app 內建 bundle 的 codebus CLI 絕對路徑（不依賴 PATH）；shell out 用 argv array（非 shell string，防路徑含空格 / 注入）。
- app 完全不解析 / merge client 的 JSON / TOML 設定檔，只偵測 client + 構造命令 + shell out + 回報結果；偵測不到 client 就 skip + 友善提示，不報錯。
- 新增三個 Tauri IPC 命令（client 安裝狀態查詢 / 安裝 / 移除）。

## Non-Goals

詳細 Non-Goals 收在 design.md 的 Goals / Non-Goals 段；此處先列 scope 排除以利審閱：

- 不做 `wiki_list` 分頁、不把 keyword search 升級成 RAG / 語意檢索（`wiki_search` 仍是 grep fallback）。
- 不做首次啟動 onboarding 引導（目前無 onboarding flow；開關放在 Settings）。
- 不加任何寫操作、不加 `run_list` tool。
- 不在本 change 對 macOS / Linux 做實機驗證（命令構造跨平台，但實機只驗 Windows）。
- 不改既有 app-state.json 檔案 schema（`schema_version: 1` 不動）。
- 純 CLI（沒開過 app）建立的 vault 不在 registry → registry 模式看不到；靠文件說明 + `--vault` 覆寫兜底（已知限制，誠實寫明，不在本 change 修）。

## Capabilities

### New Capabilities

- `mcp-client-install`: app 端把 codebus 一鍵註冊 / 移除為 client（claude / codex）的 user-scope MCP server — client 偵測、shell out client 原生 CLI（argv array、寫 bundle CLI 絕對路徑）、Settings 開關 + 狀態查詢，及對應的 Tauri IPC 命令。

### Modified Capabilities

- `mcp-server`: 由 single-vault 升級為雙啟動模式（registry / `--vault` pinned）；三個 wiki tool 新增 optional `vault` 參數、新增 `vault_list` tool、vault 省略行為依模式定義、registry 白名單安全邊界。
- `app-shell`: `App-State Persistence` 放寬「CLI 不得讀 app-state.json」的限制——`mcp` 子命令可**唯讀** registry（其餘 CLI 子命令仍不得讀寫，app 仍是唯一 writer）；`IPC Command Registry` 納入 Phase 2 的 MCP 安裝命令（總數 + 名稱）。

## Impact

**Phase 1（multi-vault server）**

- New:
  - codebus-core/src/app_state.rs（app-state 讀取邏輯下放 core 的新模組 + 既有檔案持久化單元測試）
  - codebus-cli/src/mcp/registry.rs（registry 讀取 + vault 白名單解析，composes core::app_state 與 canonicalize gate）
  - codebus-cli/tests/mcp_multi_vault.rs（registry 模式 + 白名單 + 省略行為整合測試）
- Modified:
  - codebus-cli/src/commands/mcp.rs（McpArgs.vault 改 Option、雙模式 dispatch）
  - codebus-cli/src/mcp/server.rs（tool 加 vault 參數、vault_list tool、instructions 文案、vault 解析）
  - codebus-cli/src/mcp/mod.rs（serve 入口分流 registry vs pinned）
  - codebus-core/src/lib.rs（公開 app_state 模組）
  - codebus-app/src-tauri/src/state/app_state.rs（薄包裝 re-export core、保留 AppRuntimeState / ActiveRuns）
  - README.md（MCP 段落改 multi-vault）
  - docs/security.md（§7 MCP 暴露面：multi-vault + registry 白名單）

**Phase 2（app 一鍵接入）**

- New:
  - codebus-app/src-tauri/src/ipc/mcp_install.rs（client 安裝狀態 / 安裝 / 移除 IPC，shell out client CLI）
  - codebus-app/src/components/settings/McpIntegrationSection.tsx（Settings MCP 整合區塊）
  - codebus-app/src-tauri/tests/mcp_install_ipc.rs（命令構造 / client 偵測單元測試）
- Modified:
  - codebus-app/src-tauri/src/ipc/mod.rs（註冊新命令、REGISTERED_COMMANDS 清單 + 計數測試同步）
  - codebus-app/src/lib/ipc.ts（新命令 binding）
  - codebus-app/src/components/settings/SettingsModal.tsx（掛載 MCP 整合區塊）
  - codebus-app/src/i18n（新增區塊文案 key）
- Dependencies: 無新增 crate / npm 套件（重用既有 cli_status 偵測 seam、Tauri resource path API、std::process shell out）。
