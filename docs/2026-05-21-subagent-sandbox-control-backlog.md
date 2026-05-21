# Backlog: 確認 swap 對 subagent 的控制與限制（驗證測試）

**Date:** 2026-05-21
**Surfaced during:** discuss 2026-05-21（MCP 隔離實測後延伸——同類 sandbox 邊界問題）
**Severity:** security 驗證缺口（未知 = 風險，需實測釐清）
**Owner:** harry
**Status:** open

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
