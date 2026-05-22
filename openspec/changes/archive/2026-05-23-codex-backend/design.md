## Context

Stage 1（archived `agent-backend-seam`）已建好 provider seam：`AgentBackend` trait（3 method）+ `ClaudeBackend` + 中性 `SpawnSpec` + provider-neutral `invoke` 迴圈 + 統一 `agent.providers.<name>.*` config。目前僅 claude 一個實作，verb 硬編 `ClaudeBackend::new`，config 拒非-claude `active_provider`。codex 是純加法。

所有事實基礎來自 2026-05-22 對 codex CLI 0.132.0 的端到端實機 spike，逐條結果記在 docs/2026-05-14-multi-provider-agent-backend-backlog.md 的 §4(F)（隔離）與 §7（Azure）。本文件記「為什麼」，事實證據以該 doc 為準。

```
verb (goal/query/fix/chat/quiz)
   └─ dispatch(active_provider) ─▶ Box<dyn AgentBackend>  { ClaudeBackend(既有) | CodexBackend(本change) }
   └─ invoke(&dyn AgentBackend, SpawnSpec) 迴圈（既有,不動）
```

## Goals / Non-Goals

**Goals:**

- 以 codex 作為第二 provider 純加法接入既有 seam，可用 `agent.active_provider: codex` 切換、每個 verb 自動改走 codex。
- 在不影響使用者既有 codex 設定的前提下達成隔離（不繼承 user 全域 MCP/plugin、不被被分析 repo 的 `.codex/` 或 `AGENTS.md` 注入）。
- 支援 Azure OpenAI 部署（與 Claude 的 azure 設定分開）。

**Non-Goals:**

- codex 的 GUI 設定 UI（前端 Settings/EndpointSection、`check_cli_installed` 擴 codex）— 另一個 change。
- 多 provider 同時啟用（v1 single active provider）。
- 比 sandbox 更細的 codex 工具白名單。
- 機器層 `requirements.toml` 硬隔離。
- 既有 `.claude` 路徑與 ClaudeBackend 行為不變。

## Decisions

**D1 隔離靠 per-spawn 旗標、不靠 trust/requirements.toml**：每次 spawn 帶 `--ignore-user-config` + `--disable apps` + `--ignore-rules` + `-c project_root_markers=['<vault-marker>']` + `-s <sandbox>` + `--skip-git-repo-check` + `--ephemeral`。全 per-spawn、不寫使用者環境（spike 確認 `--ignore-user-config` 下 codex 不寫 user config）。

**D2 trust 不對稱（spike 關鍵發現,曾誤判已更正）**：`.codex/config.toml` 受 trust 管（未信任→不載）；`.codex/skills/` 與 `AGENTS.md` 不受 trust 管。故被分析 repo 的 config 注入由 `--ignore-user-config`（令未信任）擋；repo 的 skills/AGENTS 注入由 `project_root_markers`（root 釘在 `.codebus/`）擋；兩旗標各管一渠道、缺一不可。codebus 自家 `.codebus/.codex/skills/` 與 `.codebus/AGENTS.md` 在此配方下都會被採用。

**D3 model/effort 走 CLI flag**：`.codex/config.toml` 受 trust 管、隔離下不載，故 model 用 `-m`、effort 用 `-c model_reasoning_effort=`（`-c` 覆寫不受 trust 影響,spike 已驗）。

**D4 command_allowance degrade + warn**：codex 無 `--allowedTools` 等價物（以 sandbox 把關）。依既有 seam 契約與 provider-no-hard-gate 原則，best-effort + warning，不擋 spawn。

**D5 指示主走 AGENTS.md、skills 為輔**：`.codebus/AGENTS.md`（鏡射 CLAUDE.md）always 載入、最可靠；`.codebus/.codex/skills/`（同 SKILL.md 格式）description 在隔離下仍註冊（曾誤判「不註冊」已更正），雙寫即可。

**D6 Azure 走 Responses API**：codex 0.132.0 砍 `wire_api="chat"`，必須 `wire_api="responses"`；base_url 到 `/openai`（codex 自接 `/responses`）；auth 用 `api-key` header（非 Bearer）、`api-version` 走 query param；deployment 名當 `-m`。

## Implementation Contract

**Behavior**：當 `agent.active_provider: codex` 時，所有 verb 的 agent spawn 改用 codex CLI；輸出（思考/工具呼叫/usage）經 `parse_codex_stream_line` 正規化成既有 `StreamEvent`，下游 RunLog / 渲染不感知 provider 差異。Azure 設定存在時打 Azure 端點。

**Interface / data shape**：
- `CodexBackend::build_command(&SpawnSpec) -> Command`：組 `codex exec` argv（見 D1/D3）；`Permission::ReadOnly→-s read-only`、`Workspace→-s workspace-write`；resume 用 `codex exec resume <id>`；binary 經 `CODEBUS_CODEX_BIN`（預設 `codex`）。
- `parse_codex_stream_line(line) -> Vec<StreamEvent>`：對映見 codex-backend spec 與 backlog §spike 表（command_execution→ToolUse+ToolResult；agent_message→Thought；turn.completed.usage→Usage）。
- `extract_session_id(line) -> Option<String>`：`thread.started.thread_id`。
- dispatch fn：`active_provider`(claude|absent→Claude, codex→Codex) → `Box<dyn AgentBackend>`。
- config：`agent.providers.codex.{active,system,azure}`；azure 攜 `base_url`/`api_version`/`keyring_service`(預設 `codebus-azure`)。
- vault：`<vault>/.codebus/AGENTS.md`、`<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md`、vault-unique marker 檔。

**Failure modes**：`active_provider` 非 claude/codex → `ConfigLoadError::YamlParse`；codex active profile 缺 verb → parse 錯不靜默 default；`command_allowance` 在 codex → warning 非錯；spawn 必須關/餵空 stdin（否則 codex exec 卡死等 stdin）。

**Acceptance criteria**：`cargo test --package codebus-core`（含新 CodexBackend argv 測試、parse_codex_stream_line 對映測試、dispatch routing 測試、codex config parse 測試,皆 TDD RED 先行）；config 解析接受 `active_provider: codex`、拒 `gemini`；vault 生成寫出 AGENTS.md + `.codex/skills` + marker 且 write-if-missing；手動 e2e 實跑 codex（含 Azure）依 v3-roadmap deferred-registry 慣例歸檔。

**Scope boundaries**：見 Non-Goals。core + CLI 範圍;不動前端、不動 ClaudeBackend、不動既有 `.claude` 路徑。

## Risks / Trade-offs

- **codex 版本漂移**：`wire_api="chat"` 已在 0.132.0 被砍、approval flag 互動/非互動有別、`--disable apps` 旗標名等都可能隨版本變。對映與旗標集中在 `CodexBackend`，升版時單點維護；CI 無法測真實 codex，靠手動 e2e。
- **trust 不對稱依賴**：隔離正確性依賴「config 受 trust 管、skills/AGENTS 不受」這個實測行為;若 codex 改變此語意,隔離假設需重驗（測試以 spike 的清淨 codeword/banner canary 方法重跑）。
- **web.run 等內建工具仍在工具清單**：spike 未觀察到實際網路外洩（無 `web_search_call` 事件;shell 網路被 read-only 沙箱擋），但工具名仍在;若未來需嚴格對齊 Claude 無-web 姿態,需另找關閉機制（非本 change 阻塞項）。
- **AGENTS.md 作為主指示通道**：與 Claude 的 skill-bundle 機制不同形,verb 工作流內容需在 AGENTS.md 與 skills 兩處維持一致;以「鏡射 CLAUDE.md」收斂單一來源。
