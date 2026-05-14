# Backlog: MyCoder CLI 整合（台智雲 Agentic AI CLI）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** feature gap（multi-provider 擴充）
**Owner:** harry
**Status:** parked — conditional（需確認存取方式）

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
