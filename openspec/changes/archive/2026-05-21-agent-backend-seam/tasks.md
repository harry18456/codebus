# Tasks

## 1. SpawnSpec 與 AgentBackend trait 契約

- [x] 1.1 [P] RED：寫 `SpawnSpec` / `Permission` / `CommandPrefix` 型別測試——建構 `SpawnSpec { verb: Verb::Quiz, permission: Permission::ReadOnly, command_allowance: Some(CommandPrefix(vec!["codebus","quiz","validate"])), .. }` 並讀回欄位；斷言 `command_allowance` 儲存中性 token 序列、非 Claude glob 字串。驗證：新測試 `spawn_spec_carries_neutral_intent` 編譯失敗（型別未定義）。
- [x] 1.2 GREEN：在 `codebus-core/src/agent/spawn_spec.rs` 定義 `SpawnSpec` / `Permission`(ReadOnly|Workspace) / `CommandPrefix`，並由 `agent/mod.rs` re-export。驗證：1.1 測試轉綠。（需求：`SpawnSpec Provider-Neutral Intent`；設計決策：SpawnSpec 為中性意圖、重用既有 Verb enum、不立 SpawnRole）
- [x] 1.3 RED：寫 `AgentBackend` trait 契約測試——以一個 in-test stub 實作 `build_command`/`parse_stream_line`/`extract_session_id`，斷言 `invoke` 可吃 `&dyn AgentBackend`（編譯層 + stub 驅動）。驗證：測試 `agent_backend_trait_object_is_drivable` 編譯失敗（trait 未定義）。
- [x] 1.4 GREEN：在 `codebus-core/src/agent/backend.rs` 定義 `AgentBackend: Send + Sync` trait（3 method，無 tool/sandbox/model 參數），`agent/mod.rs` re-export。驗證：1.3 測試轉綠。（需求：`Agent Backend Trait Contract`；設計決策：AgentBackend trait 抽既有 invoke 的 3 個 Claude-專屬呼叫點）

## 2. ClaudeBackend 實作（argv byte-equivalent）

- [x] 2.1 RED：寫 byte-equivalent argv 測試——對代表性 `SpawnSpec`（goal/Workspace、query/ReadOnly、含 `command_allowance`、含 `resume_session_id`），斷言 `ClaudeBackend::build_command` 產出的 argv token 序列等於重構前 `build_claude_cmd` 對等 `InvokeAgentOptions` 的 argv（含 `-p /codebus-<verb>`、`--tools`/`--allowedTools` 分歧、`--permission-mode acceptEdits`、`--strict-mcp-config` + 空 `--mcp-config`、`--model`/`--effort`、`--resume` 位於 `--tools` 之前）。驗證：測試 `claude_backend_argv_byte_equivalent` 失敗（`ClaudeBackend` 未實作）。
- [x] 2.2 GREEN：在 `codebus-core/src/agent/claude_backend.rs` 實作 `ClaudeBackend::build_command`——把 `SpawnSpec.verb`+`input` 組成 slash command、`permission` 對映 read-only/workspace toolset、`command_allowance` 對映 `Bash(<prefix> *)` allowedTools specifier + 裸 `Bash` tools、保留 MCP 隔離旗標。可重用既有 `build_tools_csv`/`build_allowed_tools_csv` 私有 helper。驗證：2.1 測試轉綠。（需求：`Claude Backend Argv Equivalence`；設計決策：ClaudeBackend 把 SpawnSpec 翻譯回現有 argv，保證 byte-equivalent）
- [x] 2.3 GREEN：實作 `ClaudeBackend::parse_stream_line` / `extract_session_id`，分別包裹既有 `parse_claude_stream_line` / `sniff_init_session_id`，行為不變。驗證：新測試 `claude_backend_parse_matches_legacy` 斷言對相同輸入行的輸出與既有函式相同，轉綠。

## 3. invoke 迴圈改由 trait 驅動

- [x] 3.1 RED：寫測試斷言 `invoke` 從 `backend.build_command` spawn、用 `backend.parse_stream_line` 解析每行、用 `backend.extract_session_id` 抽 session id（以 stub backend 驗 delegation）。驗證：測試 `invoke_delegates_to_backend` 因 `invoke` 仍寫死 Claude 呼叫而失敗。
- [x] 3.2 GREEN：改 `agent::invoke` 簽名為接受 `backend: &dyn AgentBackend`，把 `build_claude_cmd`/`parse_claude_stream_line`/`sniff_init_session_id` 三處呼叫改為委派 backend；spawn/stdio/cancel/stderr-thread/token-accumulate 迴圈本體不動、不含 `claude` 字面或 Claude 旗標。驗證：3.1 轉綠，且既有 `invoke_*` 行為測試（cancel/none-path 等）assertion 不變通過。（需求：`Invocation Loop Drives Backend Trait`；設計決策：AgentBackend trait 抽既有 invoke 的 3 個 Claude-專屬呼叫點）

## 4. config schema 統一為 agent.providers.\<name\>（claude-only）

- [x] 4.1 [P] RED：寫新 schema load/validate 測試——`agent.active_provider: claude` + `agent.providers.claude.{active,system,azure}` 的 happy path 與拒絕路徑（active profile 缺 verb 子塊、invalid SystemModel、非 active profile 可 partial），移植自既有 `claude_code` 測試並改寫 YAML 路徑。驗證：新測試套 `agent_providers_schema_*` 因解析仍認 `claude_code` 而失敗。
- [x] 4.2 [P] GREEN：改 `codebus-core/src/config/endpoint.rs` 與 `codebus-core/src/config/claude_code.rs` 的解析/驗證，頂層由 `claude_code` 改為 `agent.active_provider` + `agent.providers.<name>`；claude 內層 `active`/`system`/`azure` 結構、`SystemModel` 對映、四 verb 子塊、`verify` 解析、keyring/env 欄位語意不變。驗證：4.1 轉綠。（需求：`Endpoint Profile Schema`、`System Profile Model Aliases`；設計決策：config schema 統一為 agent.providers 巢狀結構，刪除 legacy 偵測）
- [x] 4.3 RED：寫測試斷言讀到舊 `claude_code:` 頂層 verb key 時，**不**印 legacy 遷移警告、視為無 `agent` 區塊而落回 provider 預設（claude/system 預設 model）。驗證：測試 `legacy_claude_code_no_longer_warns` 因 legacy 偵測仍存在而失敗。
- [x] 4.4 GREEN：移除 `ParseOutcome::Legacy` 偵測分支與遷移警告輸出（`endpoint.rs` / `claude_code.rs` 就地移除，含 `LEGACY_MIGRATION_WARNING` 常數與其引用）。驗證：4.3 轉綠；既有「legacy 觸發警告」測試一併刪除/改寫，標示為預期變更。（需求：`Legacy Config Schema Warning Without Rewrite`；設計決策：config schema 統一為 agent.providers 巢狀結構，刪除 legacy 偵測）

## 5. verb 層改組 SpawnSpec

- [x] 5.1 RED：寫測試斷言 quiz flow 產出三個 `SpawnSpec`——plan(`Verb::Quiz`,ReadOnly)、generate(`Verb::Quiz`,ReadOnly,command_allowance=["codebus","quiz","validate"])、content-verify(`Verb::Verify`,ReadOnly)；並斷言 `[CODEBUS_*]` marker 解析仍由 verb 層執行（非 backend）。驗證：測試 `quiz_builds_three_spawn_specs` 失敗（verb 仍建 `InvokeAgentOptions`）。
- [x] 5.2 GREEN：改 `codebus-core/src/verb/{goal,query,fix,chat,quiz,content_verify}.rs` 各 spawn 點建 `SpawnSpec` 並透過 `invoke(&ClaudeBackend, spec, ..)` 呼叫；既有 marker 解析（promote-suggestion / quiz scope / no-match）邏輯原地保留在 verb 層。驗證：5.1 轉綠，且既有各 verb 行為測試（toolset、marker、vault 前置）assertion 不變通過。（設計決策：verb 層組 SpawnSpec、marker 解析留在 verb 層）

## 6. init starter config 改新格式

- [x] 6.1 [P] RED：寫測試斷言 `codebus init`（`write_starter_config_if_missing`）寫出的 starter config 為新 `agent.active_provider: claude` + `agent.providers.claude.*` 格式且可被新 loader 解析。驗證：測試 `starter_config_uses_agent_schema` 失敗（仍寫 `claude_code`）。
- [x] 6.2 [P] GREEN：改 `codebus-core/src/config/global_starter.rs` 產生新 schema starter。驗證：6.1 轉綠。

## 7. 全套回歸驗證

- [x] 7.1 跑 `cargo test --package codebus-core` 全綠，並逐一確認既有 agent-spawn / stream / 各 verb 行為測試**未改 assertion** 即通過（這是「Claude runtime 行為 byte-equivalent」的鐵證）；config schema 路徑相關測試的改寫標示為預期變更、非 regression。驗證：`cargo test --package codebus-core` 退出碼 0。
