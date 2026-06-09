# Backlog: MyCoder CLI 整合（台智雲 Agentic AI CLI）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（multi-provider 擴充）
**Owner:** harry
**Status:** pending — 2026-05-20 spike 結論：等對方 CLI 長出 codebus 需要的 contract

---

## 2026-05-20 更新：pending（基於 agy 1.0.0 spike 結果）

2026-05-20 對 Antigravity CLI（`agy` v1.0.0，2026-05-19 上市）做了 spike，發現幾個結構性 gap 跟 mycoder 估同類型 CLI 大概率同樣存在：

| Seam | Claude CLI | agy 1.0.0 | mycoder（估） |
|---|---|---|---|
| Tool whitelist | `--tools` / `--allowedTools` | **無**（只 `--sandbox` 全有全無）| 待 spike |
| Stream format | `--output-format stream-json --verbose` | **無**（plain text）| 待 spike |
| Agentic tool loop in `-p` mode | ✓ | **看不到證據**（tool call 跑空 EXIT 0）| 待 spike |
| Skill bundle | `.claude/skills/*/SKILL.md` + slash | `agy plugin import claude`（潛在橋）| 待 spike |
| Session resume | `--resume <id>` | `--conversation <id>` ✓ | 待 spike |

agy 的 CLI 設計偏向「IDE-primary、CLI 是薄 chat 包裝」，跟 codebus「spawn `claude -p` 走 skill 觸發 agentic 流程」的核心模式直接對撞 —— 不是 codebus 抽 trait 就能補的，是對方 CLI 本身要長出 binary-layer toolset gate 跟 structured stream。

**重要校準（避免未來走錯方向）：** 卡的不是「codebus 太深嵌 Claude」（codebus 的 `StreamEvent` / `TokenUsage` / `RunLog` 在 `v3-run-log-events` 已 normalized，只剩 `parse_claude_stream_line` 一支 Claude 專屬 parser），卡的是「對方 CLI 還沒長出 codebus 需要的 contract」。

**重啟條件**：
- mycoder CLI 確認具備 `--tools` 白名單或等價機制，且 `-p` mode 能真正 invoke tool（不是 silent exit）
- 或 codex CLI 上線且 contract 較接近 Claude（contract 較像 v1 對標）
- 或對方提供 stream-json 等價的結構化輸出

在這之前本 backlog 不動，原下方「前置條件」段落（取得方式 / output format / auth）依然要在重啟時逐條 spike，但**先決條件升級為「對方 CLI 是否具備 codebus contract」**。

### 2026-05-20 更新：codex 0.132.0 已滿足「第二家 second-impl」條件

同日 codex spike 結果（詳見 [`2026-05-14-multi-provider-agent-backend-backlog.md`](2026-05-14-multi-provider-agent-backend-backlog.md) 2026-05-20 段）：codex CLI **所有 codebus 需要的 contract 都有**，已成為合格 second-impl 對標目標。`v3-multi-agentic-provider` 已從 💭 升級 🟢 unblocked。

對 mycoder 的影響：

- ✅ 「跨家 abstraction 怎麼長」這個前提**現在可以靠 codex 進場時邊做邊驗證**，不需要等 mycoder 自己出 spike
- ⏳ 但 mycoder 自己的存取條件 + CLI behavior **仍未驗證**，重啟本條 backlog 還是要先做自己的 spike
- 預期：等 `v3-multi-agentic-provider` change 真的做完、`AgentBackend` 抽象長出來後，mycoder 變成「加第三個 backend impl」級別的工作（前提是 mycoder 自身 CLI contract 也合格）

---

## 觀察

台智雲（AFS，華碩子公司）推出 MyCoder CLI，定位為企業級 Agentic AI coding assistant，功能類似 Claude Code / Codex CLI。

若 codebus 已實作 `AgentBackend` trait（見 `multi-provider-agent-backend` backlog），新增 MyCoder 支援只需要再加一個 backend 實作，無需改動核心架構。

## 前置條件（Conditional）

本條 backlog **blocked** 直到以下條件確認：

- [ ] MyCoder CLI 的取得方式（公開下載 / 需要台智雲帳號 / 企業合約）
- [ ] CLI 的 output format（stream-json 或其他格式）
- [ ] API endpoint / auth 機制（local binary 或需連台智雲雲端）
- [ ] 授權條款是否允許第三方整合

## Proposed fix（確認前提後）

依賴 `multi-provider-agent-backend` change 的 `AgentBackend` trait：

```rust
// codebus-core/src/agent/backend/mycoder.rs（示意）
pub struct MyCoderBackend {
    binary_path: PathBuf,
    endpoint: Option<Url>,
    api_key: Option<String>,
}

impl AgentBackend for MyCoderBackend {
    fn spawn(&self, opts: SpawnOpts) -> Result<AgentHandle> {
        // 依 MyCoder CLI 的 flag / env var 規格實作
    }
}
```

### Config

```yaml
agent:
  provider: mycoder
  mycoder:
    binary: mycoder        # PATH 中的 binary 名稱
    endpoint: null         # 若需要自訂 endpoint
```

### Tasks（粗估，確認前提後再細化）

1. MyCoder CLI 行為 spike（event schema / flag / auth）
2. `MyCoderBackend` 實作
3. Config schema 加 `agent.mycoder.*`
4. Settings UI 加 MyCoder 選項（與 Codex 同批）
5. Integration test：MyCoder smoke test

工程量：中（spike 結果影響估算；若 protocol 近似 Codex 可快速複用）。

## Out of scope

- MyCoder 雲端功能（僅用 CLI local 模式，與 codebus local-first 一致）

## 依賴

- **`multi-provider-agent-backend` backlog**：必須先完成 AgentBackend trait
- 存取條件確認前保持 parked

## 何時動

1. 先確認存取條件
2. 確認後，與 `multi-provider-agent-backend` change 一起評估
