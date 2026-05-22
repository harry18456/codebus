## Why

codebus 定位為 multi-provider 工具。Stage 1（archived change `agent-backend-seam`）已建好 provider seam：`AgentBackend` trait（3 method）+ `ClaudeBackend` + 中性 `SpawnSpec` + provider-neutral `invoke` 迴圈 + 統一的 `agent.providers.<name>.*` config。但目前只有 claude 一個實作，每個 verb 硬編 `ClaudeBackend::new(...)`，且 config 解析主動拒絕非-claude 的 `active_provider`。

harry 近期要實際以 OpenAI Codex（含 Azure OpenAI 部署）跑 codebus。2026-05-22 已對 codex CLI 0.132.0 做端到端實機 spike，逐條驗證了 event contract、sandbox 對映、per-spawn 隔離配方、skill/AGENTS.md 載入規則、以及 Azure OpenAI 連線（findings 全在 docs/2026-05-14-multi-provider-agent-backend-backlog.md 的 §4(F) 與 §7）。本 change 把 codex 作為第二 provider 以純加法接入既有 seam。

## What Changes

- 新增 `CodexBackend` 實作 `AgentBackend` 三方法：`build_command`（組 codex exec 的隔離配方 argv：`--ignore-user-config` + `--disable apps` + `--ignore-rules` + `project_root_markers` + `-s <sandbox>` + `-m`/`-c model_reasoning_effort` + resume）、`parse_codex_stream_line`（codex JSONL event → 既有 `StreamEvent`）、`extract_session_id`（`thread.started.thread_id`）。
- 新增 runtime provider dispatch 選擇層（`active_provider` → `Box<dyn AgentBackend>`），取代 5 個 verb 目前硬編的 `ClaudeBackend::new(...)` 建構點。
- 解除 config 解析對非-claude `active_provider` 的 reject；新增 `agent.providers.codex.*` 配置 schema（`system` profile 自由字串 model + 獨立 `azure` profile：base_url / api-version / keyring_service，對映 codex 的 Responses API + `api-key` header）。
- skill bundle 雙寫到 `<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md`，並生成 `<vault>/.codebus/AGENTS.md`（鏡射 `.codebus/CLAUDE.md` 的 taxonomy/frontmatter/語言政策）作為 codex 的權威指示通道。
- Permission 對映：`ReadOnly` → `-s read-only`、`Workspace` → `-s workspace-write`；`command_allowance` 在 codex 無等價物時 degrade + warn（不設 hard gate）。

## Non-Goals (optional)

- codex 的 GUI 設定介面（前端 Settings/EndpointSection、`check_cli_installed` 擴充 codex provider）— 屬另一個 change（app-shell spec 已註明「未來 provider 值另一個 change 擴 match arm」）。
- 同時啟用多個 provider（v1 仍 single active provider）。
- 比 sandbox 更細的 codex 工具白名單（codex 以 `-s` sandbox 把關，spike 已驗 read-only 擋寫入/網路、足夠）。
- 機器層 `requirements.toml` 硬隔離（per-spawn 旗標已足夠；留作未來 enterprise 選項）。
- 既有 `.codebus/.claude/skills/` 與 Claude 路徑不變（純加法）。

## Capabilities

### New Capabilities

- `codex-backend`: `CodexBackend`（codex argv 隔離配方、stream 解析、session id 抽取）與 provider dispatch 選擇層。
- `codex-config`: `agent.providers.codex.*` 配置 schema（`system` + `azure` profile、Responses API 端點、keyring service）。

### Modified Capabilities

- `claude-code-config`: 解除 `active_provider` 的「只支援 claude」限制，改為接受 `claude` 或 `codex`。
- `skill-bundles`: 新增 codex 指示材料化（`.codebus/AGENTS.md` 生成 + `.codebus/.codex/skills/` 雙寫）。

## Impact

- Affected specs: `codex-backend`（新增）、`codex-config`（新增）、`claude-code-config`（修改）、`skill-bundles`（修改）
- Affected code:
  - New:
    - codebus-core/src/agent/codex_backend.rs
    - codebus-core/src/config/codex.rs
  - Modified:
    - codebus-core/src/agent/mod.rs
    - codebus-core/src/stream/parser.rs
    - codebus-core/src/config/endpoint.rs
    - codebus-core/src/config/mod.rs
    - codebus-core/src/verb/goal.rs
    - codebus-core/src/verb/query.rs
    - codebus-core/src/verb/fix.rs
    - codebus-core/src/verb/chat.rs
    - codebus-core/src/verb/quiz.rs
    - codebus-core/src/skill_bundle/mod.rs
  - Removed: (none)
