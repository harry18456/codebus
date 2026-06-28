## 0. Pre-apply 校準（同步點盤點，先做）

- [x] 0.1 依 design「Pre-apply 校準」盤點 mcp-server 的列舉同步點：tool 數 3→4（加 `vault_list`）、三個 wiki tool 加 optional `vault`。產出待改點清單（grep `wiki_list`/`wiki_search` 找全部列舉處：`server.rs`、`codebus-cli/tests/mcp_server.rs`、spec、`README.md`、`docs/security.md` §7）。驗證＝清單逐項對齊本 change 的 spec delta，無遺漏。
- [x] 0.2 拍板 D2. app-state 讀取下放 codebus-core（移動，非複製）的測試落點（建議：5 個持久化單元測試隨 impl 移到 core、`vault_list.rs` 測試零改動），並盤點 IPC 封閉集 29→32 同步點（`REGISTERED_COMMANDS`、`generate_ipc_handler!`、計數/名稱兩測試、app-shell spec、`lib/ipc.ts`）。驗證＝二選一拍板記錄於 change，後續任務一致遵循。

## 1. Phase 1 — app-state 下放 codebus-core

- [x] 1.1 依 D2. app-state 讀取下放 codebus-core（移動，非複製），把 `AppState`/`StoredVaultEntry`/`load_app_state`/`save_app_state`/`app_state_path`/`CURRENT_SCHEMA_VERSION`/`AppState::empty` 移動到新模組 `codebus-core/src/app_state.rs` 並由 `codebus-core/src/lib.rs` 公開；保持 App-State Persistence 行為不變（schema_version=1、missing→建空、parse/未來 schema→空且不覆寫、save 原子寫）。驗證＝`cargo test -p codebus-core` 涵蓋移入的持久化測試並綠。
- [x] 1.2 依 D2，app 端 `codebus-app/src-tauri/src/state/app_state.rs` 改薄包裝 `pub use codebus_core::app_state::{...}` 並保留 `AppRuntimeState`/`AppRuntimeState::new`（Tauri runtime state，不下放）。行為＝app 公開 API surface 不變。驗證＝`vault_list.rs` 既有測試零改動、`cargo test --workspace` 綠。

## 2. Phase 1 — registry 解析與白名單安全

- [x] 2.1 依 D5. registry 唯讀 + vault 白名單（canonicalize 比對），新增 `codebus-cli/src/mcp/registry.rs`：composes `codebus_core::app_state::load_app_state` + canonicalize gate，回 resolved wiki_root 或 MCP error。行為＝清單外 `vault`（如 `~/.ssh`）拒、`is_missing` entry skip、registry 唯讀不寫。實作對齊 Read-only security boundary。驗證＝`registry.rs` 單元測試（命中／清單外拒／missing skip）。
- [x] 2.2 依 D6. registry 每次 vault 解析時重讀（非啟動快照），vault 解析走 `spawn_blocking` 每次重讀 app-state.json。行為＝server 執行中 app 新增 vault，下次解析即可見、不必重啟。驗證＝整合測試 scenario「Newly added vault becomes visible without restart」。

## 3. Phase 1 — mcp server 雙模式與 tool 介面

- [x] 3.1 依 D1. 雙啟動模式：registry 預設 + `--vault` pinned，`codebus-cli/src/commands/mcp.rs` 的 `McpArgs.vault` 改 `Option<PathBuf>`、`run()` 依有無 `--vault` 分流，`mcp/mod.rs::serve` 收 registry/pinned 模式。行為＝`codebus mcp` 起 registry 模式、`codebus mcp --vault X` 維持 v1 同查詢語意（result 形狀為向後相容加欄位超集）。實作對齊 Single-vault stdio MCP server lifecycle（更名 Stdio MCP server lifecycle and startup modes）。驗證＝整合測試兩模式啟動 scenario。
- [x] 3.2 依 D7. mcp-server tool 查詢核心邏輯零改動（`tools.rs` 不動），在 `server.rs` 為三個 wiki tool 加 optional `vault` 參數（依 D4. vault 參數 optional、省略行為依模式（張力 T3）的矩陣，用 `registry.rs` 解析）——`wiki_list` / `wiki_search` 省略時跨所有 present vault 查、每筆標 `vault` + `name`（`wiki_search` 全域 cap 20 跨 vault 合計）、`wiki_read` 多 vault 省略報錯、pinned 收到 ≠ 釘定 vault 報錯（P1 fail-loud）——並新增 `vault_list` tool 回 `[{vault,name}]`（依 D3. vault 以正規化 path 定址（張力 T1）：`vault`=正規化絕對路徑 id、`name`=display_name 不參與定址），instructions 文案改 multi-vault。實作對齊 Tools-only query surface without path parameters（更名 Tools-only query surface with registry-scoped vault selection）、wiki_list returns the page index、wiki_read returns the paginated page body、wiki_search performs keyword substring search、vault_list enumerates registry vaults、Vault selection across startup modes。驗證＝整合測試「tools/list enumerates the four query tools」「Wiki tools expose an optional vault selector」。

## 4. Phase 1 — 整合測試與文件

- [x] 4.1 完成 Phase 1 — multi-vault server 的整合測試 `codebus-cli/tests/mcp_multi_vault.rs`：覆蓋 registry 啟動、`vault_list` 形狀、白名單命中／拒絕（含 registry 外 path）、Vault selection across startup modes 全矩陣（`wiki_list`/`wiki_search` 省略跨所有 present vault 且每筆標來源、`wiki_search` 跨 vault 全域 cap 20 + truncated、`wiki_read` 多 vault 省略報錯、pinned 收 ≠ 釘定 vault 報錯）、Read-only security boundary 在 registry 模式 raw/code 仍不可達且跨 vault 聚合不逸出 registry。驗證＝`cargo test -p codebus-cli --test mcp_multi_vault` 綠，且既有 `mcp_server.rs` pinned 模式測試維持綠。
- [x] 4.2 [P] 更新 `README.md` MCP 段與 `docs/security.md` §7：四個 tool、registry/pinned 雙模式、registry 白名單、`vault_list` 暴露已登錄 vault path 清單的暴露面說明。行為＝對外文件與 spec 一致（呼應 Pre-apply 校準）。驗證＝內容 review 對齊 mcp-server spec（四 tool + whitelist 文案）。

## 5. Phase 2 — 一鍵接入 IPC

- [x] 5.1 Phase 2 — app 一鍵接入：依 D8. 一鍵接入＝shell out client 原生 CLI（張力 T2 / 決策 5）、D9. client 偵測重用 cli_status、bin 解析重用 agent backend、D10. bundle CLI 絕對路徑解析（Tauri resource），新增 `codebus-app/src-tauri/src/ipc/mcp_install.rs`：`mcp_client_status`/`mcp_client_install`/`mcp_client_remove`，shell out client CLI（argv array、claude 帶 `--scope user`、codex 無 scope、寫 bundle CLI 絕對路徑＋dev fallback），偵測重用 `cli_status::probe_binary`。實作對齊 One-click MCP client registration、Client detection and absent-client handling、MCP-integration IPC commands。驗證＝命令構造單元測試（claude 含 `--scope user`、codex 不含、絕對路徑、argv array、非零退出回 `AppError::Io`）。
- [x] 5.2 註冊三個新命令並維持 IPC Command Registry 封閉集：`ipc/mod.rs` 的 `REGISTERED_COMMANDS` + `generate_ipc_handler!` + 計數／名稱測試 29→32，`codebus-app/src/lib/ipc.ts` 加 binding。驗證＝`REGISTERED_COMMANDS` 計數與名稱集合測試綠、`npm run typecheck` 綠。

## 6. Phase 2 — Settings MCP 整合區塊

- [x] 6.1 新增 `codebus-app/src/components/settings/McpIntegrationSection.tsx` 並掛入 `SettingsModal.tsx`，加 i18n 文案 key。行為＝claude / codex **各一列獨立開關**（不隨 active_provider，P3）：偵測到該 client 顯示開關＋註冊狀態、偵測不到則該列 disable＋提示（不影響另一列）、切換呼叫 install/remove（對齊 One-click MCP client registration、Client detection and absent-client handling）。驗證＝前端測試 detected／absent 兩態 + 兩列獨立 + GUI smoke 開關往返。
