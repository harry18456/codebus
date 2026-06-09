# Backlog: codebus 作為 MCP Server（對外暴露 vault 操作）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** 擴充性 / 生態整合
**Owner:** harry
**Status:** parked

---

## 觀察

codebus app（C-F ship 後）會有一組穩定的 IPC commands，本質上是「vault 操作 API」。
透過 MCP transport 把這些 operations 對外暴露，任何 MCP-compatible 工具（Claude Code、MyCoder、Codex CLI）都能直接存取 codebus vault 的知識與功能。

兩個方向互補：
- **multi-provider**（backlog）：codebus 呼叫不同 AI agent
- **MCP server**（本條）：其他 AI 工具呼叫 codebus

codebus 變成個人知識管理的中樞，而不只是一個 standalone app。

## Proposed operations（v1 query-only）

v1 只做讀取 / 查詢，不暴露寫入（避免外部工具亂改 wiki）。

| MCP Tool | 說明 | 後端對應 |
|----------|------|---------|
| `vault_list` | 列出所有已登記 vault 路徑 | config `vaults[]` |
| `wiki_read` | 讀取指定 wiki page 內容 | `Read(wiki/pages/<slug>.md)` |
| `wiki_search` | 語意搜尋 wiki（需 RAG index） | `rag::search()` |
| `wiki_index` | 列出 vault 所有 wiki pages + frontmatter | `wiki/index.md` |
| `run_list` | 列出近期 goal run 紀錄（outcome / summary） | RunLog |

未來 v2 可選加：
- `goal_spawn`：啟動 goal（帶 prompt）
- `chat_turn`：發送 chat turn

## 技術設計

### Transport

**stdio-based MCP server**（最簡）：
- `codebus mcp-serve` 啟動，走 stdin / stdout JSON-RPC
- 任何工具在 MCP config 加一行即可接入
- 不需要開 port，無 auth 問題（local process）

```json
// Claude Code MCP config（示意）
{
  "mcpServers": {
    "codebus": {
      "command": "codebus",
      "args": ["mcp-serve", "--vault", "/path/to/vault"]
    }
  }
}
```

### MCP protocol layer

- `rmcp` crate（Rust MCP SDK）或自行實作 JSON-RPC 2.0
- Tool schema 對應上表 operations
- `wiki_search` 依賴 RAG index（若 index 不存在則回傳 error / fallback to grep）

### Tasks（粗估）

1. spec ADDED `mcp-server`：定義 tool schema + transport 規格
2. `rmcp` 或 JSON-RPC 層整合
3. 各 operation 實作（對應既有 codebus_core 函數）
4. `codebus mcp-serve` CLI command
5. `wiki_search` 與 RAG index backlog 整合（先用 grep fallback）
6. Integration test：Claude Code / MyCoder 接入 smoke test

工程量：中-重（3-5 個半天；主要是 MCP protocol 層 + `wiki_search` 依賴 RAG）。

## Out of scope

- HTTP-based MCP server（v1 stdio 即可）
- 寫入 operations（write / edit wiki、modify config）— v1 read-only
- Multi-vault 同時 serve（v1 single vault per server instance）
- Auth / token（stdio local process，不需要）

## 依賴

- **after F**：IPC surface 穩定後再 expose
- **RAG index** backlog：`wiki_search` 需要；若 RAG 未做則 grep fallback
- 與 RAG backlog 可同批 propose（兩條自然配對）

## 何時動

F archive 之後，與 RAG index 同批評估。
若 RAG 未準備好，可先 ship 不含 `wiki_search` 的版本（其他 operations 不依賴 RAG）。
