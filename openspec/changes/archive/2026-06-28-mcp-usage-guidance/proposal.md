## Why

註冊 MCP 只讓 codebus 的 wiki 工具「可用」，實務上卻不會被用——agent 在任意 repo 開工時不會主動 vault_list，也分不清何時該查 wiki。真正的用途是「跨專案 wiki 參考庫」：使用者累積多個 codebase 的 wiki，工作時讓 agent 跨庫查既有理解（套用某 codebase 的做法、跨專案參考）。要讓它真的被 claude code 與 codex 用起來，需兩件事：(A) tool descriptions 講清「這是什麼、何時 reach for」，(B) 啟用某 client 的 MCP 時順帶把使用指引寫進該 client 的全域指令檔、停用時移除。

## What Changes

- **A — tool descriptions 補「何時用」**：vault_list / wiki_list / wiki_search / wiki_read 的描述加入「這是你建過的跨專案 codebase wiki 參考庫、何時該用它（套用某 codebase 的做法、跨專案參考、被要求時）、當參考用並回 source 驗」的框架，不只描述機制。
- **B — 啟用即寫 global md guidance（claude code 與 codex 皆支援）**：啟用某 client 的 MCP 整合時，於該 client 的全域指令檔以標記式 managed block 寫入一段 wiki 使用指引；停用時移除該標記塊。
  - 路徑：claude 寫 ~/.claude/CLAUDE.md（honor CLAUDE_CONFIG_DIR）、codex 寫 ~/.codex/AGENTS.md（honor CODEX_HOME），兩者皆已實機確認。
  - 安全：以 codebus:mcp:start / codebus:mcp:end 標記包住，啟用＝冪等 upsert（重複啟用不疊第二塊）、停用＝只刪該塊並收斂前後空行、標記塊以外一字不動、原子寫（temp+rename）、檔案不存在就建。
  - 揭露：Settings 該 client 列的文案明寫「啟用會同時在你的 <client> 全域指令加入一段（停用會移除）」。
  - 失敗處理：MCP 註冊為權威；global md 寫入失敗為非致命（surface 警告、不回滾註冊、原子寫保證不半寫壞檔）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `mcp-server`: 新增 requirement——4 個 query tool 的 description 傳達「跨專案 wiki 參考庫」定位與何時使用（A）。
- `mcp-client-install`: 新增 requirement——install / remove 連帶 upsert / remove 該 client 全域指令檔的標記式 guidance block（B），含冪等、對稱移除、per-client 路徑解析、原子寫、Settings 揭露文案、非致命失敗。

## Impact

- Affected specs: mcp-server, mcp-client-install（皆新增 requirement，無移除）
- Affected code:
  - Modified:
    - codebus-cli/src/mcp/server.rs — 4 個 tool 的 description 補「何時用 / 跨專案參考」框架（A）；既有「keyword 非整句」規範保留
    - codebus-app/src-tauri/src/ipc/mcp_install.rs — install 成功註冊後 upsert guidance block、remove 後移除（B）；非致命失敗處理
    - codebus-app/src/components/settings/McpIntegrationSection.tsx — 加揭露文案（t key）
    - codebus-app/src/i18n/messages.ts — 揭露文案 i18n（zh + en）
  - New:
    - codebus-app/src-tauri/src/ipc/global_md.rs — 標記式 block upsert / remove + per-client 全域 md 路徑解析（honor CLAUDE_CONFIG_DIR / CODEX_HOME）+ block 內容常數
  - Removed: (none)
