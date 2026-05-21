## Problem

codebus 透過 spawn 本機 `claude` CLI（`codebus_core::agent::claude_cli::build_claude_cmd`）來跑 goal / query / fix / chat / quiz 等 verb，並以 `--tools` / `--allowedTools` 把工具集鎖成唯讀或受限的明確清單（例：query/chat 的 `Read,Glob,Grep`）。設計意圖是「spawn 出來的 codebus agent 跑在受限 sandbox」。

但實測（2026-05-21，本機 claude + 一個本地 filesystem MCP server + claude.ai 連接器）證明：`--tools` / `--allowedTools` **只管內建工具，對 MCP 工具無效**。在完全複製 codebus flag 的 `claude -p` spawn 下，init 事件實際暴露的工具集包含使用者 user-scope / 連接器層的 MCP 工具（8 個 claude.ai 連接器 authenticate 工具），即使 `--tools` 只列 `Read,Glob,Grep`。也就是：使用者若設定了 MCP（特別是已認證的連接器），codebus spawn 出來的 agent 看得到也能呼叫那些 MCP 工具——這超出 codebus toolset sandbox 的宣告範圍。

## Root Cause

`build_claude_cmd` 組 argv 時只下 `--tools` / `--allowedTools` / `--permission-mode` / `--output-format stream-json --verbose`，**沒有任何 MCP 載入層的隔離旗標**（無 `--strict-mcp-config`、無 `--mcp-config`）。因此 claude 走它預設的 MCP 解析：user-scope（`~/.claude.json`）與連接器層的 MCP server 會被載入、其工具會註冊進 session。toolset 旗標位於「工具呼叫授權層」且只認內建工具，無法在「載入層」攔下 MCP。

## Proposed Solution

在唯一的 spawn chokepoint `build_claude_cmd`（`codebus-core/src/agent/claude_cli.rs`）加上 MCP 載入層硬隔離：spawn `claude` 時一律帶 `--strict-mcp-config` 並指向一份空的 MCP 設定（`--mcp-config`，內容等同 `{"mcpServers":{}}`），使任何 ambient MCP（user-scope / project `.mcp.json` / 連接器）都不被載入。

因為 CLI 與 app 兩端都匯流到 codebus-core 的 verb → `agent::invoke` → `build_claude_cmd`（production 環境唯一 spawn `claude` 的地方），單點修改即同時對 CLI 與 app、且對所有 verb（含 goal content-verify 的二次 spawn）一致生效，不需於各 surface 重複。

實測（2026-05-21）已驗證：在相同 flag 上追加 `--strict-mcp-config --mcp-config <empty>` 後，init 事件的 `mcp__*` 工具清單為空、`mcp_servers` 為空，內建工具集（`Read,Glob,Grep` 等）不受影響。

## Non-Goals

- 不提供「允許特定 MCP server / 工具」的設定或 allowlist —— solo dev 階段 codebus agent 無消費外部 MCP 的需求，YAGNI；隔離為全有全無、無 escape hatch。
- 不改 `--tools` / `--allowedTools` 既有內建工具閘的語意與內容。
- 不改 codebus 自身的 `codebus mcp`（codebus 作為 MCP server）功能 —— 那是 codebus 對外暴露，與本 change（codebus 消費外部 MCP）無關。
- 不改 verb 編排、RunLog、stream 解析、resume session 等其他 spawn 行為。
- 不在 CLI / app surface 各自加旗標（修在共用 chokepoint 即涵蓋）。

## Success Criteria

- `build_claude_cmd` 產出的 argv 一律包含 `--strict-mcp-config` 與指向空設定的 `--mcp-config`（對 goal / query / fix / chat / quiz / verify 所有 toolset 與 model/effort 組合皆成立）。
- 既有 `--resume` 順序不變式維持：`--resume <id>`（當 `resume_session_id` 為 `Some`）仍位於 `--tools` 之前；新增的 MCP 旗標不破壞此既有契約。
- 單元測試斷言 argv 含上述兩個 MCP 旗標、且空 MCP 設定的內容/路徑正確；既有 argv 形狀測試（如 resume 缺省「byte-equivalent argv」斷言）相應更新為含新旗標的新基準。
- 既有 CLI 整合測試（mock_claude 路徑）與 app crate 測試不因新增旗標而失敗。

## Impact

- Affected specs: verb-library（新增一條 Agent Spawn MCP Isolation requirement）
- Affected code:
  - Modified: codebus-core/src/agent/claude_cli.rs（`build_claude_cmd` 加 MCP 隔離旗標 + 對應單元測試 + 更新既有 argv 形狀斷言）
  - Modified: openspec/specs/verb-library/spec.md（archive 時由 spec delta 套用）
