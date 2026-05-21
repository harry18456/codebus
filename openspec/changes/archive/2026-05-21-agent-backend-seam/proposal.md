## Why

codebus 定位為 multi-provider 工具，但目前 spawn agent 的邏輯硬編碼在 `agent::claude_cli`（直接組 `claude` argv、解析 Claude 專屬 stream-json）。資料層（`StreamEvent` / `TokenUsage` / `RunLog`）在 `v3-run-log-events` 已 normalized，缺的是一道 provider 抽象 seam。

本 change 是「先純封裝、後整合」分期交付的**第一階段**：在不引入任何新 provider 的前提下，把 provider-無關的水電一次鋪好——抽出 `AgentBackend` trait、把現有 Claude 邏輯收進 `ClaudeBackend`、導入中性 `SpawnSpec`、並把 config schema 統一成 provider-agnostic 的 `agent.providers.*` 格式。如此第二階段加 codex 時可以是**純加法**，不必再回頭動封裝或改 Claude 行為。

## What Changes

- 新增 `AgentBackend` trait（3 個 method：`build_command(spec)` / `parse_stream_line(line)` / `extract_session_id(line)`）作為外殼與 provider 之間的唯一契約。
- 新增 `SpawnSpec`（中性意圖：`verb` / `input` / `permission` / `command_allowance` / `resume_session_id`）+ `Permission` + `CommandPrefix`。重用既有 `Verb` enum，**不**另立 `SpawnRole`。
- 新增 `ClaudeBackend`：把現有 `build_claude_cmd` / `parse_claude_stream_line` / `sniff_init_session_id` 邏輯搬入，實作 trait；`SpawnSpec` → Claude argv（`-p /codebus-{verb}`、`--tools`/`--allowedTools`/`--permission-mode`、MCP 隔離旗標）。
- `agent::invoke` 迴圈改吃 `&dyn AgentBackend`，spawn/stdio/cancel/accumulate 迴圈本身 provider-無關不變。
- verb 層改為組 `SpawnSpec` 餵 backend（quiz 三 spawn 各一個）；codebus 語意 marker 解析（`[CODEBUS_*]`）**保留**在 verb 層。
- **BREAKING（config 檔格式）**：config schema 從 `claude_code.*` 統一為 `agent.active_provider` + `agent.providers.<name>.*`（本階段只填 `claude`，內含既有 `system`/`azure` endpoint profile）。**刪除** legacy schema 偵測與遷移警告（專案未 release，無遷移、無向後相容）。Claude 的 *runtime 行為*（argv / env / stream）byte-equivalent 不變。

## Non-Goals (optional)

詳見 design.md 的 Goals / Non-Goals。核心排除：本階段不引入 `CodexBackend`、不加 codex provider config 區塊、不做 skill bundle 雙寫、不跑 codex 本機 spike、不實作 active_provider 多家路由（只有 claude，dispatch 點鋪好即可）。

## Capabilities

### New Capabilities

- `agent-backend`: provider-無關的 agent 後端抽象——`AgentBackend` trait 契約、`SpawnSpec` 中性意圖、`ClaudeBackend` 實作、`invoke` 迴圈以 `&dyn AgentBackend` 驅動。

### Modified Capabilities

- `claude-code-config`: config schema 由 `claude_code.*` 統一為 `agent.providers.<name>.*`（claude-only），移除 legacy schema 偵測；endpoint profile（system/azure）、SystemModel、keyring、scoped env 注入語意不變，僅 YAML 路徑改變。

## Impact

- Affected specs: `agent-backend`（new）、`claude-code-config`（modified）
- Affected code:
  - New:
    - codebus-core/src/agent/backend.rs
    - codebus-core/src/agent/claude_backend.rs
    - codebus-core/src/agent/spawn_spec.rs
  - Modified:
    - codebus-core/src/agent/mod.rs
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/config/endpoint.rs
    - codebus-core/src/config/claude_code.rs
    - codebus-core/src/config/mod.rs
    - codebus-core/src/agent/env_overrides.rs
    - codebus-core/src/verb/goal.rs
    - codebus-core/src/verb/query.rs
    - codebus-core/src/verb/fix.rs
    - codebus-core/src/verb/chat.rs
    - codebus-core/src/verb/quiz.rs
    - codebus-core/src/verb/content_verify.rs
  - Removed:
    - (none — legacy schema 偵測在 endpoint.rs / claude_code.rs 內就地移除，非整檔刪除)
