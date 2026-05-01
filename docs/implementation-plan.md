# CodeBus 實作順序總表

> 跨模組依賴關係與實作步驟 — 把各 module spec 的「實作順序」章節縫成一條主線。
> 關聯：`../README.md §九`（高層時程）、各 `module-*.md` / `agent-*.md` §實作順序（細項工期）。

---

## 一、五條強制規則（不可延後）

這五件事必須在對應觸發點**之前**就落地，否則後面 retrofit 成本極高。

| 規則 | 必須在此之前落地 | 理由 |
|---|---|---|
| **Sanitizer Pass 1 + 2 可用** | 第一次 LLM call（步驟 16） | 一旦有 code/doc 送出 LLM，未 sanitize 的資料就可能外流；先有再擋 |
| **Sandbox `ensure_in_workspace` helper 可用** | 第一個真 tool 實作（步驟 17） | Tool 沒 sandbox 等於開後門；retrofit 要改每個 tool |
| **`reasoning_log.jsonl` 寫入** | Explorer ReAct loop 第一次跑（步驟 18） | Demo 靈魂，也是 golden sample 迭代的依據，不可補做 |
| **`UsageTracker` 串到 Provider** | 第一次 LLM call（步驟 16） | Token / cost 之後要靠它做 Budget + D-007 benchmark + Demo 數據；補做要改每個 call site（D-021） |
| **`LLMCallLogger` 串到 Provider** | 第一次 LLM call（步驟 16） | 完整 request/response 稽核 + Demo 透明度證據；補做要改每個 call site（D-022） |

---

## 二、七階段 30 步

### 第一階段：基建與協議（~6.5d）

立地基，Tauri 殼、Sidecar、Sandbox、Provider 都先可通。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 1 | Monorepo 骨架（`tauri/` / `sidecar/` / `web/` / `tests/fixtures/`）+ uv 初始化 | 0.5d | — | D-013 / D-014 / dev-setup.md |
| 2 | Tauri 2.0 + Nuxt3 Hello World + fs.scope 設定 | 1d | — | README §五 / tool-sandbox.md §七 |
| 3 | FastAPI sidecar 模板 + Bearer token + `/healthz` | 0.5d | — | sidecar-api.md §一 |
| 4 | Tauri ↔ Sidecar HTTP ping（localhost + 隨機 port） | 0.5d | 2, 3 | sidecar-api.md §一 |
| 5 | **ToolContext + Sandbox `ensure_in_workspace` helper + red team fixture** | 1d | 3 | tool-sandbox.md §二 / §十五 |
| 6 | PyInstaller 打包驗證 | 0.5d | 3 | dev-setup.md |
| 7 | Qdrant 本地跑 + Python client 連通 | 0.5d | 3 | module-2-kb-builder.md §三 |
| 8 | LLM Provider Protocol + 供應商 API 實作 + Instructor 串好 | 0.5d | 3 | llm-provider.md / D-012 |
| 8.5 | **UsageTracker + LLMCallLogger 骨架**（兩層稽核 JSONL + TrackedProvider wrapper） | 1.5d | 8 | agent-core.md §十三 / D-021 / D-022 |

### 第二階段：安全鏈（~3d）

在 Day 1 就把三段式 Sanitizer 的前兩段落地，Pass 3 等到 Q&A 階段。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 9 | Sanitizer Pass 1：detect-secrets + PII regex + placeholder | 1d | 8 | sanitizer.md §九 P0 |
| 10 | Sanitizer Pass 2：Provider pre-flight 掛點 | 0.5d | 8, 9 | sanitizer.md §二 / D-015 |
| 11 | `sanitize_audit.jsonl` + `tool_audit.jsonl` 兩 logger | 0.5d | 5, 9 | security.md §四 |
| 12 | Sanitizer config 載入 + schema 驗證 | 0.5d | 9 | sanitizer.md §六 |

### 第三階段：資料層（~5d）

Module 1 → Module 2，讓 Explorer 有東西吃。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 13 | **Module 1 P0**：遍歷 + gitignore + binary/encoding + ScanResult + **scanner Pass 1 Sanitizer 串通**（openspec `scanner-sanitizer-orchestration` 已落地 2026-04-21） | 2d | 5, 9 | module-1-scanner.md §十六 P0 |
| 14 | **Module 2 P0**（Scanner Pass 1 已解鎖）：Qdrant wrapper + KBPayload + chunk + embed pipeline + content-hash 去重 — 2026-04-21 落地（change `module-2-kb-builder-p0`） | 2.5d | 7, 8, 13 | module-2-kb-builder.md §十三 P0 |
| 15 | Module 1/2 SSE progress emit 串通 — 2026-04-22 落地（change `sse-progress-skeleton`） | 0.5d | 13, 14 | sidecar-api.md §四 |
| 15.5 | KB build production wiring（OpenAI embedding + `wire_kb_dependencies` factory + dim-mismatch guard）— 2026-04-22 提案（change `kb-build-production-wiring`, D-032） | 2.5d | 15 | module-2-kb-builder.md §七 / llm-provider.md §三-bis |

### 第四階段：Agent 核心（~10d，Demo 靈魂）

照 `agent-core.md §十七` 的 Day 1-10 走。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 16 | types + mock provider + Explorer 最小 ReAct loop | 2d | 8, 10 | agent-core.md §五 |
| 17 | 真工具（search / list_dir / read_file / mark_station）+ 串 KB | 2d | 5, 14, 16 | agent-explorer-spec.md §九 P0 |
| 18 | Relevance Judge（極簡 prompt）+ **reasoning_log 寫檔** | 1d | 16 | agent-core.md §十二 |
| 19 | trace_import / find_callers | 1d | 17 | agent-explorer-spec.md §九 P1 |
| 20 | ✅ landed（`coverage-gap-recurse`）：`LLMCoverageChecker` + 遞迴（`_COVERAGE_MAX_DEPTH=3`、`_enqueue_gap_investigation` 雙推、`coverage_gaps` SSE event、HTTP 層 `llm_coverage_provider` factory） | 1d | 18, 19 | agent-core.md §七 / §九 |
| 21 | ✅ landed（`context-compression-token-budget`）：`_MESSAGE_ROLLING_WINDOW=16` + `TokenBudgetProbe` / `AggregatedTokenProbe` + `_should_stop` 四分支（cancel > tokens > steps > queue）+ `budget_warning` SSE event（per-kind once）+ HTTP 層聚合三 provider `session_total_tokens` | 1d | 16 | agent-core.md §十 / §十一 |
| 22 | SSE emit（agent_thought / judge_verdict / action_result） | 1d | 18 | sidecar-api.md §四 |
| 23 | ✅ landed P0（`golden-sample-baseline`，2026-04-25）：scoring helpers (`station_recall` / `station_noise` / `composite_score` / `IdealRoute`) 在 `sidecar/tests/golden/scoring.py` 落地、`tests/golden/timeline-storage-adapter-synthetic/` 9 檔對齊 ideal-route.md 拓撲、`test_timeline_synthetic_replay.py` 全 stack scripted replay 鎖 recall=1.0 / noise=0.0 / composite ≥ 0.9 + coverage_gaps + budget_warning(steps@4/5) + usage_delta.session_total_tokens；live LLM snapshot 留待後續 change（D-006 `[ ] 打磨期`） | 1d | 17-22 | D-006 / tests/golden/... |

### 第五階段：教材與 Q&A（~5d）

Explorer 的產出變成 tutorial.md，Q&A 讓 KB 活起來。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 24 | ✅ landed P0（`module-5-generator-p0`，2026-04-25 archive）：per-station prompt + validator + degraded fallback + `tutorial.md`/`route.json`/per-station markdown 多檔輸出。Decision 1（Generator output 過 Pass 1 Sanitizer，YES）/ Decision 2（`Station.depends_on` backfill 留 follow-up，`route.json` `prerequisites=[]` P0 hardcode）/ Decision 3（root 為 `<ws>/codebus-tutorials/{task_id}/` 而非 generic `tutorials/`）/ Decision 4（per-station retry quota=3 + degraded stub 隔離 + disk write 失敗不重試）。落地：`codebus_agent.generator` 套件 9 module（`runner` / `station` / `validator` / `stable_id` / `frontmatter` / `moc` / `route` / `log` / `prompts/`）+ 11 條 `module-5-generator` capability Requirement + 2 條 `sidecar-runtime` MODIFIED（task_id format 擴 `^(scan\|kb\|explore\|generate)_[0-9a-f]{8}$`、Background task error containment 加 `GENERATE_FAILED`）+ `POST /generate` endpoint + `app.state.llm_generate_provider` factory（`role=CHAT` `default_module="generate"` `temperature=0.4`）+ 第六 workspace audit filename `_GENERATOR_LOG_FILENAME`（落 `<ws>/.codebus/`）。49 generator unit/integration + 4 endpoint test = 53 新測；全 suite 751 passed / 19 skipped | 2.5d | 23 | module-5-generator.md §十四 P0 / openspec/specs/module-5-generator |
| 25 | ✅ landed P0（`module-8-qa-p0`，2026-04-26 archive；後續 `review-2-critical-fix` 2026-04-26 修 dedup tuple + qa_answer 字面）：`kb_search` + RAG-first 兩階段 loop + `add_to_kb`（Sanitizer Pass 3） + `kb_growth.jsonl` 第七層 audit。落地：`codebus_agent.agent.qa` (`run_qa` + `_hits_confident` + 5 budget 常數) / `agent.prompts.qa` (`QA_SYSTEM` + `render_qa_prompt` + `QA_PROMPT_VERSION="2026-04-26-1"`) / `agent.tools.kb_search` + `agent.tools.add_to_kb` + `agent.tools.qa_tools.QATools` (七工具集合) / `kb.growth_logger.KBGrowthLogger` 唯一 writer + `_KB_GROWTH_FILENAME` path 常數 / `KnowledgeBase.query.filter_stations` + `upsert_chunk` 雙層 dedup（`tuple[str, str]` 簽名 by `review-2-critical-fix`） / `POST /qa` endpoint + `app.state.kb_growth_logger_factory` + `app.state.llm_qa_provider` (`module="qa_agent"`) + `task_id` regex 擴 `qa` + `QA_FAILED` 錯誤碼 / `ReasoningLogger(mode="qa")` 寫 `qa_prompt_version` 排除 explorer/judge versions / SSE event `rag_hits` / `kb_growth` / `qa_answer`。**P1 follow-up**（留 Phase 2）：(1) 多輪 session 記憶（跨 question budget 累計）/ (2) `add_to_kb` rollback 路徑（`event_type="rollback"` 寫 kb_growth.jsonl + Qdrant point delete）/ (3) KB ops UI（清舊 entry / 重 embed / KB 體積監測）/ (4) `qa_answer` 欄位級 streaming（per-token delta）| 2.5d | 14, 17, 18 | qa-agent.md §十一 P0 / openspec/changes/archive/2026-04-26-module-8-qa-p0 |

### 第六階段：前端（~8d）

Markdown 互動 + Agent console（Demo 神器）+ 介入點。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 25.5 | **共用骨架**（`tailwind.config.ts` design tokens + `layouts/default.vue` 三段 grid + `TopBar.vue` + 七 tab `AuditPanel.vue` + `useSidecar` / `useSseTask` composable + Tauri `sidecar_handshake` command）— Phase 6 page 級動工的前置依賴，**已完成 `phase6-shell` archive 2026-04-27** | 1.5d | 25 | openspec/specs/frontend-shell/ |
| 26 | ✅ landed（`r-01-station-board`，與步驟 27 合併實作）：`@nuxtjs/mdc` + Checkpoint.vue / Quiz.vue / QAEntry.vue 三個 mdc 互動元件契約落地，dumb + emit pattern；progress.json 寫入透過 `useTutorialProgress` composable 統一路徑 | 2d | 24, 25.5 | interactive-tutorial.md §三 / openspec/specs/interactive-tutorial/ |
| 26.5 | **Auth flow（O-01 modal + `~/.codebus/authorization_audit.jsonl` writer + 4 sidecar endpoints）** —— `auth-flow` change 落地（2026-04-27 archive），新 capability `openspec/specs/authorization-audit/`（4 ADDED Requirements）+ `sidecar-runtime` / `frontend-shell` 各 1 Modified Requirement；校正 `docs/authorization.md` §五 / §六 / §十一 三處 spec drift。**Trust Layer Act 1 第一幕通電**，與 R-01 / O-04 / O-05 同列敘事核心；**已完成** | 4d | 22, 26 | docs/authorization.md / openspec/specs/authorization-audit/ |
| 27 | ✅ landed（`r-01-station-board`，與步驟 26 合併實作）：MOC 首頁 + 站牌頁 + StationLayout/Nav/Content/MOCIndex + 解鎖邏輯 + D-T11 implicit-latest task_id fallback + D-T12 `###` 次級切頁 + D-T13 empty CTA + 3 個新 Tauri command（`read_tutorial_file` / `write_progress_file` / `list_tutorial_tasks`）+ 14 case 紅隊測 | 2d | 26 | interactive-tutorial.md §六 / openspec/specs/interactive-tutorial/ |
| 28 | ✅ landed `agent-console-p0`（2026-04-29）：page `/explorer/[task_id]` + 4 console 元件（`ConsoleTimeline` / `StepCard` / `ProgressStrip` / `CoverageBanner`） + `useExplorerStream` composable + AuditPanel reasoning tab 接通；apply 期同時 ingest 補裝 vitest infra（vitest + @vue/test-utils + happy-dom，proposal「vitest 已在 web/ 內」事實錯誤已校正）；37/37 vitest 全綠 | 1.5d | 22 | sidecar-api.md §四 |
| 28.5 | ✅ landed `llm-call-inspector-p0`（2026-04-29）：page `/audit/llm` standalone + `<LlmCallInspector>` 4-tab drawer overlay（Wire payload / Response / Tokens & cost / Timeline）+ `useAuditJsonl` 橫切 composable + Tauri `read_audit_jsonl` IPC（七層 enum + 5 MiB cap + Rust↔Python filename parity test）+ AuditPanel `select-row` emit；Explorer page `llm` tab 接通 live-tail。Pre/post sanitize diff defer 另開 audit-unlock capability | 1d | 22, 28 | agent-core.md §十三.2 / D-022 |
| 28.6 | ✅ landed `sanitizer-audit-inspector-p0`（2026-04-29）：page `/audit/sanitizer` standalone + `<SanitizerAuditInspector>` overlay（10 metadata 欄位 + D-015 banner sticky + rule explainer 從 `useSanitizerRules` 拉）+ `useSanitizeAudit` thin wrapper composable + sidecar 新 endpoint `GET /sanitizer/rules`（builtin + user_yaml registry 唯讀）+ AuditPanel sanitize tab placeholder chip / pass chip + 注入 R-01 station / Explorer console page。3-pane raw/sanitized diff、unlock-with-grant flow、auto-relock countdown、raw retention、`audit_session_id` chain 整套 defer 至 P1+ `sanitizer-audit-unlock`（避免 P1 啟動時得反向考古）— D-015 不變式未動。`sanitizer-audit-unlock` 後續啟動須先做 ADR 評估 raw retention threat model | ~1d | 25.5, 22 | docs/sanitizer.md / sanitize_audit.jsonl schema / openspec/specs/sanitizer-audit-inspector/ |
| 29 | ✅ landed `phase6-step29-intervention-points`（2026-04-30）：三個介入點（路線調整 / 重跑 / 換資料夾）落地 — `useIntervention` module-level singleton + `<InterventionConfirmModal>` / `<SkipStationButton>` / `<RegenStationButton>` / `<SwitchWorkspaceMenu>` 4 leaf 元件；`useTutorialProgress` 加 `skipped_station_ids` schema（additive，舊 progress.json 讀為 `[]`）+ unlock 規則改 `completed ∪ skipped`；`POST /generate` 加 optional `target_stations: list[str] | None`，sidecar runner 新 partial-regen path（命中站 byte-overwrite，MOC + route.json byte-identical，station_id drift 拒 + log 雙錄）；`<TopBar>` workspace chip 用 SwitchWorkspaceMenu 取代既有 emit；`useIntervention` 不直接呼 `writeProgressFile`（defensive grep 守 single-writer）。三 capability MODIFIED + 0 新 capability（per D-020）。35/35 intervention vitest + 6/6 sidecar partial regen pytest 全綠；既有 184/184 vitest + 966/968 sidecar pytest baseline 同步 | 1.5d | 23, 27, 28 | D-020 |
| 30 | ✅ landed `qa-overlay-p0`（2026-04-29）：drawer overlay（不是 page，保留站學習脈絡）+ `useQaSession` module-level singleton + `<QAOverlay>` / `<QaTurnCard>` / `<QaCitations>` 元件 + `<QAEntry>` mdc 改 imperative + 全域 Cmd+K 召喚 + ESC / dim layer 關閉 + AuditPanel `kb_growth` tab live-tail（dual-source merge：disk read + SSE）+ `useAuditJsonl` 加 `liveTailFromQaSession` opt（spec ADDED Requirement，不破既有 llm 那條）+ R-01 station page provide `currentStationId` 注入給 mdc 元件。`turns` FIFO cap 50 / drawer 480px 不可拖曳 / file:line 不可點 / `↶ rollback` 不渲染（KB ops UI 留 Phase 2 per qa-agent.md §十） | 1d | 25, 28 | qa-agent.md §八 / openspec/specs/qa-overlay/ |
| 31 | ✅ landed `provider-settings-and-onboarding`（2026-05-01，**D-033 B**）：Tauri keyring IPC（`keyring_set` / `keyring_get` / `keyring_delete`，紅隊 14 case + happy path 4 case）+ sidecar `POST /internal/startup-config`（bearer 守 + secret-leak grep）+ `RegistryHolder`（`asyncio.Lock` 包 immutable 內層）+ `config/provider_pool.py` loader（兼容 legacy / 新 schema）+ 五 settings mutation endpoints + app-level SSE channel `GET /events?channel=app`（50ms 合併 emit）+ `/healthz.dependency` 三 lane（`llm_chat` / `llm_embed` / `pii`）+ `useProviderConfig()` module-level singleton + 三 settings 元件 + onboarding wizard 三 page（welcome / providers / done，keyring → upsert × 2 → setBinding × 4 ordered）+ Nuxt route middleware redirect to `/onboarding/welcome` + `<TopBar>` 齒輪入口 + `<LlmCallInspector>` / `<AuditPanel>` PII filter（`hidePiiDetection` default true，toggle banner emit）。1001 sidecar pytest + 234 web vitest + zero typecheck error baseline 守住。`docs/decisions.md` D-033 archive 段落、`docs/llm-provider.md` Provider pool schema 段、`docs/authorization.md §六` PII LLM rules version 影響段、`CLAUDE.md` setting / onboarding 啟動流程段同步落地 | 5d | 30 | openspec/specs/provider-settings/ + provider-onboarding/ + keyring-integration/ + sidecar-runtime/ + frontend-shell/ + llm-call-inspector/ |

### 第七階段：打磨與 Demo 準備（~5d）

反覆調 prompt、跨平台測試、打包、簡報腳本。詳見 README §九 第五階段。

### 打磨期候選（非承諾、時間充裕才評估）

- **App 內原生 graph view**（D-029 §連動更新）：
  視覺化 stations 之間的關聯（`route.json.stations[*].related_stations` + KB chunk `related_stations` 反查），作為 MOC 以外的 map-style 第二視圖。
  實作前須另開 decision 確認：(1) 是否搭 reasoning_log / Judge 分數顯示 station 深度 (2) 渲染引擎選型（vis-network / d3 / Cytoscape）(3) 互動範圍（hover 預覽 / 點擊跳 station 檔 / backlinks 面板）
  目前為 **評估項**，不計入 P0 工期；走到此處再看時間預算與 demo 價值決定是否開 change。

---

## 三、關鍵依賴鏈圖

```
[1] 骨架 ──┬─► [3] FastAPI ──┬─► [5] Sandbox ──────────┐
          │                  ├─► [7] Qdrant            │
          │                  └─► [8] Provider ──► [8.5] UsageTracker ⭐
          └─► [2] Tauri+Nuxt ─► [4] HTTP ping   │      │
                                                │      │
  [9] Sanitizer P1 ◄──────────────────────────  │      │
      │                                         │      │
      ├─► [10] Pass 2 pre-flight                │      │
      └─► [13] Module 1 ──► [14] Module 2 ──────┤      │
                                                │      │
                    [16] Explorer ReAct ◄───────┘      │
                         │                             │
                         ├─► [17] 真工具 ◄─────────────┘
                         ├─► [18] Judge + reasoning_log ⭐
                         ├─► [19] trace_import/find_callers
                         ├─► [20] Coverage ──► [23] Golden sample
                         └─► [22] SSE（含 usage_delta）
                                   │
                                   ├─► [24] Module 5 ──► [26] 前端元件
                                   └─► [25] Q&A ──┐
                                                  │
                                   [28] Agent console ◄── [22]（token/cost 即時顯示）
```

---

## 四、總工期

| 階段 | 工期 | 累計 |
|---|---|---|
| 一、基建與協議 | 6.5d | 6.5d |
| 二、安全鏈 | 3d | 9.5d |
| 三、資料層 | 5d | 14.5d |
| 四、Agent 核心 | 10d | 24.5d |
| 五、教材與 Q&A | 5d | 29.5d |
| 六、前端 | 9d | 38.5d |
| 七、打磨 | 5d | 43.5d |

**P0 合計約 43.5 工作天（約 8-9 週）**，對齊 README §九 5-7 週基準 + 2-3 週 buffer。

P1 項目（Git blame / `<CodeRef>` / `<Reveal>` / 稽核 UI / KB rollback / Monorepo 子模組解析）穿插在各階段尾段或打磨期，時間充裕再補。

---

## 五、里程碑檢核點

| 里程碑 | 完成條件 | 對應步驟 |
|---|---|---|
| M1：通電 | Tauri 按下按鈕能讓 sidecar 回 pong，PyInstaller 打包版也能跑，UsageTracker 單測過 | 1-8.5 |
| M2：安全落地 | Sanitizer 對 fixture 跑出正確 placeholder + audit log，red team 攻擊被擋 | 9-12 |
| M3：第一個 KB | Timeline 掃完 + 進 Qdrant + 可查到 `IStorageService`，`token_usage.jsonl` 有 embed 記錄 | 13-15 |
| M4：Agent 會動 | Explorer 在 Timeline 跑出 stations，reasoning_log + token_usage 完整 | 16-23 |
| M5：End-to-end | 輸入任務 → 產出 tutorial.md + route.json → 前端看得到 + 能 Q&A | 24-27 |
| M6：Demo ready | Agent console 即時顯示決策 + token/cost，三介入點可用，golden sample 分數過標 | 28-30 + 打磨 |

每個里程碑都有明確「看得到結果」，不至於寫兩週還不確定在做什麼。
