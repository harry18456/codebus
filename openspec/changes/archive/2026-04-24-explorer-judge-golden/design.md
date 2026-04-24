## Context

Module 4 Explorer 通電（`explorer-react-loop-p0` + `explorer-tools-p0` + `agent-sse-wiring` 三 change）後，loop / tools / SSE 三層骨幹完成；但 **Judge 仍是 placeholder prompt**（`sidecar/src/codebus_agent/agent/prompts/judge.py`），且**沒有任何 regression harness**。接下來 step 19 / 20 / 21 要碰 tools / coverage / budget，沒有 golden baseline 意味著任何行為退化都要靠 code review 肉眼抓 —— 這對「Demo 靈魂是 Explorer 找路」的專案極危險。

現行的 pinned 版本（`sidecar/src/codebus_agent/agent/prompts/__init__.py::JUDGE_PROMPT_VERSION`）已會寫進 `reasoning_log.jsonl` 每行（`docs/agent-core.md §十二`），但還沒有任何 test 真正把這欄位用做 load-bearing 鎖。本 change 把它變成**第一層 regression gate**：prompt 內容改 → bump 版本 → golden 失敗 → 實作者被迫手動 re-baseline。

## Goals / Non-Goals

**Goals:**

- Judge prompt 從「極簡 placeholder」升到能在真 LLM 上產出可用 `should_add_station` / `should_follow_imports` 訊號 —— 即**三段式指引**（角色邊界 / station 判準 / follow-imports 判準）+ `relevance` anchoring。
- 建立 golden sample harness 的**最小可行形狀**：一個 fixture（`tests/golden/demo-synthetic/`）、一份 `expected.json`、一個 pytest。
- 把 `JUDGE_PROMPT_VERSION` 變成 golden 的 load-bearing 鎖：prompt 改 → 版本 drift → test fail。
- 所有 Explorer loop / tools 既有測試（13 個 explorer_loop + 19 個 agent-sse + 其他）保持綠。

**Non-Goals:**

- 見 proposal Non-Goals 段（coverage-gap 遞迴 / trace_import / context 壓縮 / live-LLM golden / 多 fixture 矩陣 / Explorer prompt 調校 / 前端整合）。
- **不**做 Judge prompt 的 A/B 測試或 prompt 最佳化框架；本 change 只升一版、pin 一份 baseline。
- **不**做 golden fixture 自動生成工具；`expected.json` 由人手 pin。

## Decisions

### Golden fixture schema — 只鎖結構性斷言，不鎖內部細節

`tests/golden/demo-synthetic/expected.json` schema：

```json
{
  "stations": [
    { "path": "<relative-or-semantic-path>", "role": "<role-tag>" }
  ],
  "stopped_reason": "budget_exhausted" | "queue_empty" | "cancelled",
  "step_count": 3,
  "judge_prompt_version": "2026-04-25-1",
  "explorer_prompt_version": "<current-value-from-code>"
}
```

**station 比對規則**：pinned set 僅按 `(path, role)` pair 比對（set 相等）；**不**比對 `relevance` / `why` / `depends_on`（後三者對 Judge prompt 內容變化敏感、長期會噪）。

**replay 輸出**：harness 跑出來的 station list 照 `(path, role)` 取 set、和 expected.stations 做 set equality（不管順序）。`stopped_reason` / `step_count` 做 equality。prompt_version 欄位做 equality —— drift → test fail + 錯誤訊息包含 "re-baseline required"。

**為何不比整個 reasoning_log 行**：timestamp、Pydantic 內部 serialization 會飄；此外 tool_results 的 `output` 內容（KB 查 / grep 結果）對 fixture 變更敏感。只鎖結構等於「鎖行為、不鎖字串」。

**替代方案** — Deterministic JSON diff（整檔比對）：太脆、長期維護成本高，棄用。

### MockScript 形式 — inline Python fixture，不用 JSON 檔

golden harness 用 `MockProvider(script=MockScript())` 餵預定義的 `ExplorerAction` + `JudgeVerdict`。

**決策**：MockScript 直接寫在 `sidecar/tests/golden/test_explorer_replay.py` 裡（inline Python fixture），**不**另開 `tests/golden/demo-synthetic/mock-script.json` 的反序列化層。

**理由**：

- `ExplorerAction` / `JudgeVerdict` 是 Pydantic BaseModel，用 Python 寫最直接；JSON → Pydantic 要加反序列化邏輯，無必要複雜度。
- MockScript 內容本身就是 golden fixture 的一部分 —— 放進 test 檔讓讀者一眼看到「什麼輸入 → 什麼輸出」。
- 未來要多 fixture 再抽成 helper；P0 不預埋抽象。

**替代方案** — JSON / YAML 外檔：更「資料化」但多一層反序列化、減少可讀性，棄用（proposal Non-Goals 的「多 fixture 矩陣」延後時再評估）。

### Judge prompt 三段結構 — 寫死順序

`JUDGE_SYSTEM` 三段順序固定：

1. **角色邊界**：one-shot 評估 this iteration's `ToolResult`；**不**進 ReAct 子迴圈、**不**呼叫工具、**不**改 state。
2. **station 判準**：何時 `should_add_station=true`（新架構切片 / entrypoint / 協議邊界 × 與 task 明確相關）vs false（純 import 連鎖 / 雜訊 / 已 visited）。
3. **follow-imports 判準** + **relevance anchoring**：`[0.0, 1.0]` 五檔錨（0.0 / 0.3 / 0.5 / 0.8 / 1.0）。

`render_judge_prompt(task, results)` 額外塞：

- `state.visited_files` 前 20 條 + `... (N more)` 截斷（讓 Judge 知道哪些已走過、避免重 add_station）
- `state.stations` 計數 + 最近 3 條站 role/path 摘要（讓 Judge 知道目前收斂進度）
- 每個 `ToolResult`：`tool` + `args`（path / query whitelist）+ `output[:800]` 截斷（錯誤則塞 `error=<msg>`）

**為何 800 > 500**：Judge 比 Explorer 需要更多 tool output 脈絡做判斷；500 是 SSE wire 截斷目的不同（見 `agent-sse-wiring` 的 `agent_action_result.observation`），不混淆。

### `JUDGE_PROMPT_VERSION` 改 date-version 字串

現行可能是 `"v0"` 或 `"p0"`；本 change 改成 date-version：`"2026-04-25-1"`（日期 + 當日 revision）。

**理由**：與 `_RULES_VERSION = "2026-04-20-1"`（sanitizer）相同的慣例，方便日後 grep / diff；`-1` 後綴保留同日多版本空間。

### `EXPLORER_PROMPT_VERSION` 本 change 不動

Explorer prompt 的調校屬**獨立 concerns** —— 若 Explorer prompt 需升級（例如改工具選擇策略），另開 change。本 change 只動 Judge，golden 對 Explorer prompt 做 equality 比對但不 bump，確保「只改 Judge」的 scope 誠實。

### 測試層整合 — `sidecar/tests/golden/` 子目錄，pytest 自動收

新開 `sidecar/tests/golden/__init__.py` + `sidecar/tests/golden/test_explorer_replay.py`；pytest 按 `pyproject.toml` 既有 testpaths 自動收。**不**加 conftest，fixture 需要的 workspace 用 `tests/golden/demo-synthetic/workspace/` 的路徑字面值（pytest 啟動路徑是 `sidecar/`，所以 `../tests/golden/demo-synthetic/workspace/` 可 resolve）。

## Risks / Trade-offs

- [Judge prompt 寫太嚴 / 太鬆] → 經 golden harness 第一次驗證時難免調整；接受 prompt 可能在 MVP 期內再微調 1-2 次（每次 bump 版本 + 重 pin baseline）。規模控制在每次 < 30 min 手動 re-baseline。

- [scripted MockProvider vs 真 LLM 的 signal 差異] → MockScript 跑的是「Judge 的結構性行為」（每輪回 verdict → loop 收斂），不是「Judge prompt 的內容質量」。後者只能手動檢驗 prompt 文本 + 走一次 `scripts/smoke_chat_provider.py`-like 真 LLM smoke（非 CI 必跑）。**Mitigation**：propose 加一個 optional `@pytest.mark.live_llm` 標記 + skip-by-default，實作者可本機手跑驗 prompt 品質。

- [fixture path 解析脆弱] → `sidecar/` cwd 跑 pytest 時 `../tests/golden/...` 的相對路徑假設；CI / 其他環境若改 cwd 會壞。**Mitigation**：harness 用 `Path(__file__).parent.parent.parent.parent / "tests" / "golden" / ...` 絕對解析（從 test 檔本身往上找到 repo root），不依賴 cwd。

- [golden 成為 re-baseline chore 磁鐵] → 未來每改 Judge prompt 都得跑一次 harness、手動更新 `expected.json`；若 baseline 訊號模糊（例如 station set 變成 3 條 → 4 條都「看起來合理」），判斷成本上升。**Mitigation**：proposal 限定「只 pin 1 fixture」、結構性斷言粒度夠粗（只 `(path, role)` set）；未來若 re-baseline 頻率 > 每週 1 次，才重新評估 harness 形狀。

- [JUDGE_PROMPT_VERSION 漂移但 prompt 沒實質改] → 若有人手滑 bump 版本但沒動文字，harness 會炸；這是 feature，不是 bug —— 強迫 commit 要麼實質 bump、要麼 revert 版本字串。
