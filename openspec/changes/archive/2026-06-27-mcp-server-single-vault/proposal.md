## Why

codebus 已能把陌生 codebase 整理成 Obsidian 相容的 wiki vault，但這份知識目前只能透過 codebus 自家 CLI（`chat` / `query` / `quiz`）或桌面 App 消費。外部 agent（Claude Code、OpenAI Codex 等）無法在它們自己的工作流程裡查詢這份 wiki。Phase 0 spike（rmcp 1.8.0、stdio、tool-only）已驗證 Claude Code 與 codex 0.142.2 兩個 client 都能發掘並呼叫 codebus 暴露的 tool，因此本次把 spike 轉成正式、可長駐的 MCP server，讓任何 MCP client 把一個 codebus vault 的 wiki 當成可查詢的知識來源。

## What Changes

- 新增 `codebus mcp --vault <path>` 子命令：以 stdio transport 啟動一個 **single-vault、query-only、只暴露 tools** 的 MCP server（一個 server 進程綁定一個 vault，跨專案由 MCP client 設定多條 server 條目達成）。
- 暴露三個 query-only tools（皆不收 vault/路徑參數，vault 在啟動時釘定）：
  - `wiki_list()` — 列出 vault wiki 所有頁面的 slug + title。
  - `wiki_read(slug, offset?, limit?)` — 讀取單一頁面正文，內建字元為單位的分頁（回傳 `has_more` / `next_offset`），因應 MCP tools/call 無內建分頁、且單 tool 輸出有 token 上限。
  - `wiki_search(query)` — 以關鍵字做大小寫不敏感子字串搜尋（RAG 尚未實作，以 grep 兜底），回傳命中頁面的 slug + title + 片段；tool description 明寫「傳關鍵字、非整句」。
- **Enabler refactor（行為不變）**：把桌面 App 的 wiki 讀取邏輯（頁面列舉、單頁讀取、slug 解析、frontmatter 剝離、目錄遞迴、寬容 frontmatter 解析）從 App 端抽到 codebus-core，供 MCP server 與既有 Tauri command 共用；Tauri command 改為薄包裝 core。
- rmcp 1.8.0 從 spike 的 dev-dependency 轉為正式相依，並以 `mcp` feature flag 控管（預設啟用），讓不需要 MCP 的精簡 build 可選擇排除。
- 安全邊界：server 只讀 `<vault>/.codebus/wiki/`，**絕不暴露 `raw/code/`（PII 去識別化鏡像）**；tools 不接受任意路徑；slug 解析沿用既有「遞迴比對 file_stem」機制，天然防 `../` 路徑穿越。
- 文件：README 加入 MCP client 接入設定範例，docs/security.md 加入 MCP 暴露面說明。

## Non-Goals (optional)

詳細的 Goals/Non-Goals 與被否決的替代方案見 design.md。本次明確排除：multi-vault（單一 server 服務多 vault，留 v2）、MCP resources/prompts（只做 tools）、任何寫操作（query-only）、RAG 語意搜尋（以 grep 兜底）、macOS/Linux 實機驗證（本次只驗 Windows，stdio 跨平台留 follow-up）。

## Capabilities

### New Capabilities

- `mcp-server`: codebus 以 stdio MCP server 形式對外暴露單一 vault wiki 的 query-only tools（transport、tool schema、single-vault 啟動行為、安全邊界、錯誤處理語意）。

### Modified Capabilities

- `cli`: 新增第九個 top-level 子命令 `mcp`（feature-gated、預設啟用）。Subcommand Registration 由八個擴為九個。

## Impact

- Affected specs: 新增 `mcp-server`；修改 `cli`（Subcommand Registration）。
- Affected code:
  - New:
    - `codebus-core/src/wiki/read.rs`（從 App 抽出的 wiki 讀取純函式 + `WikiPageMeta`）
    - `codebus-cli/src/commands/mcp.rs`（clap 子命令 + 啟動入口）
    - `codebus-cli/src/mcp/mod.rs`（MCP server 模組）
    - `codebus-cli/src/mcp/server.rs`（rmcp tool router 與三個 tool handler）
    - `codebus-cli/tests/mcp_server.rs`（spawn server + stdio client 的整合測試）
    - `openspec/specs/mcp-server/spec.md`（新 capability spec）
  - Modified:
    - `Cargo.toml`（workspace 加入 rmcp 相依）
    - `codebus-cli/Cargo.toml`（rmcp optional 相依 + `mcp` feature、tokio `io-std`）
    - `codebus-cli/src/main.rs`（feature-gated `Command::Mcp` 路由）
    - `codebus-cli/src/commands/mod.rs`（掛載 mcp 子模組）
    - `codebus-cli/tests/cli_routing.rs`（更新既有子命令註冊測試：mcp 不再 forbidden）
    - `codebus-core/src/wiki/mod.rs`（公開 `read` 子模組與型別）
    - `codebus-app/src-tauri/src/ipc/wiki.rs`（Tauri command 改薄包裝 core）
    - `README.md`（MCP client 接入設定範例）
    - `docs/security.md`（MCP 暴露面說明）
  - Removed: (none)
