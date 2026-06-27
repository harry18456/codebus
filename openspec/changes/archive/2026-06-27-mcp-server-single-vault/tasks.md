## 1. 抽取 wiki 讀取邏輯至 codebus-core（enabler refactor，行為不變）

- [x] 1.1 在 codebus-core 新增 `wiki::read` 模組，把頁面列舉、單頁讀取（剝除 frontmatter 的正文）、slug 解析、`strip_frontmatter`、`walk_md_files`、寬容 `parse_frontmatter_yaml` 與 `WikiPageMeta` 自 App 抽出，錯誤型別用 `std::io::Error`；落實設計決策「抽取寬容 frontmatter 讀取邏輯至 codebus-core」（保留嚴格 `parse_page` 給 lint、不複用）。驗證：`cargo test -p codebus-core` 涵蓋「無 frontmatter 仍列頁、slug 作 title fallback、空 root 回空、`strip_frontmatter` 處理 CRLF、未知 slug 回 None」皆綠。
- [x] 1.2 App `codebus-app/src-tauri/src/ipc/wiki.rs` 的 `list_wiki_pages_impl` / `read_wiki_page_impl` 改為薄包裝 `codebus_core::wiki::read` 並 `map_err` 成 `AppError`，`WikiPageMeta` 與被既有測試引用的 `strip_frontmatter` / `find_page_by_slug` 改 re-export 自 core（序列化欄位 slug/path/title/goals/updated 不變、`_impl` 簽章不變）。行為不變、Tauri 前端契約相容。驗證：App 既有 wiki 單元測試（list/read/strip 那批）零改動，`cargo test`（含 codebus-app）維持綠。

## 2. rmcp 相依與 `codebus mcp` 子命令骨架

- [x] 2.1 在 workspace `Cargo.toml` 與 `codebus-cli/Cargo.toml` 加入 rmcp（optional）並定義 `mcp` feature（`default = ["mcp"]`、含 `tokio/io-std`），`codebus mcp` 子命令與其路由以 `#[cfg(feature = "mcp")]` gate；落實設計決策「rmcp 以預設啟用的 mcp feature flag 控管」。驗證：`cargo build -p codebus-cli`（default）與 `cargo build -p codebus-cli --no-default-features` 皆綠。
- [x] 2.2 新增 visible 的 feature-gated `codebus mcp --vault <path>` 子命令（clap `McpArgs { vault }`，於 `main.rs` 與 `commands/mod.rs` 以 `#[cfg(feature = "mcp")]` 掛載），啟動時驗證 `<vault>/.codebus/wiki/` 存在且為目錄、否則非零退出加 stderr 訊息；註冊為第九個 top-level 子命令，落實 cli spec「Subcommand Registration」MODIFIED（八→九含 mcp）與 spec「Single-vault stdio MCP server lifecycle」。同步更新既有 `codebus-cli/tests/cli_routing.rs`：`help_lists_exactly_the_six_subcommands` 的 forbidden 清單移除 mcp、`mcp_subcommand_is_rejected_specifically` 改為驗證 `codebus mcp --vault <missing>` 非零退出、子命令列舉測試納入 mcp。驗證：`cargo test -p codebus-cli --test cli_routing` 全綠，`codebus --help` 列出 mcp。

## 3. MCP server 與 tools 實作

- [x] 3.1 以 rmcp 1.8.0 顯式 `tool_router` 樣板（struct 帶 `tool_router: ToolRouter<Self>` 欄位、`#[tool_router]` / `#[tool]` / `#[tool_handler]`、`ServerInfo` 以 mut-default 構造、log 全走 stderr）建立只廣告 tools 的 stdio server，並以 `serve(stdio())` 啟動；落實設計決策「釘定 rmcp 1.8.0 並採用顯式 tool_router 樣板」與 spec「Tools-only query surface without path parameters」。驗證：整合測試 `tools/list` 回三個 tool、皆無 path 參數、不廣告 resources / prompts。
- [x] 3.2 實作 `wiki_list` tool（回每頁 slug + title，寬容無 frontmatter，阻塞讀檔走 `spawn_blocking`）；落實 spec「wiki_list returns the page index」。驗證：整合測試對真 vault 回含「無 frontmatter 頁」、空 vault 回空陣列（success）。
- [x] 3.3 實作 `wiki_read` tool 的字元分頁（`offset` 預設 0、`limit` 預設 12000、clamp 至 20000、char boundary 切片，回 `content` / `offset` / `next_offset` / `has_more` / `total_chars`，並剝除 frontmatter）；落實 spec「wiki_read returns the paginated page body」與設計決策「wiki_read 採字元為單位的 offset/limit 分頁」。驗證：整合測試斷言 spec 的分頁 boundary 表（含 clamp 與末段 `has_more=false`）、剝除 frontmatter、未知 slug 回 MCP error。
- [x] 3.4 實作 `wiki_search` tool（大小寫不敏感單一子字串比對 title + body、命中回 slug + title + snippet、結果 cap 20 頁加 `truncated`、空白 query 回 error，tool description 明寫「傳關鍵字非整句」）；落實 spec「wiki_search performs keyword substring search」與設計決策「wiki_search 採大小寫不敏感子字串搜尋並限制結果量」。驗證：整合測試涵蓋命中 / 未命中（空 success）/ 空白 query（error）/ 結果上限與 `truncated`。

## 4. 安全邊界與錯誤語意

- [x] 4.1 釘定 wiki root、所有 tool 不收路徑、slug 以 `file_stem` 比對加「canonicalize 後須位於 wiki root 之下」的雙重防護，確保只讀 `<vault>/.codebus/wiki/`、`raw/code/` 不可達；落實 spec「Read-only security boundary」與設計決策「vault 釘定與只讀 .codebus/wiki 的安全邊界」。驗證：整合測試以 traversal slug 與模擬 `raw/code` 路徑樣式呼叫，皆無法讀到 wiki 子樹外檔案。
- [x] 4.2 區分真錯誤與空結果（未知 slug / 空白 query / fs 讀取失敗回 `ErrorData`；`wiki_list` / `wiki_search` 無結果回 Ok 空集合），所有阻塞 fs 一律 `spawn_blocking`；落實 spec「Error-versus-empty semantics and non-blocking filesystem access」與設計決策「真錯誤回 rmcp ErrorData 並以 spawn_blocking 包裹阻塞 fs」。驗證：整合測試斷言 fs 失敗 surface 為 MCP error、無結果為 success（兩者可被 client 區分）。

## 5. 整合測試

- [x] 5.1 新增 `codebus-cli/tests/mcp_server.rs`：建臨時 vault（寫數頁 `.codebus/wiki/*.md`，含 CJK title、可搜尋關鍵字、一頁夠大以觸發分頁），spawn `codebus mcp --vault <tmp>` 子程序，以裸 newline-delimited JSON-RPC 走完 `initialize` 到 `tools/list` 到 `tools/call`（三個 tool）；落實設計決策「MCP server 整合測試以 spawn 子程序加 stdio client 驗證」。驗證：`cargo test -p codebus-cli --test mcp_server` 綠，涵蓋三個 tool 與分頁 / 安全斷言。

## 6. 文件

- [x] 6.1 [P] 在 README 加入 MCP client 接入設定範例（一個 server 條目綁定一個 `--vault`、跨專案配多條），說明 single-vault / query-only 定位。驗證：內容 review，範例可被 Claude Code / Codex 的 MCP config 直接套用。
- [x] 6.2 [P] 在 docs/security.md 加入 MCP 暴露面說明（只讀 `<vault>/.codebus/wiki/`、`raw/code/` 去識別化鏡像不外流、tools 不收路徑）。驗證：內容 review 並與既有 §5 隔離姿態一致。

## 7. 驗收

- [x] 7.1 全工作區回歸與品質閘：`cargo test`（含 codebus-app 既有 wiki 測試）、`cargo test -p codebus-core`、`cargo test -p codebus-cli`、`cargo clippy --workspace`。驗證：四項皆綠且無新增 clippy 警告。
