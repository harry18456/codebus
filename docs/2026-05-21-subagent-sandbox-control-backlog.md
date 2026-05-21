# Backlog: 確認 swap 對 subagent 的控制與限制（驗證測試）

**Date:** 2026-05-21
**Surfaced during:** discuss 2026-05-21（MCP 隔離實測後延伸——同類 sandbox 邊界問題）
**Severity:** security 驗證缺口（未知 = 風險，需實測釐清）
**Owner:** harry
**Status:** resolved — 實測確認安全，無需修補（2026-05-21）

---

## 驗證結果（2026-05-21 實測）

用 codebus 完全相同的 spawn flag（含 `spawn-mcp-isolation` 後的 `--strict-mcp-config --mcp-config '{"mcpServers":{}}'`）跑 `claude -p`，看 init 事件的 `tools` 清單：

- **query / chat toolset**（`--tools Read,Glob,Grep`）→ session tools = `Glob, Grep, LSP, Read`。**無 Task**。
- **goal toolset**（`--tools Read,Glob,Grep,Write,Edit`）→ session tools = `Edit, Glob, Grep, LSP, Read, Write`。**無 Task**。

agent 自述「NO TASK TOOL」，並指出 `agents.md` 定義的 planner / code-reviewer 等 subagent 需要 Task 工具才能啟動，而 session 未提供。

**結論**：跟 MCP（`--tools` 擋不住、會洩漏）**相反**，`Task` 是內建工具，`--tools` 沒列就**確實排除**。codebus spawn 出來的 agent 根本拿不到 Task → 無法啟動任何 subagent → 不存在「subagent 繞過 toolset / MCP 隔離」的途徑。本軸的 sandbox 成立，**無需修補**。

附帶觀察（非漏洞）：`--tools Read,Glob,Grep` 實際 init 多出 `LSP`（claude 自動帶的唯讀語言工具）。良性、與 subagent 無關，但代表 codebus 的 toolset 不完全等於它明列的清單——若日後要嚴格鎖定，可另議（非本條範圍）。

## 控制性（2026-05-21 實測延伸 — 「能否使用 / 能否控制 subagent」）

承上，原始問題其實是雙向的：除了「會不會逃逸」，也包含「codebus 能不能**主動啟用並控制** subagent」。實測結論：**完全可控、可用**。

- **能否使用**：可以。subagent 現在關著純粹因為 `Task` 不在 codebus 的 toolset。實測在 `--tools` 加上 `Task` → init tools 出現 `Task`、模型成功透過 `Agent` tool（`subagent_type`）啟動 subagent。
- **控制「有哪些 subagent」**：以 spawn cwd（= vault root）下的 `.claude/agents/<name>.md` 定義為準。實測在 temp vault 放一個 `reader`（`tools: Read`）→ 模型以 `subagent_type: 'reader'` 啟動該自訂 agent、讀檔並正確回傳內容；未放定義時 fallback 到內建 `general-purpose`。codebus 可藉「ship 哪些 agent 定義進 vault」控制可用集合。
- **控制「每個 subagent 的工具 / 模型」**：各 agent frontmatter 的 `tools:` / `model:`。

**仍未驗證（若日後要啟用 Task 必須先確認）**：被啟動的 subagent 是否**繼承** parent 的 `--strict-mcp-config`（MCP 隔離）與 `--tools` 上限。這從 parent 的 stream-json 不易觀察（subagent 的 init 不會 surface 到 parent stream）。若不繼承，啟用 Task 可能在 subagent 層重開 MCP 漏洞——啟用前需專門測這條。

**現況決策**：codebus 維持 subagent 關閉（toolset 不含 Task），符合當前「單一受限 agent」的 sandbox 模型。若未來要引入受控 subagent（例如平行化、專職 reviewer），再起獨立 change，並把「subagent 是否繼承 MCP/tool 隔離」列為該 change 的首要驗證項。

---

## 觀察

codebus 用「swap」方式 spawn 本機 `claude -p`（`codebus-core/src/agent/claude_cli.rs::build_claude_cmd`），靠 `--tools` / `--allowedTools` 把工具集鎖成明確清單建立 sandbox。現有 toolset（grounded）：

- `GOAL_TOOLSET = Read, Glob, Grep, Write, Edit`
- `QUERY_TOOLSET` / `CHAT_TOOLSET` / `GOAL_VERIFY_TOOLSET = Read, Glob, Grep`
- **皆不含 `Task`**（Claude Code 開 subagent 的工具）

直覺上「沒列 Task → 不能開 subagent」，但 2026-05-21 的 MCP 實測給了教訓：**`--tools` 只管內建工具、對 MCP 工具無效**，「沒列就擋得住」的假設在 MCP 上是錯的。因此 subagent 這條同樣不能憑直覺，必須實測：

1. **`Task` 是否真被 `--tools` 排除？** Task 是內建工具，理論上沒列就不該出現在 spawn session 的工具集——但要看 init 事件的 `tools` 實際清單確認（比照 MCP 測法）。
2. **若 subagent 真能被開**（不論透過 Task 或其他機制），subagent 是否**繼承** parent 的 `--tools` / `--allowedTools` / `--permission-mode` / `--strict-mcp-config`？還是拿到**預設全量工具集 + ambient MCP**，等於逃出 codebus sandbox？
3. **vault 內的 subagent 定義**（`.codebus/.claude/agents/*.md` 或 repo 既有 `.claude/agents/`）是否會被 spawn session 採用、並可能授予比 codebus toolset 更寬的工具？

若 (2)/(3) 答案是「subagent 不繼承限制」，那就是跟 MCP 同級的 sandbox 漏洞——codebus 宣稱的唯讀/受限保證會被 subagent 繞過。

## Proposed approach（實測，比照 MCP 那次）

不要只讀文件，要 grounded 實測：

1. **Task 可見性**：用完全複製 codebus flag 的 `claude -p`（含 `--strict-mcp-config`）spawn，看 init 事件的 `tools` 陣列有沒有 `Task`。
2. **強制嘗試開 subagent**：prompt 明確要求用 Task 開 subagent，觀察 stream——是被擋（工具不存在）還是真的 spawn。
3. **若能 spawn → 查繼承**：讓 subagent 嘗試一個 parent 被禁的動作（例如 query sandbox 下試 `Write`，或試 ambient MCP 工具），看 subagent 是否拿得到。確認 subagent 的 toolset / MCP 隔離是否沿用 parent。
4. **subagent 定義來源**：確認 `.claude/agents/` 在 spawn cwd（vault root）下是否被載入、其 `tools:` frontmatter 是否能放寬限制。

依結果決定是否需要修補（例：明確 `--disallowedTools Task`、或在 spawn 前確保 subagent 定義不放寬、或文件化「subagent 不在 sandbox 保證內」）。

## Tasks（粗估）

1. 寫實測腳本：codebus flag + Task prompt，收 init `tools` 與 stream（一次性，比照 MCP 測法）
2. 判讀：Task 是否可見、subagent 是否可 spawn、是否繼承限制、`.claude/agents/` 是否影響
3. 結論文件化：若安全 → 記錄為「已驗證」；若有漏 → 起 fix change（可能加 `--disallowedTools` 或調整 spawn）
4. （條件性）若需修補：比照 `spawn-mcp-isolation`，修在 `build_claude_cmd` 單點

工程量：輕（實測+判讀半天；若需修補另計，預估輕）。

## Out of scope

- 實作層的修補本身（先驗證，確認有漏再起 fix change）
- 重寫 toolset / verb 編排
- MCP 隔離（已由 `spawn-mcp-isolation` 解決；本條是 subagent 軸）

## 何時動

無硬依賴，可隨時做。優先序：屬「未知安全邊界」，比一般 UX backlog 高一階——尤其若日後要對外發或讓別人用 codebus，subagent 能否逃 sandbox 是必須先答清楚的問題。建議在 `spawn-mcp-isolation` 同一波 sandbox 盤點裡順手做掉。
