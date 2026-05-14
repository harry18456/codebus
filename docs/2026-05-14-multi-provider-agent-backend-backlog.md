# Backlog: multi-provider agent backend（Codex CLI + Azure endpoint）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（架構擴充性）
**Owner:** harry
**Status:** parked

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
