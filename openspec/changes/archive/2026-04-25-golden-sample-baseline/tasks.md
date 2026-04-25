## 1. Scaffolding

- [x] 1.1 建立 fixture 骨架：`tests/golden/timeline-storage-adapter-synthetic/{README.md, ideal-route.json}` 兩個 placeholder 檔（README 寫一句指向 `ideal-route.md` 的 link、ideal-route.json 寫合法 `IdealRoute` JSON 的最小骨架以利 schema 載入）
- [x] 1.2 建立 fixture workspace 目錄結構：`tests/golden/timeline-storage-adapter-synthetic/workspace/{app/types,app/services,app/composables,app/stores,app/components,README.md}`（空目錄需 `.gitkeep` 或 `__init__`-style 檔，因為 git 不追空目錄；採 placeholder `.ts` 占位，內容稍後 task 4 填）
- [x] 1.3 建立 test scaffolding：`sidecar/tests/golden/scoring.py`、`sidecar/tests/golden/test_scoring.py`、`sidecar/tests/golden/test_timeline_synthetic_replay.py` 三個 placeholder 檔（`from __future__ import annotations` + module docstring 指向 spec requirement + 空 TODO 佔位）

## 2. RED — `Golden scoring helpers compute recall, noise, and composite score`

對應 ADDED spec requirement `Golden scoring helpers compute recall, noise, and composite score`（八 scenario：recall 三 case + recall 邊界 + noise 兩 case + noise 邊界 + composite 兩 case + IdealRoute round-trip）。覆蓋 design Decision 1：scoring helpers 放 test-only 模組路徑、Decision 2：`IdealRoute` schema 用 Pydantic JSON。

- [x] 2.1 [P] `test_scoring.py::test_station_recall_returns_one_on_perfect_hit` —— `station_recall({"a","b","c"}, {"a","b","c"}) == 1.0`
- [x] 2.2 [P] `test_scoring.py::test_station_recall_returns_zero_on_no_hit` —— `station_recall({"x","y"}, {"a","b","c"}) == 0.0`
- [x] 2.3 [P] `test_scoring.py::test_station_recall_returns_partial_fraction_on_partial_hit` —— `station_recall({"a","x"}, {"a","b","c"}) == 1.0/3.0`（用 `pytest.approx` 處理浮點）
- [x] 2.4 [P] `test_scoring.py::test_station_recall_raises_on_empty_must_have` —— `station_recall(set(), set())` 必 raise `ValueError`，message 含 `"must_have_paths cannot be empty"`
- [x] 2.5 [P] `test_scoring.py::test_station_noise_pure_hits_no_noise` —— `station_noise({"a"}, {"a"}, set()) == 0.0`
- [x] 2.6 [P] `test_scoring.py::test_station_noise_treats_nice_to_have_as_not_noise` —— `station_noise({"a","n"}, must_have={"a"}, nice_to_have={"n"}) == 0.0`（extras 全在 nice_to_have 內，雜訊扣為 0）
- [x] 2.7 [P] `test_scoring.py::test_station_noise_returns_half_on_real_noise` —— `station_noise({"a","n","x"}, {"a"}, {"n"}) == 0.5`（extras={"n","x"}，雜訊={"x"}，noise=1/2=0.5）
- [x] 2.8 [P] `test_scoring.py::test_station_noise_returns_zero_when_extras_empty` —— `station_noise({"a"}, {"a"}, set()) == 0.0` 不 raise（合法 clean output）
- [x] 2.9 [P] `test_scoring.py::test_composite_score_default_weights_match_d006_formula` —— `composite_score(1.0, 0.0, 1.0) == 0.5*1.0 + 0.3*1.0 + 0.2*1.0 == 1.0`
- [x] 2.10 [P] `test_scoring.py::test_composite_score_requires_all_three_weight_keys_when_overridden` —— `composite_score(0.8, 0.2, 0.5, weights={"recall":0.6})` 必 raise `KeyError`
- [x] 2.11 [P] `test_scoring.py::test_ideal_route_round_trips_through_json` —— 建 `IdealRoute(task="t", must_have=["a"], nice_to_have=["b"], noise_paths=["c"])`，dump 再 load，assert 四欄相等

## 3. GREEN — 實作 `sidecar/tests/golden/scoring.py`

對應 ADDED spec requirement `Golden scoring helpers compute recall, noise, and composite score` 與 design Decision 1 / Decision 2。

- [x] 3.1 `sidecar/tests/golden/scoring.py` 實作 `station_recall(produced, must_have) -> float`（依 spec 公式 `len(p & m) / len(m)`、空 must_have raise `ValueError("must_have_paths cannot be empty")`）
- [x] 3.2 `sidecar/tests/golden/scoring.py` 實作 `station_noise(produced, must_have, nice_to_have) -> float`（`extras = produced - must_have`、空 extras 回 `0.0`、否則 `len(extras - nice_to_have) / len(extras)`）
- [x] 3.3 `sidecar/tests/golden/scoring.py` 實作 `composite_score(recall, noise, depth, weights=None) -> float`（D-006 公式、default `{"recall":0.5,"noise":0.3,"depth":0.2}`、override 必三 key 否則 raise `KeyError`）
- [x] 3.4 `sidecar/tests/golden/scoring.py` 實作 `IdealRoute(BaseModel)`：四欄 `task: str` / `must_have: list[str]` / `nice_to_have: list[str]` / `noise_paths: list[str]`，無 default（強制 caller 填）
- [x] 3.5 執行 `uv run pytest sidecar/tests/golden/test_scoring.py` 直到 2.x 全綠

## 4. Fixture 內容填入 — 對應 `Timeline-storage-adapter-synthetic fixture pins ideal-route stations`

對應 ADDED spec requirement `Timeline-storage-adapter-synthetic fixture pins ideal-route stations`（四 scenario：must_have 5 / nice_to_have ≥2 + 不重 / noise ≥1 + 不重 / 沒 orphan）。

- [x] 4.1 [P] 寫 `tests/golden/timeline-storage-adapter-synthetic/workspace/app/types/index.ts`（≤ 40 行，含 `interface IStorageService { getTimeline(): Promise<...>; saveTimeline(...): Promise<void>; ... }` stub、列至少 5 個 method 名稱、不必 valid TS）
- [x] 4.2 [P] 寫 `workspace/app/services/MockStorageAdapter.ts`（≤ 40 行，`class MockStorageAdapter implements IStorageService` stub、in-memory map + `// stub` 占位）
- [x] 4.3 [P] 寫 `workspace/app/services/LocalFileAdapter.ts`（≤ 40 行，`class LocalFileAdapter implements IStorageService` stub、含 `// File handle init` / `// FileSystemObserver stub` 註解標誌）
- [x] 4.4 [P] 寫 `workspace/app/composables/useStorage.ts`（≤ 40 行，`export function useStorage()` stub、含 `$storage` / `$storageReady` / `$initStorage` 名稱）
- [x] 4.5 [P] 寫 `workspace/app/stores/timeline.ts`（≤ 40 行，`useTimelineStore` Pinia stub，呼 `useStorage()` consumer pattern）
- [x] 4.6 [P] 寫 `workspace/app/stores/node.ts`（≤ 40 行，nice_to_have secondary consumer）
- [x] 4.7 [P] 寫 `workspace/app/stores/settings.ts`（≤ 40 行，nice_to_have secondary consumer）
- [x] 4.8 [P] 寫 `workspace/app/components/EventCard.vue`（≤ 40 行，`<template><div></div></template>` UI noise 占位）
- [x] 4.9 [P] 寫 `workspace/README.md`（≤ 20 行，noise — 純 repo readme 文字）
- [x] 4.10 寫 `tests/golden/timeline-storage-adapter-synthetic/ideal-route.json`：`task` 欄寫「在 Timeline 專案新增 Google Drive Adapter 同步功能」、`must_have` 5 路徑（types/MockStorageAdapter/LocalFileAdapter/useStorage/timeline）、`nice_to_have` 2 路徑（node/settings）、`noise_paths` 2 路徑（EventCard.vue/README.md）；路徑用相對 `workspace/app/...` 形式
- [x] 4.11 寫 `tests/golden/timeline-storage-adapter-synthetic/README.md`：說明 fixture 結構、對應 `ideal-route.md` 的 5 站連結、警告「mini synthetic 不是真 timeline mirror」、未來 live LLM 用法

## 5. RED — Fixture 結構 schema 測試

對應 ADDED spec requirement `Timeline-storage-adapter-synthetic fixture pins ideal-route stations`（四 scenario）。

- [x] 5.1 [P] `test_timeline_synthetic_replay.py::test_fixture_provides_exactly_five_must_have_entries` —— 載入 `ideal-route.json`，assert `len(must_have) == 5`，全在 `workspace/app/` 之下
- [x] 5.2 [P] `test_timeline_synthetic_replay.py::test_fixture_nice_to_have_captures_secondary_consumers` —— assert `len(nice_to_have) >= 2` 且 `set(nice_to_have) & set(must_have) == set()`
- [x] 5.3 [P] `test_timeline_synthetic_replay.py::test_fixture_noise_paths_captures_off_route_files` —— assert `len(noise_paths) >= 1`、無與 must_have / nice_to_have 重疊
- [x] 5.4 [P] `test_timeline_synthetic_replay.py::test_all_workspace_files_appear_in_ideal_route_schema` —— `os.walk` workspace、相對化路徑後 assert 每檔在 `must_have ∪ nice_to_have ∪ noise_paths` 中、且唯一

## 6. RED — Full-stack scripted golden replay

對應 ADDED spec requirement `Full-stack golden replay wires Coverage, token probe, and SSE emitter`（六 scenario：recall 1.0 / noise 0.0 / composite ≥ 0.9 / coverage_gaps emit / 沒 budget_warning / usage_delta 帶 session_total_tokens）。覆蓋 design Decision 4：Coverage round 用 LLMCoverageChecker、Decision 5：AggregatedTokenProbe 全 stack wire、Decision 6：門檻不寫 expected.json、Decision 8：drift guard 形狀。

- [x] 6.1 [P] `test_timeline_synthetic_replay.py::test_replay_achieves_recall_one_on_synthetic_timeline_fixture` —— 跑全 stack replay、assert `station_recall(produced_paths, must_have) == 1.0`
- [x] 6.2 [P] `test_timeline_synthetic_replay.py::test_replay_reports_zero_noise_on_clean_run` —— 同 replay、assert `station_noise(produced, must_have, nice_to_have) == 0.0`
- [x] 6.3 [P] `test_timeline_synthetic_replay.py::test_composite_score_crosses_threshold` —— 同 replay、assert `composite_score(1.0, 0.0, 1.0) >= 0.9`（事實上 == 1.0）
- [x] 6.4 [P] `test_timeline_synthetic_replay.py::test_coverage_round_emits_coverage_gaps_event_under_spy_emitter` —— 同 replay、spy emitter 抓事件、assert 至少一筆 `type="coverage_gaps"` 且 `will_recurse=False`、`skip_reason="no_gaps"`
- [x] 6.5 [P] `test_timeline_synthetic_replay.py::test_five_step_run_emits_one_steps_warning_at_eighty_percent_boundary` —— 同 replay、assert 恰 1 筆 `type="budget_warning"` 且 `kind="steps"`/`current=4`/`budget=5`/`pct=0.8`，0 筆 `kind="tokens"`、`result.stopped_reason == "budget_exhausted"`（production `>=` 閾值 + budget_steps=5 必在 4/5=0.8 邊界發 1 次；token 端 10_000 budget 撐得住 5 iter 不跨 8_000）
- [x] 6.6 [P] `test_timeline_synthetic_replay.py::test_usage_delta_events_carry_session_total_tokens_additive_field` —— 同 replay、過濾 `usage_delta` events、assert 每筆都有 `session_total_tokens: int >= 0`

## 7. GREEN — 寫 full-stack replay

對應 design Decision 4 / Decision 5 / Decision 7：fixture 檔案內容極簡、Decision 8：drift guard 形狀。

- [x] 7.1 `test_timeline_synthetic_replay.py` 加 `_timeline_synthetic_root()` helper：`Path(__file__).resolve().parents[3] / "tests" / "golden" / "timeline-storage-adapter-synthetic"`（與 `_golden_root` 同模式但指向新 fixture）
- [x] 7.2 `test_timeline_synthetic_replay.py` 加 `_load_ideal_route()` helper：載入 `ideal-route.json` 回 `IdealRoute` 實例
- [x] 7.3 `test_timeline_synthetic_replay.py` 加 `_make_factory(role, default_module, script)` helper（與 `test_explorer_replay.py::_make_factory` 同模式 — inline 重新定義因 conftest scope per-directory）
- [x] 7.4 `test_timeline_synthetic_replay.py` 加 `_SpyEmitter` class（list-based 捕 emit；structurally 滿足 `SSEEmitter` Protocol）
- [x] 7.5 `test_timeline_synthetic_replay.py` 加 `_run_full_stack_replay(workspace_dir, must_have_paths)` helper：建 reasoning + judge + coverage 三個 MockScript（reasoning push 5 個 `ExplorerAction(thought, tool_calls=[ToolCall(name="echo", arguments={"path": p})])` for p in must_have；judge push 5 個 `JudgeVerdict(should_add_station=True, should_follow_imports=True)`（True 是因為 `_MIN_STATIONS_FOR_CONVERGENCE=3` 在 queue 空 + stations≥3 時觸發 `queue_empty` 提前停；True 讓 queue 持續非空，loop 自然 budget_exhausted 收斂）；coverage push 1 個 `CoverageResult(gaps=[])`）；建三個 TrackedProvider；建 `LLMCoverageChecker`；建 `AggregatedTokenProbe`；建 `_SpyEmitter`；wire 所有 `set_emitter`；跑 `run_explorer(..., token_probe=probe, emitter=spy)`、回 `(result, spy_events)`
- [x] 7.6 執行 `uv run pytest sidecar/tests/golden/test_timeline_synthetic_replay.py` 直到 5.x 與 6.x 全綠

## 8. 文件與 metadata 更新

- [x] 8.1 `CLAUDE.md` archive 時間軸加入本 change；「下一步」改指向 **步驟 24 `Module 5 Generator P0`**
- [x] 8.2 `docs/agent-explorer-spec.md §十一 評估方式` 改成「scoring helpers landed at `sidecar/tests/golden/scoring.py`，可重複用於未來 fixture」+ 連結到 `tests/golden/timeline-storage-adapter-synthetic/`；live LLM snapshot 段標「待打磨期」
- [x] 8.3 `docs/implementation-plan.md` 步驟 23 狀態 `⏳` → `✅ landed P0（golden-sample-baseline）；live LLM snapshot 待後續 change`
- [x] 8.4 `docs/decisions.md` D-006 後續 checklist：`[x] 建立 tests/golden/`（已 demo-synthetic + 新 timeline-synthetic）、`[x] scoring 公式落地`（station_recall / station_noise / composite_score 在 `sidecar/tests/golden/scoring.py`）；保留 `[ ] 打磨期：真 LLM snapshot`

## 9. 驗證與 commit gate

- [x] 9.1 執行 `uv run pytest sidecar/tests/golden/` 全綠（既有 `test_explorer_replay.py` 7 測 + 新 `test_scoring.py` 11 測 + 新 `test_timeline_synthetic_replay.py` 10 測）—— 28 passed
- [x] 9.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression（demo-synthetic baseline 不動 → 既有 7 個 explorer-replay 測仍綠；agent / api / providers / scanner / kb 全部不受影響因 production code 零改動）—— 698 passed / 19 skipped (symlink + qdrant + packaged binary 環境相依)
- [x] 9.3 執行 `pre-commit run --all-files` 全綠（新 fixture 檔通過 trailing-whitespace / EOF / line-ending）—— stage-0 hook 全 Passed
