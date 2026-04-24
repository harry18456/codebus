## 1. Scaffolding

- [x] 1.1 建 test scaffolding：`sidecar/tests/agent/test_judge_prompt.py`（占位，測 JUDGE_SYSTEM + render_judge_prompt 內容契約）；`sidecar/tests/golden/__init__.py`（空）；`sidecar/tests/golden/test_explorer_replay.py`（占位）
- [x] 1.2 建 fixture scaffolding：`tests/golden/demo-synthetic/workspace/`（空資料夾或沿用 README 規劃的子結構放幾個 dummy .py 檔，確定 Scanner/Explorer 在此 workspace 能 resolve）；`tests/golden/demo-synthetic/expected.json` 放 `{}`（占位，內容在任務 5.2 填入）

## 2. RED — `Judge prompt produces station and follow-imports signals`

對應 spec `explorer-golden / Judge prompt produces station and follow-imports signals`。測試覆蓋 design 小節「Judge prompt 三段結構 — 寫死順序」。

- [x] 2.1 [P] `tests/agent/test_judge_prompt.py` 加 `test_judge_system_carries_role_bounds_section`（落實 spec Requirement `Judge prompt produces station and follow-imports signals`）—— 讀 `JUDGE_SYSTEM`，assert 字串含 "one-shot" / "不進 ReAct" / "不呼叫工具" / "不改 state" 語意關鍵字組（允許其中一種同義表達匹配）
- [x] 2.2 [P] `test_judge_prompt.py` 加 `test_judge_system_carries_station_decision_section` —— assert `JUDGE_SYSTEM` 至少含一條正向判準（"架構切片" / "entrypoint" / "協議邊界" 擇一）與一條負向判準（"純 import" / "已 visited" 擇一）
- [x] 2.3 [P] `test_judge_prompt.py` 加 `test_judge_system_carries_follow_imports_and_relevance_anchor` —— assert `JUDGE_SYSTEM` 含 `relevance` 五檔錨（`0.0` / `0.3` / `0.5` / `0.8` / `1.0` 數字全出現）
- [x] 2.4 [P] `test_judge_prompt.py` 加 `test_render_judge_prompt_includes_visited_and_stations` —— 建一個 `ExplorerState`（25 個 `visited_files`、4 個 `stations`），assert `render_judge_prompt(task, [])` 輸出含 "visited" 字樣 + 前 20 條 + "... (5 more)" 截斷 marker + stations count "4" + 最近 3 條 role/path
- [x] 2.5 [P] `test_judge_prompt.py` 加 `test_render_judge_prompt_truncates_tool_output_at_800_chars` —— 餵一個 `ToolResult.output = "x" * 10000`，assert 輸出該片段 length ≤ 810（含任何截斷 marker）；錯誤 `ToolResult`（error 非 None）assert 輸出含 `error=` 而非 `output=`

## 3. GREEN — 升級 Judge prompt

對應 design 小節「Judge prompt 三段結構 — 寫死順序」。

- [x] 3.1 `sidecar/src/codebus_agent/agent/prompts/judge.py` 改寫 `JUDGE_SYSTEM` 為三段式（角色邊界 / station 判準 / follow-imports + relevance anchoring），符合 2.1 ~ 2.3 斷言；保留 zh-TW 語氣、每段 ≤ 10 行
- [x] 3.2 `judge.py` 改寫 `render_judge_prompt(task, results)`：接 `state`（需改 signature 成 `render_judge_prompt(state: ExplorerState, results: list[ToolResult])` 並更新 `agent/judge.py::LLMJudge.evaluate` 呼叫點）；加入 visited_files 摘要（前 20 + `... (N more)`）、stations count + 最近 3 條、ToolResult output 截到 800 字（錯誤塞 `error=<msg>`）
- [x] 3.3 執行 `uv run pytest sidecar/tests/agent/test_judge_prompt.py sidecar/tests/agent/test_judge.py` 確認 2.1 ~ 2.5 全綠且既有 4 個 judge 測試不 regression

## 4. RED — `JUDGE_PROMPT_VERSION uses date-version format and bumps with content changes`

對應 spec `explorer-golden / JUDGE_PROMPT_VERSION uses date-version format and bumps with content changes`。測試覆蓋 design 小節「`JUDGE_PROMPT_VERSION` 改 date-version 字串」與「`EXPLORER_PROMPT_VERSION` 本 change 不動」。

- [x] 4.1 [P] `tests/agent/test_judge_prompt.py` 加 `test_judge_prompt_version_matches_date_format`（落實 spec Requirement `JUDGE_PROMPT_VERSION uses date-version format and bumps with content changes`）—— assert `JUDGE_PROMPT_VERSION` 匹配 regex `^\d{4}-\d{2}-\d{2}-\d+$`
- [x] 4.2 [P] `test_judge_prompt.py` 加 `test_explorer_prompt_version_unchanged_by_this_change` —— assert `EXPLORER_PROMPT_VERSION` 仍是這個 change 之前既有的值（硬編 pin；若既有值改動會 fail）

## 5. GREEN — JUDGE_PROMPT_VERSION bump + 建立 golden expected.json

- [x] 5.1 `sidecar/src/codebus_agent/agent/prompts/judge.py` 把 `JUDGE_PROMPT_VERSION` 改成 date-version 字串（例 `"2026-04-25-1"`），配合 2.x 的 prompt 改寫
- [x] 5.2 `tests/golden/demo-synthetic/expected.json` 填入 golden baseline JSON：`stations: [{path, role} × N]` / `stopped_reason` / `step_count` / `judge_prompt_version` / `explorer_prompt_version` 五欄；內容先以推算值寫入，走完 task 7 harness 後若實際跑出結果與 pinned 不同則回來更新（P0 允許此 iteration）
- [x] 5.3 執行 `uv run pytest sidecar/tests/agent/test_judge_prompt.py` 確認 4.1 ~ 4.2 全綠

## 6. RED — `Golden fixture pins expected stations, stopped_reason, step_count, and prompt versions`

對應 spec `explorer-golden / Golden fixture pins expected stations, stopped_reason, step_count, and prompt versions`。測試覆蓋 design 小節「Golden fixture schema — 只鎖結構性斷言，不鎖內部細節」。

- [x] 6.1 [P] `tests/golden/test_explorer_replay.py` 加 `test_expected_json_has_five_load_bearing_fields`（落實 spec Requirement `Golden fixture pins expected stations, stopped_reason, step_count, and prompt versions`）—— 讀 `tests/golden/demo-synthetic/expected.json`（用 `Path(__file__)`-based 解析），assert top-level keys 集合 == `{stations, stopped_reason, step_count, judge_prompt_version, explorer_prompt_version}`
- [x] 6.2 [P] `test_explorer_replay.py` 加 `test_expected_json_station_shape` —— assert `stations` 是 list，每個元素有 `path` (str) + `role` (str)
- [x] 6.3 [P] `test_explorer_replay.py` 加 `test_expected_json_stopped_reason_allowed_value` —— assert `stopped_reason` ∈ `{"budget_exhausted", "queue_empty", "cancelled"}`

## 7. GREEN — Golden replay harness 主體

對應 spec `explorer-golden / Golden replay harness runs under pytest and fails on drift`。落實 design 小節「MockScript 形式 — inline Python fixture」與「測試層整合 — `sidecar/tests/golden/` 子目錄」。

- [x] 7.1 `tests/golden/test_explorer_replay.py` 加 module-level helper `_golden_root() -> Path` —— 用 `Path(__file__).resolve().parents[3] / "tests" / "golden" / "demo-synthetic"` 絕對解析 fixture 根（實作前先 echo 路徑確認 parents 階數）
- [x] 7.2 `test_explorer_replay.py` 加 `test_golden_replay_matches_baseline`（落實 spec Requirement `Golden replay harness runs under pytest and fails on drift`，亦貫徹 design 決策「MockScript 形式 — inline Python fixture，不用 JSON 檔」與「測試層整合 — `sidecar/tests/golden/` 子目錄，pytest 自動收」）—— 建 `MockScript` 內含 `N` 個 `ExplorerAction`（inline Python fixture，值由實作者按 `_MIN_STATIONS_FOR_CONVERGENCE=3` 與 budget_steps 3 對齊）+ 對應的 `JudgeVerdict` 序列；呼 `run_explorer(...)`；產出與 `expected.json` 做比對：(a) stations `(path, role)` set equality、(b) `stopped_reason` equality、(c) `step_count` equality、(d) `reasoning_log.jsonl` 行數 == `step_count`
- [x] 7.3 執行 `uv run pytest sidecar/tests/golden/test_explorer_replay.py` 直到 7.2 綠；若 expected.json 與實際 run 不符，回頭微調 5.2 / 7.2 直到穩定

## 8. RED — `Golden replay harness runs under pytest and fails on drift`（drift scenarios）

對應 spec `explorer-golden / Golden replay harness runs under pytest and fails on drift` 的三個 drift scenario。

- [x] 8.1 [P] `test_explorer_replay.py` 加 `test_station_set_drift_fails_with_named_diff` —— 參考 7.2 但 MockScript 多產一個 `should_add_station=True` 的 JudgeVerdict 讓 station set 多出一條；assert `pytest.raises(AssertionError)` 且錯誤訊息含差異的 `(path, role)` 字串
- [x] 8.2 [P] `test_explorer_replay.py` 加 `test_prompt_version_drift_fails_with_rebaseline_hint` —— 用 `monkeypatch.setattr` 把 `JUDGE_PROMPT_VERSION` 改成 `expected.json` 以外的值；assert harness `pytest.raises(AssertionError, match="re-baseline")` 或等義提示
- [x] 8.3 [P] `test_explorer_replay.py` 加 `test_reasoning_log_line_count_mismatch_fails` —— 故意把 `budget_steps_left` 設成比 `expected.step_count` 大 1；assert `step_count` 比對失敗 → `pytest.raises(AssertionError)`

## 9. GREEN — 補 drift guards

- [x] 9.1 `test_explorer_replay.py::test_golden_replay_matches_baseline` 的斷言補 prompt_version drift guard：比對 pinned `judge_prompt_version` / `explorer_prompt_version` vs live `JUDGE_PROMPT_VERSION` / `EXPLORER_PROMPT_VERSION`，不等 → raise `AssertionError("re-baseline required: JUDGE_PROMPT_VERSION drifted from pinned baseline ...")`
- [x] 9.2 `test_explorer_replay.py` 的 station 比對錯誤訊息要 include 差異的 `(path, role)` 字串（符合 8.1 的期望）
- [x] 9.3 執行 `uv run pytest sidecar/tests/golden/test_explorer_replay.py` 確認 6.x + 7.x + 8.x 全綠

## 10. 文件 + repo metadata 更新

- [x] 10.1 `CLAUDE.md` archive 時間軸加入本 change；「下一步」改指向步驟 19 `explorer-tools-p1`（`trace_import` / `find_callers`）或步驟 20 `coverage-gap-recurse`
- [x] 10.2 `docs/agent-core.md §十二` 補一行 note：「Judge prompt 重寫需 bump `JUDGE_PROMPT_VERSION` + 重建 golden baseline」，並引用本 change spec
- [x] 10.3 `docs/decisions.md` D-006 加「golden harness 落地形狀」段落：scripted MockProvider / `expected.json` schema（5 欄）/ 單 fixture MVP 規模 / re-baseline 觸發機制

## 11. 驗證與 commit gate

- [x] 11.1 執行 `uv run pytest sidecar/tests/agent/test_judge_prompt.py sidecar/tests/agent/test_judge.py sidecar/tests/golden/test_explorer_replay.py` 確認新測試層全綠
- [x] 11.2 執行 `uv run pytest sidecar/tests/` 完整 suite 無 regression（既有 13 個 explorer_loop + 19 個 agent-sse 系列 + 4 個 judge + 其他照舊通過）
- [x] 11.3 執行 `pre-commit run --all-files` 全綠
