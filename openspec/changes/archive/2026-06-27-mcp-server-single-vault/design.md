## Context

codebus 把陌生 codebase 整理成 `<repo>/.codebus/` 下的 Obsidian 相容 wiki vault。目前消費這份 wiki 的路徑有二：codebus CLI 的 `chat`/`query`/`quiz` 動詞，以及 Tauri 桌面 App 的 Wiki 頁面（`codebus-app/src-tauri/src/ipc/wiki.rs` 的 `list_wiki_pages` / `read_wiki_page`）。外部 agent（Claude Code、OpenAI Codex）無法直接查這份 wiki。

Phase 0 spike（2026-06-26、rmcp 1.8.0、stdio、tool-only）已 PASS：證明 rmcp 1.8.0 的宣告式巨集樣板可編可跑，Claude Code 與 codex 0.142.2 兩個 client 都能 `tools/list` 發掘並 `tools/call` 呼叫，對本 repo 真 vault 回傳 17 頁含 CJK title。本設計把 spike 轉成正式版。

**約束：**
- transport 鎖定 stdio；SDK 鎖定 rmcp 1.8.0（API 與 GitHub `main` 範例有漂移，見 Decisions）。
- single-vault：一個 server 進程綁定一個 `--vault`，跨專案由 MCP client 設定多條 server 條目達成。
- query-only、只暴露 tools（不做 resources/prompts、不做寫操作）。
- 安全：server 暴露給外部 agent，只能讀 `<vault>/.codebus/wiki/`，**絕不**觸及 `raw/code/`（PII 去識別化鏡像）。
- 現有兩個 wiki parser 並存：codebus-core `wiki::frontmatter::parse_page` 是**嚴格**驗證器（title/type/sources/goals/created/updated/related/stale 全必填，缺一即 `Err`），供 lint 用；App `wiki.rs` 的 `parse_frontmatter_yaml` 是**寬容**解析器（無 frontmatter 或缺欄位仍列頁），供頁面索引用。

## Goals / Non-Goals

**Goals:**

- 新增 `codebus mcp --vault <path>` 子命令，以 stdio 啟動 single-vault、query-only、tool-only 的 MCP server。
- 暴露 `wiki_list` / `wiki_read`（含分頁）/ `wiki_search`（grep）三個 tool。
- 把 App 的 wiki 讀取邏輯抽到 codebus-core 共用，**行為不變**，App 既有 wiki 測試維持綠。
- 安全邊界可驗證：tools 不收路徑、vault 釘定、只讀 `.codebus/wiki/`。
- rmcp 以 feature flag 控管，預設可用但可選排除。

**Non-Goals:**

- multi-vault（單一 server 服務多個 vault）— 留 v2。
- MCP resources / prompts — 只做 tools。
- 任何寫操作 — query-only。
- RAG / 向量語意搜尋 — 以 grep 子字串兜底，介面預留未來替換。
- macOS / Linux 實機驗證 — 本次只在 Windows 驗；stdio 跨平台留 follow-up。
- 超大 vault 的 `wiki_list` 分頁 — list 只回 slug+title（輕量），先不分頁。

## Decisions

### 抽取寬容 frontmatter 讀取邏輯至 codebus-core

把 App `wiki.rs` 的 `list_wiki_pages_impl` / `read_wiki_page_impl` / `find_page_by_slug` / `strip_frontmatter` / `walk_md_files` / `parse_frontmatter_yaml` 與 `WikiPageMeta` 抽到 codebus-core 新模組 `wiki::read`，回傳型別改用 `std::io::Error`（App `AppError` 已實作 `From<std::io::Error>`，薄包裝零摩擦）。

**為何不複用 core 既有 `parse_page`：** `parse_page` 是嚴格驗證器，對缺欄位的頁面回 `Err`。MCP `wiki_list` 與 App 索引都要「無 frontmatter / 缺欄位仍列頁」的寬容契約（App 既有測試 `list_wiki_pages_extracts_frontmatter_title` 明確涵蓋無 frontmatter 的 raw.md 仍列出）。若改用 `parse_page` 會漏頁並打破既有測試。因此保留嚴格 `parse_page` 給 lint，另抽寬容版供索引/MCP。

**App 既有測試維持綠的策略：** App `wiki.rs` 保留 `list_wiki_pages_impl` / `read_wiki_page_impl` 為**薄包裝**（呼叫 `codebus_core::wiki::read` 對應函式後 `map_err` 成 `AppError`），`WikiPageMeta` 改為 re-export 自 core（序列化欄位 slug/path/title/goals/updated 不變，Tauri 前端相容）。App 既有單元測試呼叫 `_impl` 的簽章不變，零改動維持綠；core 端另加等價單元測試。Obsidian URL 相關函式與測試**不**抽，續留 App。

替代方案：把測試一併搬到 core、App 不留測試 — 否決，因為任務要求「App 既有 wiki 測試維持綠」，保留薄包裝層測試最穩。

### rmcp 以預設啟用的 mcp feature flag 控管

在 workspace `Cargo.toml` 加 rmcp（版本 `1.8.0`，features：server、macros、transport-io、schemars）；codebus-cli 以 optional 方式引入 rmcp，定義 feature `mcp = ["dep:rmcp", "tokio/io-std"]` 並設 `default = ["mcp"]`。`codebus mcp` 子命令與其 `Command` variant、match arm 全部 `#[cfg(feature = "mcp")]` gate。

**為何 default-on 而非無 flag 直接進主 bin：** rmcp + schemars 是 query-only server 專屬相依，主 bin 的 goal/query/fix/chat/quiz 全用不到；optional 才符合「不灌大主 bin」。但設 default-on 讓本機安裝 CLI（fix 必需）零摩擦即得 `codebus mcp`；想要最小 binary 的 packager 可 `--no-default-features` 排除。tokio 已是相依，僅 stdio 所需的 `io-std` feature 由 flag 補上。

替代方案：(a) 無 feature flag、rmcp 永遠編 — 否決，違反不灌大主 bin。(b) feature 預設關閉 — 否決，使用者得記得加 `--features mcp` 才有子命令，摩擦高。

### 釘定 rmcp 1.8.0 並採用顯式 tool_router 樣板

server struct 帶 `tool_router: ToolRouter<Self>` 欄位、`fn new()` 內以 `Self::tool_router()` 初始化；`#[tool_router]` 標 tools impl、`#[tool(description = ...)]` 標每個 method、`#[tool_handler]` 標 `impl ServerHandler`。tool 參數型別用 `Parameters<T>`（`T: Deserialize + schemars::JsonSchema`，走 `rmcp::schemars` re-export）。`ServerInfo`（即 `InitializeResult`）在 1.8.0 是 `#[non_exhaustive]`，**不可** struct-literal（連 `..Default::default()` 也不行，E0639）；以先取 `ServerInfo::default()` 再 mutate `capabilities` 的方式構造。input schema 由 `JsonSchema` derive 全自動生成。

**為何釘定 1.8.0：** GitHub rust-sdk main 的範例已演進成 unit struct + `#[tool_router(server_handler)]` 自動生成 handler，領先 1.8.0，直接抄編不過。釘定版本讓編譯器當裁判，用 spike 已坐實的顯式樣板。

替代方案：抄 main 範例的精簡樣板 — 否決，與 1.8.0 API 不符。

### wiki_read 採字元為單位的 offset/limit 分頁

MCP `tools/call` 結果無內建分頁（MCP cursor 只用於 `*/list`），且單 tool 輸出對 client 有 token 上限（Claude 約 25k）。`wiki_read(slug, offset?, limit?)` 對**已剝除 frontmatter 的正文**以 Unicode 字元（`char`）為單位切片：`offset` 預設 0，`limit` 預設 12000、上限 clamp 至 20000。回傳 `{ slug, title, content, offset, next_offset, has_more, total_chars }`。

**為何用字元而非 byte 或行：** byte offset 會切斷 UTF-8 / CJK 多位元組字元產生亂碼；行為單位則受長行影響且 agent 心智模型較複雜。字元切片保證不切壞字元、語意單純。

**為何 limit 上限 20000 char：** CJK 最壞約 1 token/char，20000 char 約 20k token，在 25k 上限下留 buffer；英文約 0.25 token/char 更安全。預設 12000 兼顧單次往返涵蓋多數頁面與安全餘裕。

替代方案：(a) byte offset — 否決，切壞 CJK。(b) 自動 chunk 不暴露 offset — 否決，agent 無法控制續讀位置。

### wiki_search 採大小寫不敏感子字串搜尋並限制結果量

`wiki_search(query)` 對每頁 title + 正文做大小寫不敏感子字串比對（以小寫化的 query 作單一 needle，不做分詞），命中頁面回 `{ slug, title, snippet }`：snippet 取第一個命中位置前後約 ±100 字元（char boundary）。結果上限 20 頁、每頁 1 snippet；超過則 `truncated = true`。空白 query 回 `ErrorData`（invalid params）。tool description **明寫「傳關鍵字（如 authentication），非整句問題」**，因子字串比對對整句幾乎不命中。

**為何不分詞 / 不做 RAG：** RAG 未實作；MVP 以最可預期的單一子字串比對兜底，行為對 agent 完全透明。分詞 AND/OR 與語意檢索留 follow-up，介面（單一 `query` 字串）對未來替換為 RAG 相容。

替代方案：(a) 空白分詞 AND 比對 — 否決，MVP 增複雜度且行為較不可預期。(b) 不限結果量 — 否決，超大 vault 會爆 token。

### vault 釘定與只讀 .codebus/wiki 的安全邊界

vault 在 `--vault` 啟動時解析為絕對路徑並驗證 `<vault>/.codebus/wiki/` 存在且為目錄（否則啟動失敗、非零退出加 stderr 訊息）。所有 tool **不收** vault / 路徑參數，wiki root 在 server 建構時釘定。slug 解析沿用 `find_page_by_slug`（遞迴 walk 加比對 `file_stem`，slug 不參與路徑拼接，天然防 `../`）；另加 defense-in-depth：解析出的頁面路徑 canonicalize 後須仍位於 wiki root 之下。`raw/code/` 不在 wiki root 子樹內，永不會被 walk 命中。

替代方案：tool 收 `vault` 參數做 multi-vault — 否決，擴大攻擊面且屬 v2 範圍。

### 真錯誤回 rmcp ErrorData 並以 spawn_blocking 包裹阻塞 fs

區分「正常空結果」與「真錯誤」：`wiki_list` 無頁面、`wiki_search` 無命中回 `Ok`（空陣列）；`wiki_read` 的 slug 不存在、空白 query、底層 fs 讀取失敗回 `Err(ErrorData)`（不吞成空結果）。async tool handler 內的阻塞 fs（目錄遞迴、讀檔）以 `tokio::task::spawn_blocking` 包裹，避免卡住 runtime。所有 log / 診斷走 **stderr**（stdout 是 JSON-RPC 通道，污染會破壞協定）。

替代方案：錯誤一律回空結果 — 否決，掩蓋故障、agent 無從判斷是「真的沒有」還是「壞了」。

### MCP server 整合測試以 spawn 子程序加 stdio client 驗證

整合測試（`codebus-cli/tests/mcp_server.rs`）建一個臨時 vault（寫數頁 `.codebus/wiki/*.md`，含 CJK title、可搜尋關鍵字、一頁夠大以觸發分頁），spawn `codebus mcp --vault <tmp>` 子程序，透過子程序 stdin/stdout 以裸 newline-delimited JSON-RPC 走完 `initialize` 到 `tools/list` 到 `tools/call`（`wiki_list` / `wiki_read` / `wiki_search`），斷言真資料與分頁旗標、安全邊界（無 `raw/code` 洩漏）。

**為何裸 JSON-RPC 而非引入 rmcp client dev-dep：** cli 已有 serde_json，裸 JSON-RPC 零新增相依、貼近 spike 已驗證的驗證 client 路徑。實機接 Claude/Codex 由中樞另行驗，不在本測試。

## Implementation Contract

**Behavior（可觀察）：**
- `codebus mcp --vault <path>`：`<vault>/.codebus/wiki/` 存在時，啟動 stdio MCP server 並阻塞服務（log 走 stderr）；不存在時非零退出並在 stderr 說明。
- server 廣告 `tools` capability、不廣告 resources/prompts。`tools/list` 回 `wiki_list` / `wiki_read` / `wiki_search` 三個 tool，schema 由參數型別自動生成，`wiki_search` description 含「傳關鍵字非整句」。

**Interface / data shape：**
- 子命令參數：`McpArgs { vault: PathBuf }`（`--vault`，required）。
- `wiki_list()` 回 slug 與 title 陣列（依現有 `WikiPageMeta` 投影出 slug/title）。
- `wiki_read(slug: String, offset: Option<usize>, limit: Option<usize>)` 回 `{ slug, title, content, offset, next_offset: Option<usize>, has_more: bool, total_chars }`。
- `wiki_search(query: String)` 回 `{ results: [{ slug, title, snippet }], truncated: bool }`。
- core 新介面涵蓋四項行為：頁面列舉、單頁讀取（剝除 frontmatter 的正文）、slug 解析、frontmatter 剝離，連同 `WikiPageMeta` 型別，回傳以 `std::io::Error` 表錯（確切函式名以實作收斂）。

**Failure modes：**
- slug 不存在，`wiki_read` 回 `ErrorData`（invalid params 類）。
- 空白 query，`wiki_search` 回 `ErrorData`。
- vault wiki 目錄不存在，子命令啟動失敗（非零退出）。
- 單頁讀取 fs 失敗，tool 回 `ErrorData`，不靜默成空字串。
- list / search 無結果，回 `Ok` 空集合（非錯誤）。

**Acceptance criteria：**
- core 測試涵蓋 `wiki::read` 列舉/讀取/slug/剝離行為（含無 frontmatter 仍列頁、未知 slug 回錯）。
- 含 codebus-app 的全工作區測試，App 既有 wiki 測試全綠（薄包裝行為不變）。
- codebus-cli 的 `mcp_server` 整合測試跑通 initialize 到 tools/list 到 tools/call 三 tool，驗分頁 `has_more`/`next_offset` 與「`raw/code` 不可達」。
- codebus-cli 在 default features 與 `--no-default-features` 兩種 build 皆綠（feature gate 正確）。
- `cargo clippy --workspace` 無新增警告。

**Scope boundaries：**
- In scope：抽 core、`mcp` feature 加子命令、三個 query-only tool、安全邊界、`mcp-server` spec、上述測試、README 加 security.md 文件。
- Out of scope：multi-vault、resources/prompts、寫操作、RAG、macOS/Linux 實機、`wiki_list` 分頁、改動既有 verb 行為。

## Risks / Trade-offs

- [rmcp 1.8.0 與 GitHub main 範例 API 漂移，誤抄會編不過] → 釘定版本 `1.8.0`、採用 spike 已坐實的顯式 tool_router 樣板、`ServerInfo` 用 mut-default 構造。
- [default-on feature 讓預設 build 變大、CI 變慢] → rmcp 為 query-only 專屬，可 `--no-default-features` 排除；CI 影響為一次性建置成本，換取本機安裝零摩擦。
- [子字串搜尋對整句問題幾乎不命中，agent 體驗差] → tool description 明寫「傳關鍵字非整句」；介面預留未來換 RAG。
- [分頁字元上限對極端 CJK 仍可能逼近 token 上限] → 上限 20000 char 已對 1 token/char 最壞情況留 buffer；agent 可指定更小 limit。
- [抽 core 改動 App 共用碼，可能回歸] → 薄包裝保持 `_impl` 簽章與序列化欄位不變、App 既有測試維持綠當回歸網。
- [slug 路徑穿越] → file_stem 比對加 canonicalize 後須在 wiki root 之下雙重防護；`raw/code` 不在子樹內天然不可達。
