## Why

`explorer-react-loop-p0`（2026-04-24）+ `explorer-tools-p0`（2026-04-24）+ `agent-sse-wiring`（2026-04-24）之後，Module 4 Explorer ReAct loop、四個 P0 真工具、SSE 通電都已落地 —— **但 Judge 仍是極簡 default prompt**（`sidecar/src/codebus_agent/agent/prompts/judge.py::JUDGE_SYSTEM` 現在只有幾行 placeholder 文案），且**沒有任何 regression baseline** 擋住未來 change 把 Explorer 的輸出行為弄壞。對齊 `docs/implementation-plan.md` 步驟 18（Relevance Judge prompt 調 + reasoning_log 寫檔，後者已於步驟 16 落地）與步驟 23（Golden sample 首跑），兩者在時間線上強耦合：**golden fixture 凍的是「Judge prompt = X 版本下的 Explorer 輸出」** —— prompt 內容變了，baseline 就得重 pin；沒有 baseline，prompt 調到多好也沒法驗證有沒有 regress。所以本 change 把兩步合併，一次把 Judge prompt 從極簡升到能產出可用 station / follow-imports 訊號，並鎖住第一份 golden baseline（`tests/golden/demo-synthetic/`），往後 step 19 / 20 / 21 動 loop / tools / budget 都能靠 `pytest tests/golden/` 擋住 regression。

對齊 `docs/decisions.md` **D-006**（golden-sample based regression harness）、**D-012**（自寫 ReAct loop + Instructor/Pydantic structured output）、`docs/agent-core.md §十二`（JUDGE_PROMPT_VERSION 已寫進 reasoning_log 每行 — 本 change 把這個版本欄位變成 golden 的 load-bearing 鎖）、`docs/agent-explorer-spec.md`（Judge 責任邊界：relevance scoring + station decision + follow-imports decision，不進 ReAct 子迴圈）。

## What Changes

**Judge prompt 升級**（`sidecar/src/codebus_agent/agent/prompts/judge.py`）：

- `JUDGE_SYSTEM`：從現行極簡 default 重寫為**三段式指引** —— (a) Judge 在 Explorer loop 裡的角色（one-shot 評估這一輪的 `ToolResult`，不進 ReAct 子迴圈）、(b) `should_add_station` 判準（出現新的、與 task 明確相關的架構切片、entrypoint、協議邊界 → true；純 import 連鎖或雜訊 → false）、(c) `should_follow_imports` 判準（ToolResult 揭露了新的未探訪符號 / 檔案 → true；已 visited 或明顯不相關 → false）；`relevance` 在 `[0.0, 1.0]` 的 anchoring（0.0 無關、0.3 邊緣、0.5 相關、0.8 核心、1.0 entrypoint）。
- `render_judge_prompt(task, results)`：加入 `state.visited_files` 摘要（前 20 條 + `... (N more)` 截斷）、`state.stations` 計數 + 最近 3 條站點 role/path 摘要，讓 Judge 知道目前收斂狀態；`ToolResult` 摘要保留 tool name + args（path / query whitelist）+ `output[:800]` 截斷（比現行 500 寬一點，Judge 比 Explorer 需要更多脈絡），錯誤 ToolResult 改塞 `error=<msg>` 使 Judge 明確看到失敗。
- `JUDGE_PROMPT_VERSION` bump（在 `sidecar/src/codebus_agent/agent/prompts/__init__.py`）：新的 date-version 字串（例 `"2026-04-25-1"`）。
- `EXPLORER_PROMPT_VERSION` **不動**（Explorer prompt 本 change 不碰；避免 golden 同時鎖 Explorer + Judge 兩邊行為）。

**Golden sample harness + 首份 fixture**：

- 選 `tests/golden/demo-synthetic/` 當第一份 fixture（比 `timeline-gdrive-adapter/` 小、確定性高，適合 MVP pin）；若目錄尚無預期產出，本 change 一併補 `expected.json`（schema 見下）。
- 新檔 `sidecar/tests/golden/__init__.py` + `sidecar/tests/golden/test_explorer_replay.py`：
  - 讀 `tests/golden/demo-synthetic/workspace/` 當 workspace root。
  - 用 scripted MockProvider 餵一組預定義的 `ExplorerAction`（pin 在 `tests/golden/demo-synthetic/mock-script.json` 或 inline fixture），以及對應的 `JudgeVerdict` — 目的是**凍 Explorer loop 的機械行為 + Judge prompt 輸入格式**，不是跑真 LLM（真 LLM 對比留到 polish 期 live snapshot）。
  - `expected.json` schema：`{ "stations": [...] , "stopped_reason": "...", "step_count": N, "judge_prompt_version": "...", "explorer_prompt_version": "..." }`；station 比對只比 `(path, role)` pair set（不比 `relevance` / `why`，後兩者因 Judge prompt 重寫而敏感）。
  - 斷言：(a) stations set 完全匹配、(b) `stopped_reason` 匹配、(c) `step_count` 匹配、(d) `judge_prompt_version` 匹配 pinned 值（drift → test fail，強迫 re-baseline）、(e) reasoning_log 行數等於 `step_count`。
- fixture 固定值（以上 (d) 的 `judge_prompt_version` pinned）：golden 的 reasoning_log 由 harness 直接寫到 `tmp_path`，不落 repo；`expected.json` 是唯一 ground truth 檔。

**CLAUDE.md + 文件更新**：

- `CLAUDE.md` archive 時間軸加本 change。
- `docs/agent-core.md §十二` 補一行 note：「Judge prompt 重寫需 bump `JUDGE_PROMPT_VERSION` + 重建 golden baseline」。
- `docs/decisions.md` **D-006** 加「golden harness 落地形狀」段（scripted MockProvider / `expected.json` schema / 單 fixture MVP 規模）。

## Non-Goals

明確排除（留給後續 change 或打磨期）：

- **Coverage-gap 遞迴**：`_COVERAGE_RECURSION_ENABLED` 保持 `False`（step 20 `coverage-gap-recurse`）。
- **`trace_import` / `find_callers` P1 工具**：step 19 `explorer-tools-p1`。
- **Context 壓縮 + token-aware budget**：step 21。
- **Live-LLM golden**：本 change 只跑 scripted MockProvider；真 OpenAI snapshot replay 留打磨期（需要額外 snapshot 層 + cost guardrails）。
- **多 fixture 矩陣**：只 pin `demo-synthetic/`；`timeline-gdrive-adapter/` 的 golden 延後（預計 step 23 延伸或打磨期）。
- **Explorer prompt 調校**：`EXPLORER_PROMPT_VERSION` 與 `EXPLORER_SYSTEM` 不動；若 Explorer prompt 需升級，走獨立 change（同樣需重建 baseline）。
- **前端 Agent console 整合**：step 28 / 28.5 範疇；本 change 不動前端。
- **`should_recurse_coverage` 判準**：Judge 目前不回這個欄位（coverage recursion 還是 dormant），本 change 不新增此訊號。

**拒絕的設計**：

- **「跑真 LLM、拿輸出當 ground truth」**：非確定性 + cost + rate limit，CI 跑不穩定；scripted MockProvider 才是唯一適合 MVP 的 baseline 型態。
- **「比對整個 reasoning_log JSON 行完全相等」**：timestamp / 內部 Pydantic serialization 細節會飄；`expected.json` 只鎖**結構性斷言**（stations / stopped_reason / step_count）+ prompt_version。
- **「golden fixture 存真實 LLM response snapshot」**：未來要換 prompt、換 model、換 provider 都得重錄；scripted MockProvider 隔絕 LLM 層。

## Capabilities

### New Capabilities

- `explorer-golden`：Judge prompt 對「station / follow-imports / relevance anchoring」的內容契約，加上 golden sample 的 fixture schema、replay harness、prompt-version 鎖機制。支撐 `docs/decisions.md` D-006（golden-sample regression harness）與 `docs/agent-core.md §十二`（prompt_version 寫每行 reasoning_log）的交匯點。

### Modified Capabilities

（無 —— 既有 spec 的機制級 Requirement 保持不變；prompt 內容改動與 golden 契約都屬新 capability `explorer-golden`。）

## Impact

**受影響 spec**：

- `openspec/specs/explorer-golden/spec.md`（新建）

**受影響 code**：

- `sidecar/src/codebus_agent/agent/prompts/judge.py`（JUDGE_SYSTEM + render_judge_prompt 重寫）
- `sidecar/src/codebus_agent/agent/prompts/__init__.py`（`JUDGE_PROMPT_VERSION` bump；`EXPLORER_PROMPT_VERSION` 不動）
- `sidecar/tests/golden/__init__.py`（新檔）
- `sidecar/tests/golden/test_explorer_replay.py`（新檔 — pytest harness）
- `tests/golden/demo-synthetic/expected.json`（新或更新 — golden ground truth）
- `tests/golden/demo-synthetic/mock-script.json`（新 — scripted ExplorerAction / JudgeVerdict 序列，或以 inline fixture 取代）

**受影響測試**：

- 新：`sidecar/tests/golden/test_explorer_replay.py`
- 既有：`sidecar/tests/agent/test_judge.py` 需同步 regenerate expected verdict（prompt 重寫後既有 `test_judge_system_prompt_contains_*` 類測試若有硬編字串比對，需順勢更新）
- 其他 Explorer / tools / sse 測試不動（prompt 內容屬 LLMJudge 內部）

**受影響文件**：

- `CLAUDE.md`（archive 時間軸）
- `docs/agent-core.md §十二`（Judge prompt 重寫與 golden baseline 的連動原則）
- `docs/decisions.md` D-006（補 golden harness 落地形狀）

**無新依賴**（pytest / Instructor / MockProvider 都既有）。
