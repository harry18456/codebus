## Why

`docs/implementation-plan.md` **步驟 24 Module 5 Generator P0** 是 Stage 5 第一塊（教材與 Q&A 階段）。Module 4 Explorer 已交付完整 ExplorerResult（stations / reasoning_log / coverage / golden sample 全套 P0 + P1），但**沒有 tutorial 出口**：使用者跑完 explore 看到的只是 stations 清單與 reasoning_log JSONL，無法當教材消費。Module 5 把 stations 轉成可學習的多檔 markdown 教材。

完整設計已在 `docs/module-5-generator.md`（488 行 spec）涵蓋輸入 / 輸出 / 流程 / prompt / validator / SSE / 失敗處理。本 change 是**從設計到 spec + code 落地**的轉換，並鎖死 propose 階段的 4 條 design decisions（從 `docs/reviews/2026-04-25-stage-4.md` Module 5 預警 + Cat 3 latent risk #5 抽出）。

對齊 **D-029**（多檔 + MOC + frontmatter + stable station id + 拒絕 Obsidian 整合）、**D-006**（golden sample evaluation；本 change 落地後可跑 Module 4→5 全鏈 golden）、**D-016**（Q&A 接續；MOC 結尾的 `<QAEntry>` 為 Module 8 入口預留）、**D-022**（audit chain 純度；Generator output 過 Pass 1 Sanitizer 維持「離開 sidecar 的內容都已清理」承諾）。

## What Changes

**A. 新 capability `module-5-generator`**（核心 SHALL 條款）

- `codebus_agent.generator.run_generator(state, llm_chat_provider, kb, options)` async entrypoint，接受 `ExplorerResult.stations` + `KnowledgeBase` + `task` + `options(mode, target_persona)`，回 `GeneratorResult(tutorial_path, station_paths, route_path, log_path, degraded_count)`
- Per-station LLM call pipeline（`_generate_station(...)`）：context build（related_files + KB hits + previous_stations_summary + stable id 已配發）→ `provider.chat(messages, response_model=StationMarkdown)` → validator → retry up to 3 → degraded fallback
- Validator pipeline（`validate_station_markdown(md, idx) -> ValidationResult`）：`<Checkpoint>` ≥ 1 / `<Quiz>` ≤ 1 / 長度 ≤ 800 字元 / code block ≤ 30 行 / `<CodeRef>` 路徑在 workspace
- Stable station id (`s{NN}-{slug}`) generation：title → kebab-case slug ≤ 40 char + collision suffix `-2`/`-3`
- frontmatter renderer (YAML) 含 D-029 §7.3 schema（13 欄，含 `station_id` / `degraded` / `required_checks`）
- MOC assembler：`tutorial.md` 純索引頁（站列表 + standard markdown link + 結尾 `<QAEntry>`）
- `route.json` writer：D-029 §八 schema（含 `station_id` + `file_path` + `required_checks` + `degraded`）
- Plain mode prompt 模板分支（`<Checkpoint>` → `- [ ]` / `<Quiz>` → `> 思考題` / 保留 `###` 分頁）

**B. 新 sidecar endpoint `POST /generate`**（`sidecar-runtime` capability MODIFIED）

- request body：`{workspace_root, task, stations: [...], task_id, options: {mode, target_persona}}`
- response：`{task_id: "generate_<8hex>"}`，async background task 走 `_run_background_task` 既有錯誤收斂
- task_id format Requirement 主文擴 `^(scan|kb|explore|generate)_[0-9a-f]{8}$`
- Background task error containment Requirement 補 `GENERATE_FAILED` error code + scenario
- single-slot task registry 既有規則延伸（一個 generate 在跑時 block 其他 task creation）

**C. SSE event 擴充**（`sidecar-runtime` MODIFIED）

- 新增 `progress.phase="generating"` event 變體：`status ∈ {generating, validating, retry, writing_file, assembling_moc}` + `current_station` / `total_stations` / `station_id` / `file_path` 欄
- 與 Explorer 既有 `usage_delta` / `llm_call` / `progress` event 共用 channel，per-task `TaskHandle` 配發

**D. Multi-file output 結構落地**（D-029 §七）

- `<ws>/codebus-tutorials/{task_id}/tutorial.md` — MOC 索引頁
- `<ws>/codebus-tutorials/{task_id}/stations/s{NN}-{slug}.md` — 每站一檔（含 frontmatter）
- `<ws>/codebus-tutorials/{task_id}/route.json` — 結構化路線
- `<ws>/.codebus/generator_log.jsonl` — operational log（per-station 生成 / retry / degraded 紀錄；放 `.codebus/` 與 `reasoning_log.jsonl` 同層）

**E. 4 條 design decisions 鎖定（propose 階段必確認）**

E-1. **Generator output 過 Pass 1 Sanitizer** ✅ YES
- 落地點：每站 markdown 寫檔前 `SanitizerEngine.sanitize(content, source=FileSource(...))` 一次；`tutorial.md` MOC 同樣處理
- 理由：defense in depth — LLM input 雖已 Pass 1（Scanner）+ Pass 2（TrackedProvider）sanitize，但 LLM 是 creative entity，可能：(a) 從 sanitized placeholder 反推（理論上單向不可逆，但 prompt injection 場景需防）(b) synthesize secret-like patterns（合成 fake JWT 結構造成假警報但更糟是合成 real-looking secret）(c) echo 未被 source-side scan 涵蓋的內容（如 LLM 模型 weights 訓練時記住的 public secret）
- 成本：每站 markdown ~800 字元，Pass 1 regex scan 微秒級
- Audit：每次 sanitize 命中寫 `<ws>/.codebus/sanitize_audit.jsonl` `pass_num=1` `source=generator`（既有 audit chain 自然延伸，不需新 layer）
- 拒絕方案：(B) 信任 LLM input 已 sanitize（過度信任 LLM）/ (C) prompt 明指示 LLM 不 echo（LLM 紀律不可靠）

E-2. **Station.depends_on backfill 留 P1，不在本 P0** ❌ NO
- 落地點：本 change `_update_state` 維持 hardcode `depends_on=[]`；Generator 不解析 MOC graph 反向回填 ExplorerResult
- 理由：scope discipline — depends_on backfill 涉及 MOC graph parser（parse `related_stations` frontmatter + build inverted index + 防 cycle）是獨立 capability，混入 Module 5 P0 會把 ~3d 變 ~5d
- 影響：`composite_score(recall, noise, depth=1.0)` 的 `depth` placeholder 暫保（與 `golden-sample-baseline` 已 land 的 P0 行為一致）；待 P1 / follow-up `depends-on-backfill` change 解
- Generator 仍輸出 `route.json` 帶 `related_stations: [s01-x, s03-y]`（從 frontmatter 抓出來），只是 ExplorerResult.stations[i].depends_on 不被反向回填——**前端可從 route.json 直接讀 station 之間關係**，不依賴 ExplorerResult depends_on
- 拒絕方案：(A) 本 P0 一起做（scope creep ~2d）

E-3. **輸出根目錄 `<ws>/codebus-tutorials/{task_id}/`**（**改 docs/module-5-generator.md L27 `tutorials/` → `codebus-tutorials/`**）
- 落地點：runtime 寫 `<workspace_root>/codebus-tutorials/{task_id}/...`；endpoint return 與 SSE event 的 `file_path` 全用此 root
- 理由：教材是 **user-facing product**（使用者打開 IDE 直接看），非 audit；放 `.codebus/` 隱藏目錄不直觀。原 spec 用 `tutorials/` 太 generic 容易撞既有 user folder。`codebus-tutorials/` 明確標示來源 + 非隱藏 + 不衝突
- 拒絕方案：(a) 對齊舊 spec `tutorials/` — generic name 風險高 / (c) 隱藏在 `.codebus/tutorials/` — 反 user-facing
- 連動：`docs/module-5-generator.md` L27 / §七 layout 圖 / 內部跨檔 link 一律改 `codebus-tutorials/`；`<CodeRef>` 路徑與 `related_files` 仍指 workspace 內原始檔（不變）

E-4. **degraded fallback per-station stub + retry quota 3**（對齊既有 spec §十）
- 落地點：`_generate_station(...)` 內 retry loop max 3，failed 後產 minimal markdown（核心文字 + 一個 checkpoint，不出 quiz）寫入 station 檔，frontmatter `degraded: true`，log 寫 `generator_log.jsonl`；其他 station 不受影響（多檔隔離）
- 理由：per-station stub 模式既有 spec 寫死，Demo 場景下整 generate 失敗體驗最差（評審看了一眨眼就沒了），per-station stub 至少展示大致結構
- 邊界：全站都 degraded → route.json 頂層補 `degraded: true` UI 提示重跑；station 寫入 disk 失敗（disk full）→ log error 不重試
- 拒絕方案：(B) 整 generate 回滾 — Demo killer / (C) per-station 跳過 — 教材結構破裂更糟

## Non-Goals

- **Live LLM golden snapshot**：D-006 後續清單 `[ ] 打磨期` —— Module 5 P0 用 scripted MockProvider golden（與 Module 4 同模式），真 LLM snapshot 留打磨期 follow-up change
- **Q&A Agent 接續**：MOC `<QAEntry>` 元件渲染由 Module 8 P0 落地（D-016）；本 change 只在 markdown 輸出元件 tag，前端互動不在範圍
- **Vue 元件渲染（Checkpoint / Quiz / CodeRef / Reveal / QAEntry）**：前端 Stage 6 步驟 26-30；本 change 只負責 markdown 文字輸出
- **`<CodeRef>` / `<Reveal>`**：spec §十四 列在 P1，本 P0 不做（reduces 0.5d）
- **Generator log UI 查看**：spec §十四 列在 P1
- **Multi-language（英 / 日）/ 多選題 / 圖解生成 / PDF 匯出**：spec §十三 MVP 不做
- **`Station.depends_on` backfill**：見 E-2，留 follow-up change
- **Vision capability**：D-028 已決 Phase 2，介面不預埋
- **改 ExplorerResult schema**：本 change 純消費 `Station(path, role, relevance, why, depends_on=[])`，不擴 Station 欄位
- **改 audit chain 任何層**：generator_log.jsonl 是 operational log（per-Module 性質）非「七層 audit JSONL」之一，與 review tracker 列的七層體系平行

**拒絕的設計**

- **「per-iteration LLM call 一次產整份 tutorial」**：違反 D-029 多檔輸出 + per-station 隔離原則，單站失敗影響全份
- **「frontmatter 用 TOML / JSON」**：YAML 是既有 mdc 慣例（與 Nuxt @nuxtjs/mdc 對齊）
- **「station 連結用 wikilinks `[[s02-storage]]`」**：D-029 §十六.1 不變式禁，破壞 GitHub render
- **「整份 tutorial 寫進 `<ws>/.codebus/`」**：教材是 user-facing product，audit 才 hidden
- **「Pass 1 結果不寫 sanitize_audit.jsonl」**：違反 sanitizer spec 「每次 Pass 1 命中 MUST 寫 audit」契約

## Capabilities

### New Capabilities

- `module-5-generator`：Markdown Generator 核心 SHALL 條款（per-station pipeline / validator / stable id / frontmatter / MOC assembler / route.json writer / Pass 1 over output / degraded fallback / SSE generating events）

### Modified Capabilities

- `sidecar-runtime`：
  - `task_id format` Requirement 擴正則 `^(scan|kb|explore|generate)_[0-9a-f]{8}$`（新 Scenario `Generate kind follows same shape`）
  - `Background task error containment` Requirement 主文加 `POST /generate` + `GENERATE_FAILED` error code（新 Scenario `Generate task exception surfaces as safe error event`）

**Sanitizer-engine capability 不需 delta** — `SanitizerEngine.sanitize(text, source: SanitizeSource)` 已 generic 接受任意 `FileSource(path=...)`；Generator 對 station markdown 呼叫 `sanitize(content, source=FileSource(path=output_path))` 完全在 既有 SHALL 條款 `SanitizerEngine exposes pure sanitize interface` 範圍內，無新 Requirement / 新 Scenario。

## Impact

**受影響 spec**：

- `openspec/specs/module-5-generator/spec.md`（**新建** — 完整 capability spec）
- `openspec/specs/sidecar-runtime/spec.md`（MODIFIED — 兩條 Requirement）

**受影響 production code（新檔）**：

- `sidecar/src/codebus_agent/generator/__init__.py`（套件 init）
- `sidecar/src/codebus_agent/generator/types.py`（`StationMarkdown` Pydantic / `ValidationResult` / `GeneratorResult` / `Frontmatter` schema）
- `sidecar/src/codebus_agent/generator/runner.py`（`run_generator` async entrypoint + per-station loop）
- `sidecar/src/codebus_agent/generator/station.py`（`_generate_station` LLM call + retry + Pass 1 + write file）
- `sidecar/src/codebus_agent/generator/validator.py`（`validate_station_markdown` + 各條 issue）
- `sidecar/src/codebus_agent/generator/stable_id.py`（slug 規範化 + 碰撞處理）
- `sidecar/src/codebus_agent/generator/frontmatter.py`（YAML render + schema 驗證）
- `sidecar/src/codebus_agent/generator/moc.py`（MOC `tutorial.md` assembler）
- `sidecar/src/codebus_agent/generator/route.py`（`route.json` writer）
- `sidecar/src/codebus_agent/generator/log.py`（`generator_log.jsonl` writer，append-only，落 `<ws>/.codebus/`）
- `sidecar/src/codebus_agent/generator/prompts/__init__.py`（`STATION_SYSTEM` interactive / plain 兩變體 + `render_station_prompt(context)` + `STATION_PROMPT_VERSION` date-version）
- `sidecar/src/codebus_agent/api/generate.py`（new endpoint `POST /generate` + dependency wiring + handle generate task）
- `sidecar/src/codebus_agent/api/_audit_paths.py`（補 `_GENERATOR_LOG_FILENAME = "generator_log.jsonl"` 常數）
- `sidecar/src/codebus_agent/api/__init__.py`（router include `generate.py`）
- `sidecar/src/codebus_agent/api/tasks.py`（`TaskKind` 加 `generate`、regex 擴）

**受影響 production code（修改）**：

- `sidecar/src/codebus_agent/agent/types.py`（新增 `GeneratorResult` 還是放 `generator/types.py`？放 `generator/types.py` — 與 Module 4 types 隔離）

**受影響 docs**：

- `docs/module-5-generator.md`（連動更新：L27 `tutorials/` → `codebus-tutorials/`、§七 layout 圖、§八 route.json schema 路徑樣本對齊）
- `docs/implementation-plan.md`（步驟 24 標 `🚧 in-progress`，archive 後改 `✅ landed`）
- `docs/decisions.md` D-029 連動更新清單補 `[x] 多檔輸出落地（module-5-generator-p0）`
- `CLAUDE.md`（archive 表加新 row + 七層段已不需動 / Module 5 狀態描述更新 / 「下一步」改指向 Module 8 Q&A P0）

**受影響 fixture / golden**：

- 新 `tests/golden/module-5-generator-synthetic/`（mock stations 3 站 + mock KB hits + expected `tutorial.md` / `stations/s0X-*.md` / `route.json` shape）
- 既有 `tests/golden/timeline-storage-adapter-synthetic/`：本 change 不直接動，但 follow-up live-LLM snapshot 可在此 fixture 跑完整 Explorer→Generator 鏈

**受影響 tests**：

- 新 `sidecar/tests/generator/`：unit 測 validator / stable_id / frontmatter / moc / route 各條 + integration 測 `run_generator` end-to-end + degraded scenario
- 新 `sidecar/tests/api/test_generate_endpoint.py`：endpoint shape + 401 / 409 / single-slot 行為 + SSE event sequence
- 新 `sidecar/tests/golden/test_generator_replay.py`：scripted MockProvider 跑 3 站 + assert MOC + station 檔 + route.json structure

**無新依賴**（所有 LLM call 走既有 `llm_chat_provider` factory + TrackedProvider）。

**無 schema breaking change**（純加新 capability + endpoint，既有 ExplorerResult / KnowledgeBase / TrackedProvider 形狀不變）。

**Migration**：無——新 module，既有 workspace 沒 `codebus-tutorials/` 目錄。`POST /generate` 是新 endpoint，現有 client 不受影響。

**估計工期**：~3.0d P0（依 spec §十四 表）。
