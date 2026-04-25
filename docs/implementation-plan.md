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
| 25 | **Module 8 Q&A P0**：`kb_search` + RAG loop + **`add_to_kb`（Sanitizer Pass 3）** + `kb_growth.jsonl` | 2.5d | 14, 17, 18 | qa-agent.md §十一 P0 |

### 第六階段：前端（~8d）

Markdown 互動 + Agent console（Demo 神器）+ 介入點。

| # | 項目 | 工期 | 依賴 | 關聯 spec |
|---|---|---|---|---|
| 26 | `@nuxtjs/mdc` + Checkpoint.vue / Quiz.vue / QAEntry.vue 元件 | 2d | 24 | interactive-tutorial.md §三 |
| 26.5 | **Auth flow（O-01 modal + `authorization_audit.jsonl` writer + 4 sidecar endpoints）** —— spec + code 同 change 落地，吃掉 `docs/authorization.md` 410 行設計，建立 `openspec/specs/authorization-audit/` capability。**Trust Layer Act 1 第一幕**，與 R-01 / O-04 / O-05 同列敘事核心 | 4d | 22, 26 | docs/authorization.md / openspec/specs/authorization-audit/（待 `auth-flow` change 建立）|
| 27 | 站牌列表 + 內容區 + progress.json 讀寫 + 解鎖邏輯 | 2d | 26 | interactive-tutorial.md §六 |
| 28 | **Agent console**（reasoning_log SSE stream）— Demo 神器 | 1.5d | 22 | sidecar-api.md §四 |
| 28.5 | **LLM Calls 分頁**（list + detail modal + filter）— Demo 透明度武器 | 1d | 22, 28 | agent-core.md §十三.2 / D-022 |
| 29 | 三個介入點（路線調整 / 重跑 / 換資料夾；Pinia + 既有 API） | 1.5d | 23, 27 | D-020 |
| 30 | 聊天 UI + 引用 panel + KB growth 稽核 tab | 1d | 25, 28 | qa-agent.md §十一 P0 / P1 |

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
