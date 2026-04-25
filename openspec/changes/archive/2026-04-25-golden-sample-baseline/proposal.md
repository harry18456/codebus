## Why

`context-compression-token-budget`（2026-04-25 archive，commit `6c8de61`）讓 Module 4 Explorer 全套 P0/P1 終於補齊（步驟 16-22 全部 landed）。下一塊 `docs/implementation-plan.md` **步驟 23 `Golden sample 首跑（Timeline ideal-route 對比）`** 是 Demo 靈魂的最後一塊驗證網——但目前 golden 基礎設施還只覆蓋 P0：

- **`tests/golden/demo-synthetic/`**（`explorer-judge-golden`，2026-04-25 landed）只 pin 5 欄（stations / stopped_reason / step_count / 兩 prompt_version），驗的是「Explorer + Judge 在 scripted MockProvider 下 stations 不漂」。這是 baseline 第一塊磚但**完全沒摸到** P1 的 trace_import / find_callers、`coverage-gap-recurse` 的遞迴 + `coverage_gaps` event、`context-compression-token-budget` 的 token budget + `budget_warning`。
- **`tests/golden/timeline-gdrive-adapter/ideal-route.md`**（D-006，2026-04-17）有手寫的 5 站 ideal route + recall/noise/depth 公式，但**只是 markdown 描述**——沒有對應 fixture workspace、沒有 scoring 程式、沒有 regression 鉤子。每次改 prompt 要驗證實際命中率，只能人工點 Explorer 跑一次再對。
- **D-006 後續清單**：完整評估 rubric「延後至打磨期」、「打磨期：接真 LLM snapshot replay」目前都還是 `[ ]`。要在 Demo 前攔到 prompt regression 必須把 scoring 從 markdown 公式變 Python code。

對齊 **D-006**（Golden sample 評估機制）、**D-004**（MVP 硬上限——只認 1 repo + 3 task，benchmark 本身要可重現）、`docs/agent-explorer-spec.md §十一 評估方式`、`docs/implementation-plan.md` **步驟 23**。

本 change 是**步驟 23 第一階段**：把「ideal-route 評估」從 markdown 公式落地成 Python 程式 + Synthetic Timeline fixture，把「全 stack replay」從現有 `_NoopCoverage` 升級成包含 Coverage 遞迴 + token probe + SSE emit 的端到端 pinned 場景。**真 LLM snapshot 留給後續 change 接**（D-006 的 `[ ] 打磨期` 那條）；本 change 仍走 scripted MockProvider，價值是把基礎設施準備好。

## What Changes

**A. Scoring helpers**（`sidecar/tests/golden/scoring.py`，**test-only utility**）：

- 新 `station_recall(produced_paths: set[str], must_have_paths: set[str]) -> float`：回 `|produced ∩ must_have| / |must_have|`；空 must_have raise `ValueError`（避免除以零）。
- 新 `station_noise(produced_paths: set[str], must_have: set[str], nice_to_have: set[str]) -> float`：分母為 `produced - must_have`，回扣掉 `nice_to_have` 後的雜訊比例 `|extras - nice_to_have| / |extras|`；`extras == ∅` 時回 `0.0`（沒雜訊就是沒）。
- 新 `composite_score(recall: float, noise: float, depth: float, weights: dict | None = None) -> float`：套 D-006 的 `0.5 * recall + 0.3 * (1 - noise) + 0.2 * depth` 公式；可注入自訂 weights 以利日後 tuning。
- 新 `IdealRoute` Pydantic schema（`tests/golden/`-shared）：`task: str` / `must_have: list[str]` / `nice_to_have: list[str]` / `noise_paths: list[str]`，附 `IdealRoute.model_validate_json` 載入 `ideal-route.json`。
- **本 change `depth` 暫回 `1.0` placeholder**（depth = `|resolved_dependencies| / |dep_chain|` 需 Module 5 的 station depends_on 真實填值；MVP 暫不裝，介面預留好讓打磨期改一行常數即可）。

**B. Timeline-style synthetic fixture**（`tests/golden/timeline-storage-adapter-synthetic/`）：

- 新 9 個 `.ts` 檔模擬 Timeline 專案的 Storage Adapter 拓撲（對齊 `tests/golden/timeline-gdrive-adapter/ideal-route.md` 的 5 站理想路線）：
  - 5 個 `must_have`：`app/types/index.ts` (interface)、`app/services/MockStorageAdapter.ts` (mock impl)、`app/services/LocalFileAdapter.ts` (real impl)、`app/composables/useStorage.ts` (init/composable)、`app/stores/timeline.ts` (consumer)
  - 2 個 `nice_to_have`：`app/stores/node.ts` / `app/stores/settings.ts`（其他 consumer）
  - 2 個 `noise`：`app/components/EventCard.vue`、`README.md`（路線不該進來的 UI / 文件）
- 每檔 ≤ 40 行（最小可識別 Storage Adapter 模式：interface 列 12 method、mock 用 in-memory map、real 含 fs handle stub、useStorage 寫個 ref/init pattern、store 用 `useStorage()` 拉資料）。
- 新 `tests/golden/timeline-storage-adapter-synthetic/ideal-route.json`：對齊 IdealRoute schema，鎖 5 個 must_have + 2 個 nice_to_have + 2 個 noise，`task` 欄位寫「在 Timeline 專案新增 Google Drive Adapter 同步功能」。
- 新 `tests/golden/timeline-storage-adapter-synthetic/README.md`：說明 fixture 結構 + 對應 `ideal-route.md` 的 5 站連結 + 使用方式（「未來 live LLM 跑這個會被打分」）。

**C. Full-stack scripted golden replay**（`sidecar/tests/golden/test_timeline_synthetic_replay.py`）：

- 新 replay 把整套 Module 4 stack wire 起來（與 `test_explorer_replay.py` 的 P0 形狀差異）：
  - 仍用 scripted `MockProvider` reasoning + judge（隔絕 LLM 不確定性）
  - **新加** scripted `MockProvider` coverage（push 一個空 `CoverageResult`，驗 coverage round 真有跑但不遞迴）
  - **新加** `LLMCoverageChecker(coverage_factory, workspace_root)` 取代 `_NoopCoverage`（與 production handler 形狀對齊）
  - **新加** `AggregatedTokenProbe([reasoning_provider, judge.provider, coverage.provider])` 餵給 `run_explorer`
  - **新加** `_SpyEmitter` 捕全部 SSE event 序列（含新 `coverage_gaps` / `usage_delta.session_total_tokens`）
- Scripted reasoning actions 設計成「依序 mark_station 命中 5 個 must_have 路徑」；judge verdict 全 `should_add_station=True` / `should_follow_imports=True`（後者讓 `pending_queue` 持續非空，避開 `_MIN_STATIONS_FOR_CONVERGENCE=3` 在 iter 4 觸發 `queue_empty` 提前停 — 詳見 `sidecar/src/codebus_agent/agent/explorer.py::_should_stop`）；budget_steps=5、budget_tokens=10_000；loop 在 iter 6 check 因 `budget_steps_left=0` 收斂為 `budget_exhausted`，恰好覆蓋 5 站。
- Assertions：
  - `station_recall(produced, must_have) == 1.0`、`station_noise(produced, must_have, nice_to_have) == 0.0`
  - `composite_score(...) >= 0.9`（含 depth 暫 placeholder 1.0 的 baseline）
  - 至少一筆 `coverage_gaps` SSE event 出現（`will_recurse=False`、`skip_reason="no_gaps"`）
  - 每筆 `usage_delta` 帶 `session_total_tokens >= 0`
  - 恰一筆 `budget_warning` event（`kind="steps"`、在 `consumed=4` / `step=4` 邊界發；`current=4` / `budget=5` / `pct=0.8`），0 筆 `kind="tokens"` —— production `_maybe_emit_budget_warning` 用 `>=` 比較，5 step 跑滿時 4/5=0.8 必發 1 次，這是 `context-compression-token-budget` archive 已 pin 的設計（見 `sidecar/tests/agent/test_budget_warning_event.py::test_first_iteration_crossing_step_threshold_emits_warning`）；token 端 budget=10_000 與 estimated tokens 拉開差距遠超 8_000，不會跨閾值。drift guard 鎖「恰 1 筆 kind=steps、0 筆 kind=tokens、發在 step 4 邊界」三條，任何 `_BUDGET_WARNING_PCT` / token estimator / prompt 累加變動都會打破
- Drift guard：fixture 改檔（新增 / 刪除 must_have） → recall 跌 → 測試紅。

**D. Scoring 單元測試**（`sidecar/tests/golden/test_scoring.py`）：

- `station_recall` 三 case：完美命中（1.0）、完全不命中（0.0）、部分命中（0.6）。
- `station_recall` 邊界：空 `must_have` raise `ValueError`。
- `station_noise` 三 case：純命中無雜訊（0.0）、命中 + nice_to_have 雜訊算扣（0.0，nice 不算雜訊）、命中 + 真噪音（0.5）。
- `station_noise` 邊界：`extras == ∅` 回 `0.0`（不是 raise，因為「沒雜訊」是合法輸出）。
- `composite_score` 兩 case：default weights 套 D-006 公式驗算、自訂 weights 加總正確。
- `IdealRoute.model_validate_json` 一 case：載入 fixture 的 `ideal-route.json` 成功且 4 欄完整。

**E. Demo-synthetic fixture 不動**（與本 change 邊界外）：

- 既有 `tests/golden/demo-synthetic/expected.json` 5 欄 baseline 不動 ——`test_explorer_replay.py` 的 7 測繼續綠（包含 prompt version drift guard）。
- 新加 fixture / 新加 replay 與 demo-synthetic 並行（不取代）。

**F. 文件同步**：

- `CLAUDE.md` archive 時間軸加入本 change；「下一步」改指向 **步驟 24 `Module 5 Generator P0`**。
- `docs/agent-explorer-spec.md §十一 評估方式` 改成「scoring helpers landed at `sidecar/tests/golden/scoring.py`，可重複用於未來 fixture」+ 連結到本 change archive。
- `docs/implementation-plan.md` 步驟 23 狀態 `⏳` → `✅ landed P0（golden-sample-baseline）；live LLM snapshot 待後續 change`。
- `docs/decisions.md` D-006 後續 checklist：把「建立 `tests/golden/` 放 ideal routes」標 `[x]`、把「scoring 公式落地」標 `[x]`，「打磨期：真 LLM snapshot」維持 `[ ]`。

## Non-Goals

- **真 LLM snapshot replay**（`@pytest.mark.live_llm` + skip-by-default）：D-006 後續清單上明列「打磨期」的事；本 change 落 scripted-only 基礎設施，留給後續 change（暫名 `golden-live-llm-snapshot`）接。理由：live LLM 跑會花錢、有 OpenAI API quota 風險、需要 secret management；先把 scoring + fixture 結構準備好，live snapshot 換工具來開花。
- **真 Timeline repo mirror**（`~/projects/timeline` 拉進 fixture）：MVP 對 D-004「1 repo + 3 task」上限的解讀是「benchmark 用合成 fixture，使用者真實 repo demo 時跑現場」。把真 repo 進 fixture 會引入 binary（PWA / UI assets）+ git submodule 治理問題；用 9 檔 mini synthetic 已足以驗 scoring 與 stations 拓撲。
- **Depth 完整評估**：D-006 公式 `depth = |resolved_dependencies| / |dep_chain|` 要 Station 的 `depends_on` 欄位真實填值；目前 Module 4 Explorer 的 `_update_state` 把 `depends_on=[]` hardcode（P0 簡化）。本 change 把 `depth` 暫回 `1.0` placeholder，等 Module 5 Generator 把 station depends_on 從教材 MOC 圖回填後再開新 change 實作 dep-chain 解析。
- **Recall/noise scoring 寫進 `expected.json`**：分數本身有浮動（depth placeholder 改 / weights tuning 都會動），不適合鎖死 baseline。本 change 在測試裡做 `>= 0.9` 門檻 assertion，分數本身不寫進 `expected.json`。
- **多語言 fixture**：`docs/decisions.md` D-006 提的「2 個 repo + 3 個任務」目前只實作 Timeline TS 風格一份，Python / Go fixture 留給打磨期。
- **改 demo-synthetic baseline schema**：既有 5 欄 baseline 是 `explorer-judge-golden` 的 P0 鎖點；本 change 不擴它（避免 re-baseline 連鎖）。新 fixture 用獨立 `ideal-route.json`，與 demo-synthetic 並行不互相干擾。
- **ScoringResult 寫進 reasoning_log.jsonl 或 SSE**：scoring 是測試時的後分析、不該污染 production audit chain（D-022 wire payload 純度原則）。

**拒絕的設計**：

- **「scoring 寫成 production module」**（`codebus_agent.scoring`）：scoring 永遠是離線 / test-time 的事，沒有 production code path 會在 runtime 算 recall。寫 production module 只會混淆架構分層；放 `sidecar/tests/golden/scoring.py` 是對的層次。
- **「ideal-route 用 markdown 而非 JSON」**：markdown 易讀，但 Python 載入要 parse 容易出錯；用 Pydantic JSON 強制 schema、編輯器有 lint。`ideal-route.md` 留作人類 reference 與設計文件，`ideal-route.json` 是機器讀的真相。
- **「scoring helpers 寫進現有 `test_explorer_replay.py`」**：兩件事責任分離——`test_explorer_replay.py` 是 demo-synthetic 的 5 欄 drift guard，scoring 是 Timeline-style fixture 的 recall 評估。寫一起會把測試檔變超大，測一個壞另一個分組亦不直覺。
- **「直接跑 timeline-gdrive-adapter/ideal-route.md 列的真實 Timeline repo」**：D-004 「1 repo + 3 task」上限不是 benchmark scope 上限——使用者跑 Demo 才指 1 repo；fixture 是另一回事。但合成 fixture 仍對齊 ideal-route.md 的拓撲，未來打磨期可以用 ideal-route.md 列出的真實檔案路徑跑 live LLM。

## Capabilities

### New Capabilities

（無 —— 所有改動掛在既有 `explorer-golden` capability 上）

### Modified Capabilities

- `explorer-golden`：
  - ADDED Requirement `Golden scoring helpers compute recall, noise, and composite score`（station_recall / station_noise / composite_score 三函式 + `IdealRoute` Pydantic schema 的合約 + 邊界條件）。
  - ADDED Requirement `Timeline-storage-adapter-synthetic fixture pins ideal-route stations`（fixture 檔案結構、`ideal-route.json` 欄位、9 檔分類規則 must_have / nice_to_have / noise）。
  - ADDED Requirement `Full-stack golden replay wires Coverage, token probe, and SSE emitter`（新 replay 引入 LLMCoverageChecker + AggregatedTokenProbe + spy emitter，pinned scenario 走完 5-station happy path 並驗 recall/noise/score 達門檻）。

## Impact

**受影響 spec**：

- `openspec/specs/explorer-golden/spec.md`（MODIFIED — 3 個 ADDED Requirement，既有 4 個 Requirement 不動）

**受影響 code（test infra only — production code 不動）**：

- `sidecar/tests/golden/scoring.py`（**新檔** — `station_recall` / `station_noise` / `composite_score` / `IdealRoute` schema）
- `sidecar/tests/golden/test_scoring.py`（**新檔** — scoring helpers 單元測試）
- `sidecar/tests/golden/test_timeline_synthetic_replay.py`（**新檔** — 全 stack scripted replay 對 Timeline 合成 fixture）
- `sidecar/tests/golden/__init__.py`（既有；無 import 變動）

**受影響 fixture（新檔）**：

- `tests/golden/timeline-storage-adapter-synthetic/README.md`
- `tests/golden/timeline-storage-adapter-synthetic/ideal-route.json`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/types/index.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/services/MockStorageAdapter.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/services/LocalFileAdapter.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/composables/useStorage.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/stores/timeline.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/stores/node.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/stores/settings.ts`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/app/components/EventCard.vue`
- `tests/golden/timeline-storage-adapter-synthetic/workspace/README.md`

**不受影響**：

- `tests/golden/demo-synthetic/`（既有 fixture）— 5 欄 baseline 不動。
- `tests/golden/timeline-gdrive-adapter/ideal-route.md`（既有 markdown）— 留作人類 reference，不刪不改；本 change 的 `ideal-route.json` 是機器讀版本。
- `sidecar/tests/golden/test_explorer_replay.py`（既有 7 測）— 全部繼續綠。
- `sidecar/src/codebus_agent/`（production code）— **零改動**。

**受影響文件**：

- `CLAUDE.md`（archive 時間軸 + 「下一步」導向步驟 24 Module 5）
- `docs/agent-explorer-spec.md §十一 評估方式`（landed 標註 + scoring 模組路徑連結）
- `docs/implementation-plan.md`（步驟 23 狀態更新為「P0 landed；live LLM 待後續」）
- `docs/decisions.md` D-006 後續 checklist（兩 `[x]`、保留 live LLM `[ ]`）

**無新依賴**（Pydantic 既有，無 `@pytest.mark.live_llm` 因為本 change 不接 live LLM）。

**無 breaking change**（純加新 fixture / 新測試 / 新 helpers；既有 demo-synthetic 與 production code 完全不動）。
