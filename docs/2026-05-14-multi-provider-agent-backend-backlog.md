# Backlog: multi-provider agent backend（Codex CLI + Azure endpoint）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（架構擴充性）
**Owner:** harry
**Status:** unblocked — 2026-05-20 codex CLI 0.132.0 spike 確認 contract 完整、second-impl 條件滿足

---

## 2026-05-20 更新：codex 0.132.0 spike 結果（contract 完整）

2026-05-20 同日連跑兩家 agentic CLI spike：

- `agy` 1.0.0（Antigravity，2026-05-19 上市）— 缺 `--tools` 白名單、無 `--output-format stream-json`、`-p` mode 看不到 agentic tool loop 證據。**不適合作為 second-impl 對標**
- `codex` 0.132.0（OpenAI Codex CLI）— **所有 codebus 需要的 contract 都有**，部分還比 Claude CLI 更乾淨

### Codex 0.132.0 seam 對映表（vs Claude）

| Seam | Claude CLI | Codex 0.132.0 | 整合工作 |
|---|---|---|---|
| Non-interactive | `claude -p` | `codex exec` | 直接對映 |
| Structured stream | `--output-format stream-json --verbose` | **`--json`** | 寫第二支 parser |
| Tool call event in non-interactive | ✓ ToolUse + ToolResult | ✓ `item.started/completed type:command_execution` 一對 | 映成 `ToolUse + ToolResult` |
| Session resume | `--resume <id>` | `resume`/`fork` 子命令 | 直接對映 |
| Sandbox 模型 | `--permission-mode acceptEdits`（單級） | **`--sandbox read-only/workspace-write/danger-full-access`**（三級，比 Claude 細）| chat → `read-only`，goal/fix → `workspace-write` |
| Approval policy | implicit via permission-mode | **`--ask-for-approval untrusted/on-request/never`** | 多一個顯式控制軸 |
| Hook system | `.claude/settings.json` PreToolUse | **存在**（`--dangerously-bypass-hook-trust` flag + `.rules` execpolicy）| `.codex/` 或同等位置寫 hook config |
| Skill bundle format | `.claude/skills/<name>/SKILL.md`（yaml frontmatter + md） | **`~/.codex/skills/<name>/SKILL.md`** —— 完全相同 yaml + md 格式 | **共用內容、雙寫 `.codebus/.claude/skills/` 跟 `.codebus/.codex/skills/`** |
| Plugin system | ✗ | **`.codex-plugin/plugin.json` + marketplace** | 可選，先用 skill 不走 plugin |
| MCP | ✗ | **client + server 全支援** | 未來可選 |
| Schema constraint | ✗ | **`--output-schema <FILE>`**（codex 獨有）| 未來 quiz/goal verify 可利用 |
| Token usage | input/output/cache_read/cache_create | input/output/**cached/reasoning_output**（更乾淨）| `TokenUsage` 已有 `reasoning_tokens` 欄位，直接對應 |

### Stream event shape 實測

`codex exec --json --sandbox read-only "list files... then say done"` 輸出：

```jsonl
{"type":"thread.started","thread_id":"019e4574-..."}      ← session_id
{"type":"turn.started"}
{"type":"item.started","item":{"type":"command_execution","command":"powershell ...","status":"in_progress"}}
{"type":"item.completed","item":{"type":"command_execution","aggregated_output":"...","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"type":"agent_message","text":"done"}}
{"type":"turn.completed","usage":{"input_tokens":30515,"cached_input_tokens":22272,"output_tokens":43,"reasoning_output_tokens":0}}
```

對映到既有 `StreamEvent`：

| Codex event | codebus `StreamEvent` |
|---|---|
| `thread.started.thread_id` | session_id 抽取點（chat verb 用） |
| `item.started type:command_execution` | 可選 skip（只是 intent，不必 emit） |
| `item.completed type:command_execution` | `ToolUse {name:"Bash", input:{command}} + ToolResult {output:aggregated_output, is_error: exit_code != 0}` 一對 |
| `item.completed type:agent_message` | `Thought { text }` |
| `turn.completed.usage` | `Usage(TokenUsage)` |

### 工程量重估

原 backlog 估「重（1 週以上；spike 結果影響估算）」，spike 完後估**約 1-2 週**：

- 不是「重做」，是「加 `CodexBackend` impl + `parse_codex_stream_line` + skill bundle 雙寫 + config schema 加 codex profile + agent::invoke routing」
- skill bundle 完全共用（同 yaml frontmatter + md 格式），雙寫成本只是 `vault/init.rs` 多 copy 一次
- sandbox 對映比 Claude 還乾淨（chat `read-only` 是 codex 原生 primitive，不需 prompt-layer defense-in-depth）

### 重要校準（vs 早期 framing）

之前 backlog 寫「codebus 一直以來定位為 multi-AI-provider 工具，但目前實作完全硬耦合到 `claude` binary」—— **這個 framing 不完整**。實際上：

- 資料層（`StreamEvent` / `TokenUsage` / `RunLog`）在 `v3-run-log-events`（Stage 2）已 normalized，**只剩一支 `parse_claude_stream_line` 是 Claude 專屬**
- 真正卡的不是「codebus 抽象不夠」，是「過去沒有合格的 second-impl 對標目標」
- agy 不合格（contract 不完整）、codex 合格（contract 完整）

### 何時動（更新）

原列「v3-app-polish-ship（F）之後」—— user 2026-05-20 明確 deprioritize polish-ship，本條跟 polish-ship 沒有硬依賴順序，**可在 user 自己想動時起 `/spectra-propose`**。

---

## 觀察

codebus 一直以來定位為 multi-AI-provider 工具，但目前實作完全硬耦合到 `claude` binary：

```rust
// codebus-core/src/agent/invoke.rs（示意）
// 直接 spawn "claude" binary，假設 --output-format stream-json
// VerbEvent 對應 Claude 專屬 event schema
```

OpenAI 於 2025 年 4 月發布 **Codex CLI**——一個 terminal-based coding agent（類似 Claude Code 但底層是 GPT-4o / o3 / o4-mini）。要支援 Codex 需要在 `codebus_core` 引入 provider 抽象層。

Azure OpenAI 是 Codex 的 enterprise deployment variant（相同 binary，不同 endpoint + auth config）。

## Proposed fix

新提一條 change：`v3-multi-provider`

### AgentBackend trait

```rust
// codebus-core/src/agent/backend.rs（示意）
pub trait AgentBackend: Send + Sync {
    fn spawn(&self, opts: SpawnOpts) -> Result<AgentHandle>;
    fn event_schema(&self) -> EventSchema;
}

pub struct ClaudeBackend { /* 現有邏輯搬過來 */ }
pub struct CodexBackend  { endpoint: Option<Url>, model: CodexModel }
```

- `VerbEvent` 需要標準化（或 backend 各自 emit normalized event）
- `codebus-app` IPC 層不感知 backend 差異

### Codex CLI 差異點

| 面向 | Claude CLI | Codex CLI |
|------|-----------|-----------|
| Output format | `--output-format stream-json` | 不同 event schema（需查文件）|
| 工具白名單 | `--tools Read,Glob,...` | 不同 flag |
| Sandbox | `--disallow-tools` / cwd 隔離 | 不同機制 |
| Auth | `~/.claude` config | `OPENAI_API_KEY` env var |

### Azure variant

只需在 `CodexBackend` 加 `endpoint: Option<Url>` config 欄位，指向 Azure OpenAI endpoint。
Auth 改用 Azure AD token 或 API key，binary 相同。

### Tasks（粗估）

1. spec ADDED `multi-provider`：定義 `AgentBackend` 介面 + event normalization 規格
2. Spike：Codex CLI event schema 研究（確認 stream-json 等價格式）
3. `codebus-core/src/agent/backend/`：trait + ClaudeBackend 搬遷
4. `codebus-core/src/agent/backend/codex.rs`：CodexBackend 實作
5. Config schema 加 `agent.provider: claude | codex`、`agent.codex.endpoint: Option<Url>`
6. Settings UI 新增 provider 選擇（也可 CLI flag）
7. Integration test：兩個 backend 各自跑 smoke test

工程量：重（1 週以上；spike 結果影響估算）。

## Out of scope

- 同時跑多個 provider（v1 always single active provider）
- 非 CLI-based provider（直接打 REST API 不走 binary）— 另行評估
- MyCoder CLI 整合 — 獨立 backlog，共用本 change 的 AgentBackend 抽象

## 依賴

- 必須在 D `v3-app-chat-cmdk` archive 之後（IPC surface 穩定再動 backend 層）
- MyCoder backlog 依賴本 change 的 trait 定義

## 何時動

v3-app-polish-ship（F）之後，或 E + F archive 且確認有 Codex CLI 採用需求時。
先 spike Codex event schema，估算確定後再 propose。
