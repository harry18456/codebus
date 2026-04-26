## Context

Module 8 Q&A 是 D-016 從 day-1 寫死的核心 capability，但 P0 動工延到所有資料層 + Module 4 / 5 通電後（per `docs/implementation-plan.md` 步驟 25）。本 change 動工時，下列基礎設施已 in-place：

- **Pass 1 / Pass 2 Sanitizer 已落地**（`sanitizer-safety-chain` 2026-04-21 archive、`scanner-sanitizer-orchestration` 同期），engine 簽名已宣稱「reusable by Pass 3 without signature change」
- **七層 audit JSONL 六層已通電**（`audit-path-unification` 2026-04-25 archive）：sanitize / tool / reasoning / token_usage / llm_calls 全在 `<ws>/.codebus/`；`kb_growth.jsonl` 是缺的最後一層
- **ReAct core 與 Protocol seam 已落地**（`explorer-react-loop-p0` 2026-04-24 archive）：`_think` / `_execute_tools` / `_should_stop` / `ReasoningLogger` / `ExplorerTools` Protocol / `Judge` Protocol / `CoverageChecker` Protocol — Q&A reuse 範圍由本設計決定（見 Decision 1）
- **TrackedProvider / chat cost / pricing-table** 已通電（`review-backlog-cleanup` 2026-04-25 archive）：`module="qa_agent"` 自動寫進 `token_usage.jsonl` 不需 caller 動手
- **`KBPayload`** schema 已宣告 `added_by` Literal 含 `"qa_agent"`、`related_stations` 已 enforce `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$` 格式（`module-2-kb-builder-p0` 2026-04-22 archive）
- **`KnowledgeBase.query` / `find_similar`** 已存在（`module-2-kb-builder-p0`），但缺 `filter_stations` 參數與 `upsert_chunk` 公開 API
- **`POST /generate`** endpoint 與 `task_id` regex `^(scan|kb|explore|generate)_[0-9a-f]{8}$` 已落地（`module-5-generator-p0` 2026-04-25 archive）— Q&A 沿同 pattern 加 `qa` 一個分支即可

Stage 4 review backlog 4 條 Module 8 預警同次解（`docs/reviews/2026-04-25-stage-4.md` Module 8 段）：`kb_growth.jsonl` writer 缺、Pass 3 sanitizer hook 缺、新工具 `audit_fields` 必宣告（spec 已 enforce，本 change 是合規不擴 spec）、Cat 3 #3 Judge / Coverage prompt fork（本設計 Decision 1 主動繞過 — Q&A 不 reuse Judge / Coverage instance，所以 prompt 沒得 fork）。

## Goals / Non-Goals

**Goals:**

- 提供 backend Q&A loop：使用者輸入問題 → RAG-first 回答（cheap path）→ 不夠則 ReAct loop 補查 → Agent 自主決定 `add_to_kb` 沉澱 → 同次走 Pass 3 Sanitizer + 寫 `kb_growth.jsonl`
- 第七層 workspace audit `kb_growth.jsonl` 寫入路徑唯一化（`KBGrowthLogger` 對齊 UsageTracker / LLMCallLogger 模式）
- `POST /qa` endpoint 與既有 `/scan` `/kb/build` `/explore` `/generate` 同 background-task 框架（task registry / SSE / 錯誤碼表）
- Q&A 與 Explorer 共用 ReAct 內部機件（避免兩套 loop drift），但**不共用** prompt instance / Judge instance / Coverage instance — 自然解決 Cat 3 #3 Folder-mode 詞彙污染問題

**Non-Goals:**

- 前端聊天 UI、引用 panel、KB Growth 稽核 tab — 步驟 30 / Phase 6 範圍
- KB rollback 機制（spec 預留 `rollback` event 形狀但不實作 writer / Qdrant delete API）
- 跨 session 記憶 / Active KB grooming / 多輪 planning / Topic mode 融合 — Phase 2+
- `agent-core` capability spec 動土（Protocol seam 已就位）
- `tool-sandbox` capability spec 動土（`audit_fields` 必填 rule 已存在）
- `usage-tracking` capability spec 動土（`module` 欄通用）
- ReAct core 行為變更（`_think` / `_execute_tools` / `_should_stop` 全 reuse）

## Decisions

### Decision 1: Q&A 不 reuse Judge / Coverage instance，自帶 prompts module

**選擇**：Q&A 自帶 `agent/prompts/qa.py`、自寫 `run_qa(question, state, ...)` 與 `_synthesize_answer`，**不**實例化 `LLMJudge` / `LLMCoverageChecker`。

**對比方案 A（reject）**：reuse Explorer 的 Judge / Coverage instance — 必須做 prompt fork（per Cat 3 #3 警告）：在 `judge.py` / `coverage.py` 加 `mode: Literal["explore", "qa"]` 參數，按 mode 切 system prompt。

**理由**：
- Q&A 的收斂條件不是「station 是否完備」（Explorer Judge 用途）也不是「coverage 缺口」（CoverageChecker 用途），而是「答案是否充分」+「budget 是否耗盡」— 走 `_should_stop` 的 budget / steps / wall 三條件就夠
- prompt fork 會造成 `judge.py` / `coverage.py` 兩個檔案各加 `if mode == "qa"` 分支，污染 Explorer 主路徑 — Q&A 演進時更動 prompt 也要小心不破 Explorer
- 自帶 prompts 的成本：~80 行 `prompts/qa.py` + ~150 行 `agent/qa.py`；ReAct mech 部分（`_think` / `_execute_tools` / `_should_stop` / `ReasoningLogger`）全 reuse，drift 風險可控
- Cat 3 #3 警告自然解決（沒 reuse instance 就不需要 fork prompt）

**取捨**：Q&A 與 Explorer 之間 prompt 不一致；若未來 Q&A / Explorer 共識「站點驗證」邏輯，需要再開 change 抽出共用 Judge — 目前 P0 不需要。

### Decision 2: RAG-first 兩階段（cheap path 先），不直接進 ReAct

**選擇**：`run_qa` 進 ReAct loop 前，先打一次 `kb.query(question, top_k=8)` 走 `_hits_confident` 判定；通過則直接 `_answer_from_hits` 回答（單次 chat call），不進 ReAct。

**`_hits_confident` 三條件全過才走 cheap path**（per `docs/qa-agent.md §四`）：
- top-1 score > 0.75
- top-3 平均 > 0.65
- top-5 涵蓋 question 中的關鍵實體（最簡單實作：把 question 拆字後比對 hits 的 text；Phase 2 可升級成 LLM-based entity extraction）

**對比方案 A（reject）**：永遠進 ReAct loop — 每個問題都跑 `_think` / `_execute_tools` 至少一輪。

**理由**：
- KB 已涵蓋 80%+ 常見問題（建好教材後的場景），多花一輪 ReAct 純浪費 token + cost
- cheap path 走完只有 1 次 chat call（`_answer_from_hits`），cost 拆帳（`token_usage.jsonl` `module="qa_agent"`）依然完整
- ReAct 入口仍保留：`_hits_confident=False` 走進去走 budget 限制範圍內的補查
- entity-coverage 條件主要為了避開「KB 表面 hit 高但實際對不上」（e.g., 使用者問「PaymentService 退款流程」，KB 只回了 PaymentService 介紹但 retain 沒提退款）

**取捨**：cheap path 的決策準（門檻 0.75 / 0.65）是經驗值，golden replay 後再 calibrate；本 P0 用 `docs/qa-agent.md §四` 數字當 baseline。

### Decision 3: Pass 3 Sanitizer source label `qa_add_to_kb`，沿用 FileSource 不擴 union

**選擇**：`add_to_kb` 走 `sanitizer.sanitize(text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))`；`sanitize_audit.jsonl` 命中時 `pass_num=3` + `source` 為結構化 `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}`。

**對比方案 A（reject）**：新增 `Pass3Source` dataclass — `SanitizeSource = FileSource | MessageSource | Pass3Source`。

**理由**：
- sanitizer spec 已宣稱「`SanitizeSource` 可 reusable by Pass 3 without signature change」— 改 union 違 spec
- `FileSource(pass_=)` 字串欄位 already supports 多 pass label 模式（scanner 用 `"scanner"`、generator 用 `"generator"`）— Q&A 加 `"qa_add_to_kb"` 是同 pattern 的第三個 instance
- pass_num 由呼叫端傳給 `sanitizer_audit.append(... pass_num=3 ...)`，與 source label 解耦；既有 `pass_num=1`（scanner）/ `pass_num=2`（TrackedProvider）/ `pass_num=3`（add_to_kb）三層稽核完整
- review #2 Pass 3 警告寫「`MessageSource` / `FileSource` 兩種 union 沒 `Pass3Source`」— 是觀察 unionb shape，不是 SHALL 加新類型；spec 已說 reusable

**取捨**：`source.pass` 字串需要 caller 自律維持 enum-like discipline（不能拼錯）— 風險低，因為單一 callsite 在 `add_to_kb`，常數化即可。

### Decision 4: `KnowledgeBase.upsert_chunk` 雙層 dedup，Layer 2 走 find_similar 重用

**選擇**：新公開 API `KnowledgeBase.upsert_chunk(text: str, *, payload: KBPayload) -> str`：
1. embed 一次（透過已綁的 provider）
2. Layer 1：`exists_by_hash(collection, payload.text_hash)` — 完全相同 text 已有
3. Layer 2：`find_similar(text, threshold=0.95)` — 向量相似度 ≥ 0.95
4. 兩層任一命中：不 upsert / 不消耗 embed token，回 `"dedup:hash"` 或 `"dedup:sim"`
5. 都不命中：Qdrant upsert，回 point_id

**對比方案 A（reject）**：在 `add_to_kb` tool 裡自己組 embed + dedup 邏輯。

**理由**：
- KB 層已有 `exists_by_hash` + `find_similar`，封裝在 `upsert_chunk` 是內聚（`add_to_kb` 不需要碰 embedding / Qdrant 細節）
- Layer 2 threshold 0.95 對齊 `docs/qa-agent.md §七` 防呆表 + 使用者「kb 已快滿」場景的去重需求
- 失敗回字串而非例外：`add_to_kb` 拿到 `"dedup:..."` 仍寫 `kb_growth.jsonl` 但標 `dedup_skipped: true` — UI 可顯示「Agent 想加但被 dedup 擋下」的稽核脈絡

**取捨**：embed 之前就 hash dedup 省一次 embed token；Layer 2 仍要 embed 一次才能算相似度（無法避）— 但 query path 本來就 embed，cost 攤平。

### Decision 5: Q&A 任務啟動前所有 dependency slot 必須齊備

**選擇**：`POST /qa` handler 進 `_run_background_task` 前，由 `_require_qa_deps(app.state)` 檢查 5 個 slot：`kb_provider` / `kb_query_provider` / `kb_growth_logger_factory` / `llm_chat_provider` / `llm_judge_provider`；任一缺即回 503 `QA_NOT_CONFIGURED` 並列出缺哪些（per Module 5 同模式）。

**對比方案 A（reject）**：lazy check — 跑到 `add_to_kb` 才發現 `kb_growth_logger_factory` 沒 wire，半途 fail。

**理由**：
- Q&A 是長跑流程（~10 step + 多次 LLM call），半途失敗使用者體驗很差
- fail-fast 對齊 sidecar 整體 degraded-but-alive 契約：缺 OpenAI key 就在 startup 標 `not-configured`，使用者看 `/healthz` 即知道
- `llm_judge_provider` 雖然 Q&A 不 reuse Judge instance（per Decision 1），但仍為 ReAct fallback 階段保留 future-proof — 若未來 Q&A 變要 self-check 答案品質，可以掛上去而不破 endpoint

**取捨**：dependency check list 是 hardcode 列表；新增 dependency 時要記得擴 `_require_qa_deps`。

### Decision 6: `task_id` regex 直接擴一個 `qa` 分支

**選擇**：`^(scan|kb|explore|generate)_[0-9a-f]{8}$` → `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`。

**對比方案 A（reject）**：泛化成 `^[a-z]+_[0-9a-f]{8}$` — 任何小寫前綴都接受。

**理由**：
- 顯式 enum 比泛 regex 安全：未來新 endpoint（e.g., `topic` Phase 2）必須走 spec MODIFIED 加進 regex，避免拼寫錯誤的 task_id 過 validation
- 既有 5 個 (scan/kb/explore/generate/qa) prefix 仍是固定集合，加進 `TaskKind` Literal 同步擴
- spec MODIFIED 一條 Requirement、test fixture 同步加 qa case — work 可控

**取捨**：每加新 endpoint 都要過 spec MODIFIED — 但這就是不變式 enforcement 的本意。

### Decision 7: `kb_growth.jsonl` 預留 `rollback` event 形狀但 P0 不寫 rollback 路徑

**選擇**：`kb_growth.jsonl` schema 第一行寫死的欄位是 add 路徑（`entry_id` / `source` / `reason` 等）；spec 描述 schema 時明寫「P1 rollback 走相同 JSONL append `event_type="rollback"` 路徑」但 P0 writer 不暴露 rollback API。

**對比方案 A（reject）**：P0 直接做 rollback writer + Qdrant delete API。

**理由**：
- P0 焦點是 add 路徑通電 + audit 寫入正確；rollback 涉及 Qdrant delete + UI confirm flow + 二次 sanitize 檢查（避免 rollback 後留下 sanitize 不完全的 trace）— 屬 P1 frontend change 範圍
- schema 預留：未來 P1 加 rollback 不需 schema migration，只要 add 一個 optional `event_type: "add" | "rollback"` 欄位（default `"add"`）即可向後相容
- `KBGrowthLogger.write(...)` 簽名 P0 不接受 `event_type` 參數，writer 內部寫死 `"add"`；P1 擴簽名加 keyword-only `event_type` 參數即可

**取捨**：spec 描述要寫得清楚 rollback 是 P1 範圍，避免 reviewer 以為 schema 沒考慮 rollback。

### Decision 8: tool `audit_fields` 不收錄 free-text 欄位

**選擇**：
- `kb_search` audit_fields = `["query", "top_k", "station_filter"]`（**不**含 `query` 文字內容會在 audit 出現是合理的 — 它是檢索意圖的關鍵，且檢索本身不是高敏感操作）
- `add_to_kb` audit_fields = `["source", "reason", "related_stations"]`（**不**含 `chunks[*].text` — text 內容已經在 sanitize_audit.jsonl 留下命中紀錄、tool_audit.jsonl 只需要知道「呼叫了 add_to_kb，source 是 X，reason 是 Y」）

**對比方案 A（reject）**：audit `chunks[*].text` 全文。

**理由**：
- 不變式 #2「Sanitizer 單向替換」+「LLM 看到的一定是 sanitize 過的」— `tool_audit.jsonl` 是 tool 呼叫稽核，把 chunks 文字塞進去等於 pre-sanitize 原文落地一份，違反不變式
- `add_to_kb` 走 sanitize 後寫 KB，chunks 文字命中替換已記錄到 `sanitize_audit.jsonl`（pass_num=3）；要追溯內容請查 sanitize_audit + Qdrant point — 不需要 tool_audit 重複
- `kb_search.query` 字串包含的是 user question 摘要 / Agent 自寫的查詢字串，不會比 question 本身更敏感；audit 它是合理的（與 Agent 決策軌跡一致）

**取捨**：`add_to_kb` 失敗除錯時，`tool_audit.jsonl` 看不到 chunks 文字 — 但失敗會 raise 例外，例外 traceback + sanitize_audit 已足夠 debug。

## Risks / Trade-offs

- **`_hits_confident` 三條件門檻是經驗值**：top-1 > 0.75 / top-3 > 0.65 / top-5 entity coverage 全過 — Demo workspace 的實際表現未驗證，可能太鬆（cheap path 回錯）或太緊（永遠走 ReAct）→ Mitigation：P0 用 `docs/qa-agent.md §四` baseline 值；golden replay test 加一條「KB 涵蓋的問題走 cheap path、KB 沒涵蓋的問題走 ReAct」雙路徑，drift 即時抓
- **Layer 2 similarity dedup 0.95 閾值偏高**：`gpt-4o-mini` embedding 相似 chunk 在常見描述變體（同義改寫）下可能落 0.85-0.90 區間，0.95 閾值會放過幾乎只是「換句話說」的重複 → Mitigation：P0 沿 `docs/qa-agent.md §七` 數字；P1 加 UI 提示「KB 可能有同義重複」並提供合併工具
- **Q&A 不接 Judge 沒法擋「Agent 答錯」**：cheap path 一次 chat 回答可能被 KB hit 誤導（hit 表面相關但實際對不上）→ Mitigation：Decision 2 entity-coverage 條件擋掉一部分；Phase 2 升級成 LLM-based 答案 self-check；本 P0 接受偶發誤答，反正使用者可以再問一次
- **`kb_growth.jsonl` 只 append 不 rollback** P0 缺一致性手段：使用者點 rollback button 但 P0 沒 wire → Mitigation：P0 前端不暴露 rollback button（步驟 30 P1 才做），UI 只顯示 KB 新增成功 / 失敗
- **Q&A budget 較緊（10 step / 50k token / 60s wall）**：複雜問題會 budget 耗盡 → Mitigation：超 budget 時 `_synthesize_answer` 仍要 attempt 用已收集的資訊回答，並 prompt 約束「資訊不足，建議讀 X / Y 檔案」（per qa-agent.md §九）
- **`kb_search` 不過濾敏感 hit**：KB 裡的 chunk 已是 Pass 1 sanitize 後內容，但 `kb_search` 回給 Agent 的 snippet 可能包含 placeholder（e.g., `<REDACTED:secret#3>`）→ Mitigation：placeholder 是設計就是要給 Agent 看的（這是稽核 chain 的價值），不擋；Agent prompt 約束「snippet 含 `<REDACTED:*>` 時不能猜原值」
- **`docs/sidecar-api.md` 與 `docs/qa-agent.md` 可能落差**：兩個 doc 描述 SSE event 細節時可能 drift → Mitigation：本 change 同次更新 `docs/sidecar-api.md §三` `POST /qa` + §四 `qa_answer` / `kb_growth` SSE event；spec 為主、doc 為次（per CLAUDE.md 不變式）

## Migration Plan

無 backward-incompatible 改動。檢查清單：

- 既有 workspace 開啟後沒有 `<ws>/.codebus/kb_growth.jsonl` — 第一次跑 Q&A 自動建檔（`KBGrowthLogger` auto-mkdir）
- `KBPayload.added_by` 已有 `"qa_agent"` Literal — 既有 KB 不影響
- `task_id` regex 擴 — 舊 task_id（scan / kb / explore / generate）仍合法
- 既有 `/scan` `/kb/build` `/explore` `/generate` endpoint 全不受影響
- 全 suite baseline 756 passed → 預期 ~800+ passed（含本 change 預估 ~40+ 新測）
