<!-- SPECTRA:START v1.0.1 -->

# Spectra Instructions

This project uses Spectra for Spec-Driven Development(SDD). Specs live in `openspec/specs/`, change proposals in `openspec/changes/`.

## Use `/spectra:*` skills when:

- A discussion needs structure before coding → `/spectra:discuss`
- User wants to plan, propose, or design a change → `/spectra:propose`
- Tasks are ready to implement → `/spectra:apply`
- There's an in-progress change to continue → `/spectra:ingest`
- User asks about specs or how something works → `/spectra:ask`
- Implementation is done → `/spectra:archive`

## Workflow

discuss? → propose → apply ⇄ ingest → archive

- `discuss` is optional — skip if requirements are clear
- Requirements change mid-work? Plan mode → `ingest` → resume `apply`

## Parked Changes

Changes can be parked（暫存）— temporarily moved out of `openspec/changes/`. Parked changes won't appear in `spectra list` but can be found with `spectra list --parked`. To restore: `spectra unpark <name>`. The `/spectra:apply` and `/spectra:ingest` skills handle parked changes automatically.

<!-- SPECTRA:END -->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 溝通語言

使用者偏好 **繁體中文（zh-TW）** 回覆。Spec 內文、commit message、code comment 的 prose 也是 zh-TW；schema / 識別字 / filename / test name 維持英文。

## Repo 現況

M1「power-on」通電（2026-04-19 archive）後，資料層 + Module 4 Explorer Agent（步驟 16-23 P0+P1+coverage+context+golden 全 archive）已補齊；下一個里程碑是 **Module 5 Generator P0**（`docs/implementation-plan.md` 步驟 24，per-station prompt + validator + degraded fallback + tutorial.md/stations 多檔輸出 D-029）。

**子系統**

- `sidecar/` — uv-managed Python 3.12 FastAPI sidecar。核心：app factory、ephemeral port + bearer、stdout handshake、`--parent-pid` watchdog、ToolSandbox（`ensure_in_workspace` + red team fixture）、LLMProvider Protocol + `ProviderRole` dispatch + `MockProvider` / `OpenAIEmbeddingProvider` / `OpenAIChatProvider`、`TrackedProvider`（唯一 `token_usage.jsonl` / `llm_calls.jsonl` 寫入路徑）、三段 Sanitizer 前兩段、Qdrant lifecycle、Module 1 Scanner、Module 2 KB Builder + KB query + dim-mismatch guard、SSE task skeleton、PyInstaller onefile spec。
- `tauri/src-tauri/` — Rust host + `sidecar_ping` command。`src/sidecar.rs` spawn 協定、`src/lib.rs::resolve_sidecar_path()` 在 packaged / dev 模式都找得到 sibling binary。
- `web/` — Nuxt 3 + Tailwind + TypeScript 骨架（npm，D-026）。目前只有 landing page 與 Sidecar Ping 按鈕；Trust Layer 四站（R-01 / O-01 / O-04 / O-05）mockup 已畫好但尚未實作。
- `openspec/specs/` — 15 個 capability spec：`agent-core` / `app-packaging` / `explorer-golden` / `explorer-sse` / `explorer-tools` / `folder-scanner` / `knowledge-base` / `llm-provider` / `qdrant-client` / `repo-layout` / `sanitizer` / `sidecar-runtime` / `tauri-shell` / `tool-sandbox` / `usage-tracking`。
- `docs/` — 19 份 Module / Agent / 橫切層 spec + `decisions.md` ADR + `README.md` / `dev-setup.md` / `implementation-plan.md` / `prompts.md`。
- `design/` — Phase A Trust Layer 的 3 份 HTML mockup（`r-01` / `o-01` / `o-05`）+ 14 張截圖。
- `tests/golden/` — `demo-synthetic/`（比賽 demo / regression fixture）+ `timeline-gdrive-adapter/`（參考實作）。
- `tests/fixtures/` — `precommit-violations/`（commit-gate 負測 fixture）。

**archive 時間軸**（由舊至新，每條 archive 對應 `openspec/changes/archive/YYYY-MM-DD-<name>/`）

| 日期 | Change | 重點 |
|---|---|---|
| 2026-04-19 | `m1-power-on` | Tauri ↔ Sidecar 通電骨架、bearer、handshake、watchdog、Provider Protocol、UsageTracker、LLMCallLogger |
| 2026-04-20 | `llm-role-routing` | `ProviderRole`（reasoning / judge / chat / embed）+ `registry.get(role)` 分派；TrackedProvider 必帶 `role` |
| 2026-04-21 | `qdrant-lifecycle-bootstrap` | `AsyncQdrantClient` 綁 app state + probe-backed healthz |
| 2026-04-21 | `scanner-skeleton` | Module 1 遍歷 + gitignore + encoding + `ScanResult` |
| 2026-04-21 | `sanitizer-safety-chain` | Pass 1 detect-secrets + PII regex + placeholder；Pass 2 provider pre-flight hook（`sanitizer_pass2_applied` 會翻 true）；`sanitize_audit.jsonl` |
| 2026-04-21 | `scanner-sanitizer-orchestration` | Scanner 入 KB 前過 Pass 1 |
| 2026-04-21 | `module-2-kb-builder-p0` | `KnowledgeBase` / `KBPayload` / token-window chunker + 策略分派 / `KBQdrantBackend` Protocol + `QdrantHttpBackend` |
| 2026-04-22 | `sse-progress-skeleton` | `POST /kb/build` async + `POST /scan?stream=true` + `GET /tasks/{id}/events\|result` + `_run_background_task` 錯誤收斂 |
| 2026-04-23 | `kb-build-production-wiring`（D-032） | `OpenAIEmbeddingProvider`（`text-embedding-3-small` dim 1536）+ `wire_kb_dependencies` factory DI + dim-mismatch guard + `/healthz` `openai_embedding` 三態 |
| 2026-04-23 | `kb-query-endpoint` | `POST /kb/query` 同步查詢 + `kb_query_provider` factory 帶 `default_module="kb_query"` 拆帳 |
| 2026-04-23 | `usage-tracker-dedup` | TrackedProvider 加 `default_module`，變成 `module` 欄唯一寫入路徑；KB 不再手動 `tracker.record(...)` |
| 2026-04-23 | `chat-provider-wiring` | `OpenAIChatProvider`（instructor-wrapped `gpt-4o-mini`）+ `OpenAIContextLengthError`/`OPENAI_CONTEXT_EXCEEDED` + 三個 chat-ish role factory（`llm_reasoning_provider` 0.1 / `llm_judge_provider` 0.0 / `llm_chat_provider` 0.2）+ `/healthz` `openai_chat` 三態；M1 `No outbound LLM traffic during M1` 不變式退役，由 `Outbound LLM traffic gated by TrackedProvider whitelist` 取代 |
| 2026-04-24 | `explorer-react-loop-p0` | Module 4 Explorer ReAct skeleton：`codebus_agent.agent` 子套件（`types` / `protocols` / `explorer` / `judge` / `reasoning_logger` / `prompts`）；`run_explorer` 六步主迴圈（Think→Act→Observe→Judge→Log→Update）+ `_should_stop` 三分支收斂（budget / queue / cancel）+ `_MIN_STATIONS_FOR_CONVERGENCE=3`；`LLMJudge` one-shot + `ReasoningLogger` append-only JSONL（`explorer_prompt_version` / `judge_prompt_version` 寫每行）；`ExplorerTools` / `Judge` / `CoverageChecker` 三個 `@runtime_checkable` Protocol（day-1 抽象 for Q&A 共用）；Coverage 遞迴 hook 以 `_COVERAGE_RECURSION_ENABLED=False` 夾住，由 `coverage-gap-recurse` 後續打開 |
| 2026-04-24 | `explorer-tools-p0` | Module 4 四個 P0 真工具：`codebus_agent.agent.tools` 子套件（`FolderTools` / `SearchHit` / `DirEntry`）；`search`（KB preferred + grep fallback，≤100 hits，text-file 副檔名過濾）/ `list_dir`（一層 + `.codebus` 排除）/ `read_file`（Pass 1 sanitize + line_range slice + 12k char truncate；`ctx.sanitizer=None` fail-loud）/ `mark_station`（`relevance=0.8` hardcoded + `(path, role, why)` 冪等）；`ToolContext` 加 optional `kb` / `usage_tracker` 欄位；`ExplorerTools` Protocol 加 optional `tool_specs()`；`sandbox.append_tool_audit_line` 共用 writer 讓 FolderTools 與 ToolSandbox 同格式寫 `tool_audit.jsonl`（第二層稽核填入）；紅隊（`../..` / symlink）全 allowed / denied 入 audit |
| 2026-04-24 | `agent-sse-wiring` | Module 4 Explorer SSE 通電：新 `explorer-sse` capability + 新 `POST /explore` endpoint（`TaskKind` 加 `explore`、regex 擴為 `^(scan\|kb\|explore)_[0-9a-f]{8}$`、`_run_background_task` 包裝沿用）；`codebus_agent.agent.emitter`（`@runtime_checkable SSEEmitter` Protocol + `NullEmitter` / `TaskHandleEmitter`）；`codebus_agent.agent.context_vars`（`current_phase_var` / `current_step_var` / `current_session_var`）；`run_explorer(..., emitter=None)` 每輪 emit `agent_thought` / N 個 `agent_action_result`（observation truncated ≤500）/ `judge_verdict` / `progress`（`total` 在迴圈前 snapshot）；`TrackedProvider.__init__(..., emitter=None)` + `set_emitter(emitter)` 成功 path emit `usage_delta`（失敗不 emit，`session_total_cost_usd` 本地累加）；`LLMCallLogger.__init__(..., emitter=None)` + `set_emitter(emitter)` emit `llm_call`（`preview` 取第一個 user message content[:200]）；`LLMJudge.set_emitter` 轉發到內部 TrackedProvider；emitter 以 `None` default 對既有測試全部相容（13 個 Explorer 舊測、14 個 TrackedProvider 舊測、6 個 LLMCallLogger 舊測全綠） |
| 2026-04-24 | `explorer-judge-golden` | Module 4 Judge prompt 升級 + 首份 golden baseline：新 `explorer-golden` capability；`JUDGE_SYSTEM` 改三段式（角色邊界 / station 判準 / follow-imports + `relevance` 五檔錨 `0.0/0.3/0.5/0.8/1.0`）；`render_judge_prompt(state, results)` 改吃 `ExplorerState`（visited 前 20 + `... (N more)` / stations count + 最近 3 條 / ToolResult output 截 800 字，錯誤塞 `error=<msg>`）；`JUDGE_PROMPT_VERSION` 改 date-version `2026-04-25-1`（`EXPLORER_PROMPT_VERSION` 不動）；`tests/golden/demo-synthetic/` 第一份 fixture（`workspace/src/{a,b,c}.py` + `expected.json` 5 欄：stations `(path, role)` set / `stopped_reason=budget_exhausted` / `step_count=3` / 兩 prompt version）；`sidecar/tests/golden/test_explorer_replay.py` 7 測（3 個 expected.json shape + 1 個 scripted MockProvider replay + 3 個 drift guards：station set / prompt version `re-baseline required` / reasoning_log 行數）；`_golden_root()` 用 `Path(__file__).resolve().parents[3]` 絕對解析，不吃 cwd |
| 2026-04-24 | `explorer-tools-p1` | Module 4 P1 差異化武器：`trace_import(symbol) -> str \| None` / `find_callers(symbol) -> list[FileMatch]` 掛在既有 `FolderTools`；language-neutral regex template（Python `def`/`class` + TS/JS `class`/`function`/`const`/`let`/`var` + Go `func`/`type` + Rust `fn`/`struct`/`enum`/`trait`，`re.escape(symbol)` 防注入）；`FileMatch(path, line, snippet)` 新 Pydantic schema；`_iter_allowed_paths_sorted` 共用 `(path_depth, rel_path)` 排序 walker；`trace_import` 第一個命中即 early-exit + symlink 指外部寫 `tool_audit.jsonl` `allowed=false`；`find_callers` 用 `\b<sym>\b` whole-word + per-file ≤ 5 + global ≤ 100 + `(path_depth, path, line)` 排序 + Pass 1 sanitize 截 200 字（命中寫 `sanitize_audit.jsonl` `pass_num=1`）+ `ctx.sanitizer=None` fail-loud + 排除 `trace_import` 回的 def line；`FolderTools.tool_specs()` 從 4 個擴到 6 個；`_AUDIT_FIELDS` 兩個新工具 whitelist 刻意為空（`symbol` 歸 Agent 自由文字）；`test_folder_tools_structural.py` 的「unknown tool name」placeholder 從 `trace_import` 改 `find_nonexistent`；tool-side 全測 30 passed / 1 symlink skipped（Windows）|
| 2026-04-24 | `coverage-gap-recurse` | Module 4 Coverage 補查閉環通電：翻開 `_COVERAGE_RECURSION_ENABLED=True` + 新常數 `_COVERAGE_MAX_DEPTH=3`（遞迴條件 `_depth + 1 < _COVERAGE_MAX_DEPTH`，合法 depth 0/1/2）；新 `codebus_agent.agent.coverage.LLMCoverageChecker`（類 `LLMJudge` one-shot、吃 `provider_factory + workspace_root`、`set_emitter` 轉發到內部 TrackedProvider）+ 新 prompt 模組 `codebus_agent.agent.prompts.coverage`（三段式 `COVERAGE_SYSTEM`、`render_coverage_prompt` visited window 20 + `... (N more)` footer、`COVERAGE_PROMPT_VERSION="2026-04-26-1"`）；`run_explorer` 新 keyword-only `_depth: int = 0`、main while 退出後呼 `coverage.check` 一次、`_coverage_skip_reason(gaps, budget_ok, depth_ok)` 按 `no_gaps > max_depth_reached > budget_exhausted` 優先序、`_enqueue_gap_investigation(state, gaps)` 雙推（`pending_queue` 塞 `suggested_target` 或 `gap:<desc[:80]>` placeholder、`messages` 塞一條 `role="user"` 摘要 gap ≤3 條）、coverage round `Step` 只在 `len(gaps)>0` 時寫（`thought=[coverage] round-{depth+1} gaps={N} will_recurse={bool}`，`step_count` 不自增）；新 `coverage_gaps` SSE event（`round`/`gaps`/`will_recurse`/`skip_reason`，emit 塞在 check 回傳後 + Step 寫入前；無 gaps 仍 emit `skip_reason="no_gaps"`）；innermost `stopped_reason` 沿遞迴鏈原樣傳回；HTTP 層新 `app.state.llm_coverage_provider` factory（`_make_chat_provider_factory` 帶 `default_module="coverage"` + `temperature=0.0` + `role=JUDGE`）+ `_require_explore_deps` 加 coverage slot 檢查 503（missing 列出 `llm_coverage_provider`）；`POST /explore` handler 以 `LLMCoverageChecker` 取代 `_NullCoverage`；測試新增 20 測（6 prompts/`check` + 11 遞迴/SSE + 1 loop 改寫 + 2 endpoint 相容），agent-layer 全測 20 passed |
| 2026-04-25 | `context-compression-token-budget` | Module 4 Explorer 長跑不炸閉環：TrackedProvider 加 `session_prompt_tokens` / `session_completion_tokens` / `session_total_tokens` 三個 in-memory counter（成功 chat / embed 累計、失敗不累，對稱既有 cost 機制）；`usage_delta` SSE event 新增 `session_total_tokens` 欄位（嚴格 additive）；新 `codebus_agent.agent.budget` 模組（`@runtime_checkable TokenBudgetProbe` Protocol + `AggregatedTokenProbe` 聚合跨 reasoning / judge / coverage 三 provider）；`LLMJudge.provider` / `LLMCoverageChecker.provider` 新 read-only property；`run_explorer` 新 keyword-only `token_probe: TokenBudgetProbe \| None = None`、`_should_stop` 擴四分支（precedence: cancel > tokens > steps > queue_empty）、`ExplorerResult.stopped_reason` Literal 擴 `"budget_tokens_exhausted"`；新 `_MESSAGE_ROLLING_WINDOW=16` + `_think` 送 provider 的 messages slice 到最近 16 條（`state.messages` 本尊不切、`reasoning_log.jsonl` 完整記錄）；新 `_BUDGET_WARNING_PCT=0.8` + `_BudgetWarningState` sticky flags + `_maybe_emit_budget_warning(...)` emit `budget_warning` SSE event（per-kind 每 run 最多一次、`kind in {"tokens","steps"}`、Update 後 progress 前）；warning state 沿 `_warning_state` kwarg 穿遞迴保 per-run once；HTTP 層組 `AggregatedTokenProbe([reasoning, judge.provider, coverage.provider])` 餵給 `run_explorer`；測試新增 20 測（5 TrackedProvider counter + 4 budget probe + 6 token budget enforcement + 5 budget_warning + 4 rolling window + 2 provider property + 1 endpoint wiring），既有 21 測（providers + agent + api）擴欄不破 |
| 2026-04-25 | `golden-sample-baseline` | Module 4 Explorer 評估基礎設施落地（步驟 23 P0）：新 `sidecar/tests/golden/scoring.py`（test-only utility，零 `codebus_agent.*` import）—— `station_recall(produced, must_have) -> float`（空 must_have raise `ValueError("must_have_paths cannot be empty")`）/ `station_noise(produced, must_have, nice_to_have) -> float`（空 extras 回 0.0 不 raise）/ `composite_score(recall, noise, depth, weights=None) -> float`（D-006 公式 `0.5*r + 0.3*(1-n) + 0.2*d`、override 缺 key 必 raise `KeyError`）/ `IdealRoute(BaseModel)` 四欄無 default；新 fixture `tests/golden/timeline-storage-adapter-synthetic/`（11 檔共 240 行：`README.md` + `ideal-route.json` 9 路徑分類 + 9 個 `workspace/...` ≤ 40 行 stub 對齊 `timeline-gdrive-adapter/ideal-route.md` 站 1-4 拓撲）—— must_have 5（types/MockStorageAdapter/LocalFileAdapter/useStorage/timeline）/ nice_to_have 2（node/settings）/ noise 2（EventCard/README）；新 `sidecar/tests/golden/test_timeline_synthetic_replay.py` 10 測（4 fixture schema drift guard + 6 全 stack scripted replay）—— `_run_full_stack_replay(workspace_dir, must_have_paths)` 把 reasoning + judge + coverage 三 MockProvider + LLMCoverageChecker + AggregatedTokenProbe + `_SpyEmitter` 全 wire 起來，5 iter 命中 5 must_have 路徑（`should_follow_imports=True` 讓 queue 持續非空、避開 `_MIN_STATIONS_FOR_CONVERGENCE=3` 在 iter 4 觸發 queue_empty 提前停）；assertions 鎖 recall=1.0 / noise=0.0 / composite ≥ 0.9 / 至少 1 筆 `coverage_gaps`（will_recurse=False、skip_reason="no_gaps"）/ 恰 1 筆 `budget_warning(kind="steps", current=4, budget=5, pct=0.8)` + 0 筆 `kind="tokens"`（4/5=0.8 是 production `>=` 閾值的自然邊界 — 詳見 `sidecar/tests/agent/test_budget_warning_event.py:99`）/ 每筆 `usage_delta` 帶 non-negative int `session_total_tokens`；production code 零改動；既有 demo-synthetic 7 測 + scoring 11 測 + replay 10 測 = 28 測 golden 全綠；D-006 後續清單兩 `[x]`、保留 live LLM `[ ]` 給後續 change |

**目前沒有 in-progress change**。下一步依 `docs/implementation-plan.md`：**步驟 24** Module 5 Generator P0（per-station prompt + validator + degraded fallback + tutorial.md/route.json 多檔輸出 D-029）。

## 架構快照

**混合架構**（D-001）：Tauri 2.0 殼（Rust）↔ Python Sidecar（FastAPI）↔ Qdrant（本地向量 DB）。前端 Nuxt 3 + TypeScript + Tailwind。IPC 走 `127.0.0.1:<random-port>` + Bearer token（`docs/sidecar-api.md §一`、`openspec/specs/sidecar-runtime/spec.md`）。

**sidecar 啟動協定**（M1 已實作）：Tauri spawn binary with `--parent-pid <pid>` → sidecar 首行 stdout 印 `{"port":<int>,"bearer":"<≥32 chars>"}` → Tauri 解析後用 bearer 打 `/healthz` → 200 即通電。parent process 消失 5 秒內 sidecar 自殺（watchdog）。

**八大 Module**（`README.md §五`，M2+ 才動工）：
- Module 1 Scanner → Module 2 KB Builder → Module 4 Explorer Agent → Module 5 Generator → Module 7 前端 → Module 8 Q&A Agent
- Module 3（Topic Explorer）Phase 2；Module 6（Intervention）前端實作期決定
- Module 5 輸出多檔（D-029）：`tutorials/{task_id}/tutorial.md`（MOC 索引）+ `stations/s{NN}-slug.md`（每站一檔，含 YAML frontmatter + stable station id；跨檔用標準 markdown link，禁 wikilinks）

**Agent 核心**（D-012）：自寫 ReAct loop + Instructor/Pydantic structured output。Explorer 與 Q&A Agent **共用** ReAct core，靠 `ExplorerTools` / `Judge` / `CoverageChecker` Protocol 抽象（`docs/agent-explorer-spec.md §十二`）。

**LLM 呼叫鏈**（M1 + llm-role-routing + kb-build-production-wiring + chat-provider-wiring 合成）：所有 provider 必須包 `TrackedProvider` 裝飾器——registry 在實例化階段 raise 拒絕 unwrapped provider。允許的 inner class 是一個顯式 allowlist：`TrackedProvider.ALLOWED_INNER_TYPES = {MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}`；要擴增 provider（Ollama / Anthropic）必須在新 change 裡同步改 spec `Outbound LLM traffic gated by TrackedProvider whitelist` 與 code allowlist。測試 suite 用 `respx` mock OpenAI wire。分派機制走 `ProviderRole`（`reasoning` / `judge` / `chat` / `embed`）：呼叫端用 `registry.get(role)` 或 app state 的 `llm_<role>_provider(workspace)` factory 取對應 TrackedProvider。`TrackedProvider` 建構必帶 `role`、`default_module` kwarg，每筆 `token_usage.jsonl` / `llm_calls.jsonl` 記錄都帶 `role` + `module`（後者由 `usage-tracker-dedup` 引入，是 module 欄的唯一寫入路徑）。`llm_calls.jsonl` 含 `sanitizer_pass2_applied` 欄位：sanitizer-safety-chain 後真的 Pass 2 hit 即 true。

**Trust Layer 四站**（Phase A，敘事核心 — 評審會停在這邊）：
- **R-01** Workspace（主畫面 + 六層 audit 面板）
- **O-04** LLM Call Inspector（R-01 內 slide-in panel，秀 wire payload）
- **O-05** Sanitizer Diff（LOCKED/UNLOCKED 稽核畫面）
- **O-01** Grant Modal（workspace 授權）

**三段 Sanitizer**（D-015）：Pass 1 Scanner 入 KB 前（`scanner-sanitizer-orchestration` 落地）→ Pass 2 Provider pre-flight 每次 LLM call 前（`sanitizer-safety-chain` 落地，由 `TrackedProvider` 注入 `SanitizerEngine` + `SanitizerAuditLogger`）→ Pass 3 Q&A `add_to_kb` 寫入前（待 Module 8 Q&A P0）。詳見 `docs/sanitizer.md §三`。

**七層 Audit JSONL**（workspace-level 六層 + App-level 一層；implementation status 真實標注）：

實作狀態：✅ 已實作 / ⏳ 待對應 Module 落地 / 📐 design-only（spec 在 docs/，capability spec + writer 待對應 change）

- ✅ `sanitize_audit.jsonl`（Sanitizer 命中；Pass 1 Scanner + Pass 2 Provider 都會寫，帶 `pass_num` 欄；位於 `<ws>/.codebus/`）
- ✅ `tool_audit.jsonl`（Sandbox 工具呼叫；Module 4 Explorer 實作時填入；位於 `<ws>/.codebus/`）
- ⏳ `kb_growth.jsonl`（Q&A `add_to_kb`；待 Module 8 P0 落地，需先補 `kb_growth` capability spec + Pass 3 sanitizer hook，見 `docs/reviews/2026-04-25-stage-4.md` Module 8 預警）
- ✅ `reasoning_log.jsonl`（ReAct 每 step；Module 4 實作時填入；位於 `<ws>/`）
- ✅ `token_usage.jsonl`（D-021 + `usage-tracker-dedup`）：唯一寫入路徑是 `TrackedProvider`，`module` 欄由構造時的 `default_module` 寫入（目前值：`kb_build` / `kb_query` / `reasoning` / `judge` / `chat` / `coverage`）；位於 `<ws>/`
- ✅ `llm_calls.jsonl`（D-022）：完整 wire payload，`sanitizer_pass2_applied` 在 sanitizer-safety-chain 後真會翻 true；位於 `<ws>/`
- 📐 `~/.codebus/authorization_audit.jsonl`（跨 workspace，App-level）—— 完整設計在 `docs/authorization.md`（410 行 spec），`openspec/specs/authorization-audit/` capability + writer + 4 sidecar endpoints 待 Stage 6 步驟 26.5 `auth-flow` change 落地（見 `docs/implementation-plan.md`）

**Audit 路徑不一致是已知 latent risk**（前 2 層在 `<ws>/.codebus/`、後 3 層在 `<ws>/` root）：對應前端 R-01 panel 設計時的決策點，已記錄在 `docs/reviews/2026-04-25-stage-4.md` Cat 2.5-B；建議統一到 `<ws>/.codebus/`，動工時間 TBD。

## 關鍵不變式（寫 spec / code 時必守）

1. **雙模 discriminator day 1**（D-002）：`workspace_type: "folder" | "topic"` 欄位從一開始就寫進 schema；MVP 只實作 `folder`，但 `topic` 加進來不能造成 breaking change。`ToolContext`（`sidecar/src/codebus_agent/sandbox.py` + `docs/tool-sandbox.md §三`）、`POST /scan`（`docs/sidecar-api.md §三`）、`authorization_audit`（`docs/authorization.md §五`）都遵守此約。
2. **Sanitizer 單向**：placeholder `<REDACTED:kind#N>` 無 reverse mapping，一旦替換即不可逆；原值「不額外儲存」，原檔在本機原處，不 copy 到 KB/log/網路。
3. **LLM 看到的一定是 Sanitize 過的**：`llm_calls.jsonl` 記的是 post-Sanitizer Pass 2 版本，不還原 pre-sanitize 原文（D-022）。`sanitizer_pass2_applied` 欄位在 sanitizer-safety-chain 後才真會翻 true（M1 舊行為 false）；不變的是此欄位永遠存在、型別不變。
4. **Provider 必包 TrackedProvider + allowlist 同步**：registry guard 在實例化階段攔截 unwrapped provider（`sidecar/src/codebus_agent/providers/`）；而且只有 `TrackedProvider.ALLOWED_INNER_TYPES` 裡列的 inner class 才能包入。目前 allowlist 是 `{MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}`——要加新 live provider 必須同步修 `openspec/specs/llm-provider/spec.md` 的 `Outbound LLM traffic gated by TrackedProvider whitelist` Requirement，spec 列的 allowlist 與 code 列的 ALLOWED_INNER_TYPES 不可分歧。
5. **Bearer + loopback 不可鬆綁**：sidecar 只 bind `127.0.0.1:0`（ephemeral）、bearer token 記憶體常駐不落盤（D-local-2）；任何 endpoint 不得跳過 bearer middleware。
6. **ensure_in_workspace 紅線**：所有檔案操作必須先過 `ensure_in_workspace(path, ctx)`（`Path.resolve(strict=False)` + `is_relative_to`）——覆蓋 `..` 逃逸、symlink、Windows UNC、`\\?\` long-path 全譜系。紅隊 fixture 在 `sidecar/tests/sandbox/`。
7. **檔名 kebab-case**：`docs/*.md`、`design/*.html`、`design/screenshots/*.png` 一律 `{代號}-{語意}`。舊版直接刪，不留 `-v1` 後綴（歷史去 git log 找）。
8. **Spec 為主、mockup 其次**：`design/*.html` 與 `docs/*.md` 衝突時以 spec 為準，回頭修 mockup。
9. **Sanitizer rules 改動必 bump version**：`docs/sanitizer.md` 的 rule pattern 有任何增減，必須同步 bump rules version；`docs/authorization.md §六` 規定使用者同意需依版本重取。不得「靜默改 rule」——會造成既有 workspace 套用新 rule 但未重授權，稽核鏈斷裂。

## 決策記憶 — `docs/decisions.md`

所有非 trivial 的技術取捨都寫成 **D-XXX ADR**（脈絡 / 選項 / 理由 / 後續）。Spec 首行必引相關 D-XXX。改決策時**先改 `decisions.md`，再改引用它的 spec**。常查：
- D-001 混合架構 / D-002 Topic mode 不進 MVP / D-003 LLM Provider 抽象（2026-04-20 role routing 落地 → 2026-04-23 `chat-provider-wiring` 接上 chat-ish live provider，M1 「No outbound」不變式退役）
- D-011 資安 / D-012 自寫 ReAct / D-014 uv toolchain
- D-015 Sanitizer / D-016 Q&A add_to_kb / D-017 ToolSandbox
- D-021 token_usage / D-022 llm_calls（`module` 欄由 `usage-tracker-dedup` 收束成單一寫入路徑）
- D-026 前端 toolchain 改 npm（原本 bun）
- D-027 Qdrant 走 local binary 主路徑（docker compose 降為 fallback）
- D-028 LLM Vision 延後至 Phase 2（介面不預埋 Capability enum）
- D-029 Module 5 多檔輸出（MOC + `stations/s0X-slug.md` + frontmatter + stable station id）+ 拒絕 Obsidian 整合
- D-032 KB build production wiring（`text-embedding-3-small` dim 1536 hard-coded、`CODEBUS_OPENAI_API_KEY` 唯一 env 來源、不 fallback `OPENAI_API_KEY`、SDK retry 不再疊）

## 常用指令

**Python Sidecar**（`sidecar/`，uv toolchain · D-014）
```bash
cd sidecar
uv sync                            # 裝依賴 + 建 venv
uv run pytest                      # 全測（~698 passed + ~19 skipped；Qdrant / symlink 相關環境相依會自動 skip）
uv run pytest tests/sandbox/       # 紅隊 path-escape 專測
uv run pytest tests/providers/     # Mock / Tracked / OpenAIEmbedding / OpenAIChat / UsageTracker / LLMCallLogger
uv run pytest tests/qdrant/ -v     # smoke test（需 Qdrant 起來才會跑）
uv run pytest tests/test_wire_kb_dependencies.py  # DI hook 八 slot + 兩 healthz probe
uv run pytest -k healthz           # 按關鍵字挑單測
uv run python -m codebus_agent.api.main           # 獨立起 sidecar（讀 port/bearer 看 stdout）
uv run python -m codebus_agent.api.main --healthz # 自檢模式，印 JSON 不起 HTTP
# Real-endpoint smoke test — loads CODEBUS_OPENAI_API_KEY from repo-root .env 但不 echo 到 stdout
uv run python scripts/smoke_chat_provider.py      # 驗 /healthz openai_chat + 一次真實 chat call + token_usage 拆帳
```

**Qdrant 本地 binary**（D-027）
```bash
# 先把 qdrant binary 放到 ~/.codebus/bin/qdrant(.exe)，或設 $CODEBUS_QDRANT_BIN
bash sidecar/scripts/start-qdrant.sh      # POSIX
pwsh sidecar/scripts/start-qdrant.ps1     # Windows
# 存放路徑走 env var（Qdrant 1.x 無 --storage-path flag）：
#   QDRANT__STORAGE__STORAGE_PATH / QDRANT__STORAGE__SNAPSHOTS_PATH
# Fallback：docker compose -f sidecar/docker-compose.qdrant.yml up -d
```

**前端**（`web/`，npm — D-026）
```bash
cd web
npm install
npm run dev          # http://localhost:3000（cargo tauri dev 也會自動跑這個）
npm run typecheck
npm run generate     # 出 SPA 到 .output/public，給 cargo tauri build 吃
```

**Tauri 殼**（`tauri/src-tauri/`，Rust stable ≥ 1.80）
```bash
cd tauri/src-tauri
cargo tauri dev      # 自動 spawn web + sidecar（透過 externalBin）
cargo test
cargo tauri build    # 產 MSI + NSIS（Windows）/ AppImage / dmg；依賴 sidecar/dist/codebus-sidecar-<triple>(.exe)
cargo build --release -- ...  # 只編 codebus.exe，不重打 installer
```

**PyInstaller 打包鏈**（必須先產 sidecar binary 才能 cargo tauri build）
```bash
cd sidecar
uv run pyinstaller codebus-sidecar.spec
# 產出到 sidecar/dist/codebus-sidecar-<triple>(.exe)，被 tauri.conf.json externalBin 引用
```

**Commit gate**
```bash
uv tool install pre-commit    # 首次 setup
pre-commit install            # 裝 git 原生 hook
pre-commit run --all-files    # 全檔跑 stage-0 hook（trailing-ws / eof / check-yaml / check-json / mixed-line-ending）
bash tests/precommit_gate_test.sh          # 乾淨 repo 應全綠
bash tests/precommit_violation_test.sh     # 負測：故意違規 commit 應被擋
```

## Spectra worktree 慣例

用 `/spectra-apply <change>` 起手時，skill 會在 `.spectra/worktrees/<change>/` 開 git worktree。收尾後：
```bash
git merge --ff-only change/<name>
git worktree remove .spectra/worktrees/<name>   # 若殘留目錄就加 --force
git branch -d change/<name>
```
`.spectra/` 已在 `.gitignore`，worktree 不會汙染主 repo。

## 常見引用關係

改 spec 時容易漏掉的連動（`docs/README.md §五` 完整對照）：
- 改 `sanitizer.md` → 檢查 `authorization.md §六`（rules version bump 政策）、`sidecar-api.md` audit endpoints、`security.md` §3.x
- 改 `authorization.md` → 檢查 `sidecar-api.md` POST /scan schema（`workspace_type`）、`tool-sandbox.md §三` ToolContext、`design/o-01-grant-modal.html`
- 改 `agent-core.md` → 檢查 `agent-explorer-spec.md` §十二 trait、`qa-agent.md` §二、`prompts.md`
- 改 Module spec → 檢查 `implementation-plan.md` 依賴圖 + `sidecar-api.md` 對應 endpoint
- 改 M1 已封存的 capability（`openspec/specs/<cap>/spec.md`）→ 必須走 `/spectra-propose` 開新 change；不可直接改 archive 過的 spec
