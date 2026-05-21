## Context

codebus spawn agent 的邏輯目前硬編碼在 `agent::claude_cli`：`build_claude_cmd` 直接組 `claude` argv、`parse_claude_stream_line` 解析 Claude 專屬 stream-json、`sniff_init_session_id` 抽 session id。資料層（`StreamEvent` / `TokenUsage` / `RunLog`）已在先前 normalized，`invoke()` 的 spawn/stdio/cancel/accumulate 迴圈本身已是 provider-無關，只剩上述 3 個 Claude-專屬呼叫點。

完整設計脈絡見 `docs/2026-05-21-multi-provider-design-discussion.md`。本 change 是「先純封裝、後整合 codex」分期交付的第一階段。

## Goals / Non-Goals

**Goals:**

- 抽出 `AgentBackend` trait 作為外殼與 provider 之間唯一契約。
- 把現有 Claude 邏輯收進 `ClaudeBackend`，`invoke()` 改吃 `&dyn AgentBackend`。
- 導入中性 `SpawnSpec`，verb 層改組 `SpawnSpec` 餵 backend。
- config schema 統一為 provider-agnostic 的 `agent.providers.<name>.*`（只填 claude），刪除 legacy 偵測。
- Claude runtime 行為（argv / env / stream）byte-equivalent 不變——這是「重構安全」的鐵證。

**Non-Goals:**

- 不引入 `CodexBackend` 或任何第二家 provider（第二階段）。
- 不加 codex provider config 區塊；`agent.providers` 本階段只有 `claude` 一個 key。
- 不做 skill bundle 雙寫（第二階段）。
- 不跑 codex 本機 spike（slash 叫用 / MCP 旗標——第二階段 codex backend 內部 task）。
- 不實作多家 active_provider 路由邏輯：只有 claude，dispatch 點鋪好、預設指向 claude 即可。
- 不改 verb 的可觀察行為；marker 解析、RunLog、vault 前置全部留在 verb 層不動語意。

## Decisions

### AgentBackend trait 抽既有 invoke 的 3 個 Claude-專屬呼叫點

`invoke()` 迴圈泛用，只有 `build_claude_cmd` / `parse_claude_stream_line` / `sniff_init_session_id` 是 Claude-專屬。trait 就抽這 3 點：

```rust
trait AgentBackend: Send + Sync {
    fn build_command(&self, spec: &SpawnSpec) -> std::process::Command;
    fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent>;
    fn extract_session_id(&self, line: &str) -> Option<String>;
}
```

`invoke()` 改簽名吃 `&dyn AgentBackend`，迴圈本體（spawn/stdio/cancel/stderr-thread/token accumulate）不動。替代方案：把整個 invoke 搬進 trait——否決，會把 spawn/cancel 邏輯複製進每家 provider，seam 太淺。

### SpawnSpec 為中性意圖、重用既有 Verb enum、不立 SpawnRole

外殼餵 backend 的單位是「一次 spawn 的意圖」，欄位一律中性、不洩 Claude 專屬編碼：

```rust
struct SpawnSpec {
    verb: Verb,                        // 既有 enum Goal|Query|Fix|Chat|Quiz|Verify；決定 skill + 經 backend 自己的 resolve(verb) 解出 model
    input: String,
    permission: Permission,            // ReadOnly | Workspace
    command_allowance: Option<CommandPrefix>, // 中性指令前綴，如 ["codebus","quiz","validate"]
    resume_session_id: Option<String>,
}
enum Permission { ReadOnly, Workspace }
struct CommandPrefix(Vec<String>);
```

`config::Verb` + `resolve(Verb)` 已編碼 `Chat→query`/`Quiz→query`/`Verify→verify` 的 model 對映，故不另立 `SpawnRole`（會是冗餘第二軸）。`permission` 不可由 verb 推導：quiz-plan 與 quiz-generate 同 `Verb::Quiz` 但不同 permission，故 permission/command_allowance/resume 都是 per-spawn 欄位。替代方案：保留現有 `InvokeAgentOptions` 直通 `--tools` csv——否決，那是 Claude 形狀，加第三家 provider 時外殼得反推 Claude 語法。

### ClaudeBackend 把 SpawnSpec 翻譯回現有 argv，保證 byte-equivalent

`ClaudeBackend::build_command` 把 `SpawnSpec` 對映回現有 argv：`verb`+`input` → `-p /codebus-{verb} "input"`；`permission` ReadOnly→read-only toolset、Workspace→含 Write/Edit toolset；`command_allowance` → `--allowedTools` 的 `Bash(<prefix> *)` specifier + `--tools` 的裸 `Bash`；MCP 隔離旗標（`--strict-mcp-config` + 空 config）不變。驗收硬標準：對相同輸入，產出的 argv 與重構前 `build_claude_cmd` byte-equivalent。`parse_stream_line` / `extract_session_id` 直接包裹既有 `parse_claude_stream_line` / `sniff_init_session_id`。

### config schema 統一為 agent.providers 巢狀結構，刪除 legacy 偵測

YAML 由 `claude_code.*` 改為 `agent.active_provider` + `agent.providers.<name>.*`；claude provider 內層保留既有 `active`(system|azure) + system/azure endpoint profile 結構（SystemModel、azure base_url/keyring、四 verb 子塊）。本階段 `providers` 只有 `claude`。`ParseOutcome::Legacy` 偵測與遷移警告整段移除（專案未 release、無遷移）。endpoint profile 解析/驗證、SystemModel 對映、keyring fallback、scoped env 注入語意全部不變，只是 YAML 路徑深一層。

### verb 層組 SpawnSpec、marker 解析留在 verb 層

每個 verb spawn 點改成建 `SpawnSpec` 餵 backend（quiz flow 產 `Verb::Quiz`(plan,ReadOnly) → `Verb::Quiz`(generate,ReadOnly+command_allowance) → `Verb::Verify`(content-verify)）。`[CODEBUS_*]` 語意 marker 解析（promote-suggestion / quiz scope / no-match）是 codebus 與自己 SKILL prompt 的協定、provider-無關，**保留**在 verb 層，不進 backend。

## Implementation Contract

**行為（對外可觀察）**：本 change 對 end user 零行為改變——`codebus goal/query/fix/chat/quiz` 的 argv、env、stream 輸出、RunLog、退出碼與重構前一致。唯一刻意改變的是 `~/.codebus/config.yaml` 的 schema 路徑（見下）。

**介面 / 資料形狀**：
- 新增 `agent::AgentBackend` trait（`build_command(&SpawnSpec) -> Command`、`parse_stream_line(&str) -> Vec<StreamEvent>`、`extract_session_id(&str) -> Option<String>`）。
- 新增 `agent::SpawnSpec` / `agent::Permission` / `agent::CommandPrefix`（欄位如 Decisions 所列）。
- 新增 `agent::ClaudeBackend`（實作 trait）。
- `agent::invoke` 簽名改為接受 `backend: &dyn AgentBackend`（取代內部寫死的 Claude 呼叫）。
- config：`~/.codebus/config.yaml` 頂層由 `claude_code:` 改為 `agent:`，含 `active_provider: claude` 與 `providers.claude.{active, system, azure}`；claude 內層結構與既有 `claude_code` 的 `{active, system, azure}` 相同。

**失敗模式**：config 解析失敗仍回 `ConfigLoadError::YamlParse`；active profile 缺 verb 子塊（goal/query/fix/verify）仍拒絕載入並指明缺欄。讀到舊 `claude_code:` 頂層 key：因 legacy 偵測移除，視為未知 key／無 `agent` 區塊，落回 provider 預設（claude/system 預設 model）——不再印遷移警告。

**驗收標準**：
- `cargo test --package codebus-core` 全綠；既有 agent-spawn / stream 測試不需改 assertion 即通過（證明行為不變）。
- 新增單元測試斷言：`ClaudeBackend::build_command` 對代表性 `SpawnSpec` 產出的 argv，與重構前 `build_claude_cmd` 對等輸入的 argv 完全相同（含 `--resume` 位置、MCP 隔離旗標、`--tools`/`--allowedTools` 分歧）。
- 新增 config 測試斷言新 `agent.providers.claude.*` schema 的 load/validate（移植自既有 `claude_code` 測試，路徑改寫）；斷言舊 `claude_code:` 不再觸發 legacy 警告路徑。
- `codebus init` 寫出的 starter config 為新 `agent.*` 格式。

**Scope 邊界**：In scope = trait + SpawnSpec + ClaudeBackend + invoke 簽名 + config 統一 + verb 層改組 SpawnSpec + init starter。Out of scope = CodexBackend、codex config、skill 雙寫、codex spike、多家路由邏輯（見 Non-Goals）。

## Risks / Trade-offs

- **既有 config 測試大量改寫（~30 個）誤判為 regression** → 在 tasks 明確標示「config schema 路徑改寫」為預期變更；agent-spawn 行為測試維持 assertion 不變當作真正的安全網。
- **argv 對映漏掉某旗標導致行為悄悄偏移** → 用「byte-equivalent argv」測試當鐵證，任何旗標差異即測試紅。
- **抽 SpawnSpec 屬「為第二 impl 抽象」** → 已有合格 second-impl（codex spike 通過）撐著、且是刻意的 seam 驗證階段；非投機抽象。
- **config capability 名稱 `claude-code-config` 在統一後語意略舊** → 本階段不改名（避免 spec 管理 churn），rename 留待需要時另案。

## Migration Plan

無自動遷移（專案未 release）。開發者本機在升級後重跑 `codebus init`（或手動把 `claude_code:` 區塊改寫為 `agent.providers.claude:`）。Rollback = revert commit。

## Open Questions

無（第二階段的 codex slash 叫用與 MCP 旗標屬下一個 change 的 codex backend 內部 task，與本 change 無關）。
