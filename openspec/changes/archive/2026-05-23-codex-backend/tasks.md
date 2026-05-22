# Tasks

依賴順序：config（1）→ CodexBackend（3,依賴 1 解析 model/effort）→ dispatch（2,依賴 3 可路由）→ 整合（5）。vault 材料化（4）與 1/3 不同檔、可並行。每個 code task 先寫 RED 測試再實作（`.spectra.yaml` tdd:true）。

## 1. codex 配置 schema（`codex-config` spec）

- [x] 1.1 [P] [RED] 寫 codex config 解析測試：`agent.providers.codex` system profile 接受自由字串 model（如 `gpt-5.5`,不因非 enum 而拒）；azure profile 載入 `base_url`/`api_version`/`keyring_service`；active profile 缺 `verify` 子塊→`ConfigLoadError::YamlParse` 不靜默 default；非-active profile 可缺。測試置於 codebus-core config 測試模組,初次執行應 FAIL。
- [x] 1.2 實作 codex 配置型別與解析（新增 codebus-core/src/config/codex.rs；於 codebus-core/src/config/endpoint.rs 的 `RawProviders` 加 `codex` 欄位、解除 `active_provider` 對非-claude 的 reject 改為接受 `claude`/`codex`、其餘拒；於 codebus-core/src/config/mod.rs 匯出）使 1.1 全綠。涵蓋 spec requirement: Codex Provider Config Schema；並放寬 claude-code-config 的 Endpoint Profile Schema（active_provider 接受 codex）。

## 2. provider dispatch 選擇層（`codex-backend` spec：Provider Dispatch Selection）

- [x] 2.1 [RED] 寫 dispatch routing 測試：`active_provider: codex`→回 `CodexBackend`;`claude` 或缺→`ClaudeBackend`。初次 FAIL。
- [x] 2.2 實作 dispatch fn（`active_provider` → `Box<dyn AgentBackend>`,於 codebus-core/src/agent/mod.rs）並把 codebus-core/src/verb/{goal,query,fix,chat,quiz}.rs 的 `ClaudeBackend::new(...)` 建構點改為向 dispatch 取得 backend,使 2.1 綠且既有 verb 測試不退步。涵蓋 spec requirement: Provider Dispatch Selection。

## 3. CodexBackend 實作（`codex-backend` spec：Argv Composition + Stream Parsing）

- [x] 3.1 [RED] 寫 `CodexBackend::build_command` argv 測試:`Permission::ReadOnly`→argv 含 `-s read-only` 不含 workspace-write;`Workspace`→`-s workspace-write`;任意 spawn→含 `--ignore-user-config`/`--disable apps`/`--ignore-rules` 與 `project_root_markers` 覆寫;model `gpt-5.4`+effort `high`→含 `-m gpt-5.4` 與 `-c model_reasoning_effort=high`;`resume_session_id=Some`→用 `codex exec resume <id>` 形式;`command_allowance=Some`→不 panic、發 warning、不擋。初次 FAIL。
- [x] 3.2 實作 codebus-core/src/agent/codex_backend.rs 的 `build_command`（binary 經 `CODEBUS_CODEX_BIN` 預設 `codex`;model/effort 由注入的 codex config `resolve(verb)` 取得;spawn 關閉/餵空 stdin）使 3.1 綠。涵蓋 spec requirement: Codex Backend Argv Composition。
- [x] 3.3 [P] [RED] 寫 `parse_codex_stream_line` 與 `extract_session_id` 測試,用 spike 實得的 JSONL 樣本:`item.completed`+`command_execution`(exit_code 0)→`ToolUse{name:"Shell"}`+`ToolResult{is_error:false}`;exit_code 非 0→`is_error:true`;`agent_message`→`Thought`;`turn.completed.usage`→`Usage` 四欄對映(`input_tokens`/`cached_input_tokens`/`output_tokens`/`reasoning_output_tokens`);`thread.started`→`extract_session_id` 回 `Some(thread_id)` 且 `parse` 回空。初次 FAIL。
- [x] 3.4 實作 `parse_codex_stream_line`（codebus-core/src/stream/parser.rs,鄰 `parse_claude_stream_line`）+ `CodexBackend::parse_stream_line`/`extract_session_id` 包裝,使 3.3 綠;確認不解讀 `[CODEBUS_*]` 標記。涵蓋 spec requirement: Codex Stream Parsing。

## 4. vault 材料化（`skill-bundles` spec：Codex Instruction Materialization）

- [x] 4.1 [P] [RED] 寫 skill_bundle 測試:codex 材料化時 `<vault>/.codebus/.codex/skills/codebus-{verb}/SKILL.md` 以與 `.claude` 相同 frontmatter+body 格式寫出;`<vault>/.codebus/AGENTS.md` 生成且內容鏡射 `.codebus/CLAUDE.md` 的 taxonomy/語言政策;vault-unique marker 檔存在;既有檔不被覆蓋(write-if-missing);`.claude` 路徑不變。初次 FAIL。
- [x] 4.2 實作 codebus-core/src/skill_bundle/mod.rs 的 codex 雙寫(`.codex/skills/`)+ `.codebus/AGENTS.md` 生成 + marker 檔材料化(沿用 write-if-missing),使 4.1 綠;`.codebus/.claude/` 與 repo-root `.claude/` 路徑保持不變。涵蓋 spec requirement: Codex Instruction Materialization。

## 5. 整合與實機驗證

- [x] 5.1 把 CodexBackend 接進 dispatch:codex provider 經 dispatch 建出 `CodexBackend::new(codex_config, env)`(含 Azure:由 `agent.providers.codex.azure` 的 base_url/api_version/keyring_service 組 provider override、key 從 keyring 讀);跑 `cargo test --package codebus-core` 全綠。
- [x] 5.2 手動 e2e：以 `agent.active_provider: codex` 實跑一個 verb（先 system profile 再 Azure profile,Azure 用 `wire_api=responses` + `api-key` header + `api-version`,deployment 當 `-m`）,確認真實串流正常、隔離旗標生效;GUI smoke 跑不了則照 docs/v3-roadmap.md §5 deferred registry 慣例歸檔,不卡 archive。
