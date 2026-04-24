## Why

`explorer-react-loop-p0`（2026-04-24 archived）把 Explorer ReAct 骨架拼起來了 —— 六步迴圈 + Judge one-shot + ReasoningLogger + `ExplorerTools` / `Judge` / `CoverageChecker` 三個 Protocol —— 但 `ExplorerTools` 目前是**抽象 Protocol**，沒有任何可執行的具體實作。ReAct 迴圈能走、Judge 能判、reasoning_log 能寫，但 Agent **看不到程式碼**：沒有 tool 可以 grep 字串、讀檔、列目錄、標學習站。這意味著 `run_explorer` 跑起來永遠只會產生「thought=N/A、tool_calls=[]」的空步，既沒法驗 golden sample，也沒法接 Judge 的真實 relevance 訊號。

`docs/implementation-plan.md §第四階段 步驟 17` 指定本 change 落地 **Layer 1 四個 P0 tool**（`docs/agent-explorer-spec.md §三` 八個 tool 當中 P0 列的那四個：`search` / `list_dir` / `read_file` / `mark_station`），並把 `ToolContext` 擴到真能載 KB + UsageTracker，讓 Explorer Agent 跑真 codebase 不再是想像。為什麼 M1 就把 `ToolContext` + `ensure_in_workspace` + `tool_audit.jsonl` 立起來 —— 就是為了這一刻：本 change 把真工具疊上既有 sandbox 紅線，**零 retrofit**，每個 tool 第一天就被稽核。

對齊 `docs/decisions.md` D-012（自寫 ReAct + Instructor）、D-017（ToolSandbox + audit）、D-015（Sanitizer 三段）；不與任何既有 invariant 衝突。

## What Changes

**新增 `codebus_agent.agent.tools` 子套件**（`sidecar/src/codebus_agent/agent/tools/`）：

- **`folder_tools.py`** — `FolderTools` class 實作 P0 四個 tool method：
  - `search(keyword: str) -> list[SearchHit]` — 走 KB query（當 KB 已 build 完）或 grep fallback（KB 空時）；回傳 `SearchHit(path, snippet, score)` list。每 hit 的 path 相對 `workspace_root`。
  - `list_dir(path: str) -> list[DirEntry]` — 列出目錄下 entry（name / kind: file|dir / size），過 `ensure_in_workspace(path, ctx)` 紅線，被擋就 `raise PathEscapeError`。
  - `read_file(path: str, line_range: tuple[int, int] | None = None) -> Content` — 讀檔字串（可限行範圍），一樣過 `ensure_in_workspace`，另外過 `ctx.sanitizer` Pass 1（已載於 ToolContext）再回給 LLM —— 這是本 change 的稽核重心：**LLM 看到的檔案內容永遠是 sanitize 過的**。單檔讀太長（> 3000 tokens）截頭尾 + 中間 snippet，完整內容註記到 `state.visited_files`（Explorer update step 已做）。
  - `mark_station(path: str, role: str, why: str) -> None` — side-effect only，把 `Station(path, role, relevance=?, why, depends_on=[])` 追加到呼叫 context 的 stations。P0 版 relevance 先用常數 0.8（Judge 會二次判）；Agent 給 `why` 是為了前端顯示決策理由。**不動 KB，只動探索 state**。

- **`schemas.py`** — tool 層用的 Pydantic model：`SearchHit` / `DirEntry`（既有 `agent.protocols.Content` 已能用）。

- **`registry.py`**（可選；若規模小可不建）— 聚合 tool specs（名稱 / description / params schema）供 `_think` 組 prompt 用。P0 可用 module-level constant 取代。

**擴充 `codebus_agent.sandbox.ToolContext`**：加兩個 optional 欄（不 break 既有呼叫端）：
- `kb: KnowledgeBase | None = None` — KB client，`search` 走 vector query 時用
- `usage_tracker: UsageTracker | None = None` — 留給後續 tool 若要自呼 LLM（P0 tool 不呼）

**擴充 `codebus_agent.agent.explorer.run_explorer`**：
- 接 `tool_specs` 參數改為從 `FolderTools.tool_specs()` 取（目前是 `list[dict] | None`）
- `_execute_one` dispatch 仍走 `getattr(tools, call.name)`；tool 失敗仍包 `ToolResult.error` 不拋
- `run_explorer` 呼 tool 前後 append `ctx.audit_log` 一行（透過現有 `ToolSandbox.invoke` 走的話自動寫）—— 或顯式接 `tool_audit.jsonl` writer。**決定**：走現有 `ToolSandbox.invoke` 包裝，不另建 writer。

**受影響 spec（capability 異動）**：

- **新增 `explorer-tools` capability**：四個 P0 tool 的行為 + ToolContext 依賴 + sanitizer 對 `read_file` 輸出的強制套用 + `tool_audit.jsonl` 每呼寫一行。
- **修改 `tool-sandbox`**：ToolContext 新增 `kb` / `usage_tracker` 兩個 optional 欄位（additive，不破壞 M1 紅隊）；紅隊測試套件內的建構呼叫不需改。
- **修改 `agent-core`**：`run_explorer` 的 `tool_specs` 參數語意澄清（從 list of dict 改為 ExplorerTools 自曝的 spec，來自 `tools.tool_specs()`）；ExplorerTools Protocol 增加 optional method `tool_specs() -> list[dict]`（為 Prompt render 用）。

**受影響測試（`sidecar/tests/agent/tools/` 新目錄）**：

- `test_folder_tools.py` —— 四個 tool 的 happy path unit test（`MockKB` / temp workspace）
- `test_folder_tools_sandbox.py` —— 紅隊：`read_file` / `list_dir` 吃 `../..` / symlink / UNC / 長路徑前綴，必 `PathEscapeError`
- `test_folder_tools_sanitizer.py` —— `read_file` 讀含 secret / email 的 fixture，回傳必是 placeholder，`sanitize_audit.jsonl` Pass 1 行數對
- `test_folder_tools_audit.py` —— 每次 tool 呼叫（allow + deny）都在 `tool_audit.jsonl` 寫一行，`schema_version=1`，`args_summary` 只含 whitelist 欄
- `test_explorer_loop_with_real_tools.py` —— Explorer loop 跑 mini workspace（3-5 檔），驗 `ExplorerAction(tool_calls=[search+read_file+mark_station])` 的完整閉迴路：KB query → read_file → mark_station 後 `state.stations` 多一站
- 更新 `tests/agent/test_explorer_loop.py`：把 `_DummyTools` / `_EchoTools` / `_BoomTools` 留著做 loop 骨架單測，但加一個 real-ish integration 導引

**M2 承諾（本 change 明文）**：
- `search` 在 KB 未 build 完時**降級為 grep**，不 raise。降級策略走 `glob` 掃 `*.py|*.md|*.ts|*.rs`（延用 Scanner 的 text-file 判斷），上限 100 hits。
- `read_file` 的 sanitize 走 `ctx.sanitizer`（Pass 1），**不走 Pass 2**（Pass 2 是 LLM pre-flight，由 TrackedProvider 自動套）；若 `ctx.sanitizer is None`，read_file 回 raw 但寫 `sanitize_audit.jsonl` 一行 `{"skipped": true, "reason": "sanitizer_not_configured"}` 以留 trail（決定：**不降級靜默**）。
- `mark_station` 的 `relevance` 在 P0 hardcode 0.8；Judge 一致性 tune 延後到 `explorer-golden-sample-p0`（步驟 23）。

## Non-Goals

明確排除（避免 P0 蔓延；各自是未來獨立 change）：

- **`trace_import` / `find_callers`** —— 屬 P1（步驟 19 的 `explorer-tools-p1` change）。需要 AST / symbol index，工期非 trivial。
- **`add_to_queue` / `stop`** —— Explorer `_update_state` 已有 queue 增補邏輯，且 `ExplorerAction.stop` 欄位 Agent 可直接設；這兩個 tool 在 P0 不需要具象化。
- **Coverage Checker tool 版本** —— `coverage.check()` 是一支獨立 one-shot LLM call，不進 tool registry（`docs/agent-core.md §七`）。
- **Topic-mode tool（`web_search` / `fetch_page` / `evaluate_source`）** —— Phase 2 範疇（`docs/agent-explorer-spec.md §十二.2`）。本 change 只改動 Protocol shape 不破壞 Topic 未來兼容。
- **Generator (Module 5) 把 `stations[*].path` 轉 tutorial.md** —— Module 5 P0（步驟 24）。
- **SSE emit `agent_thought` / `action_result`** —— 步驟 22 `agent-sse-wiring`。本 change 的 tool 呼叫**不 emit SSE**（寫 `tool_audit.jsonl` 即可，前端靠 audit tail 也能近即時渲染）。
- **`POST /explore` HTTP endpoint** —— 沒進 `api/`。Explorer 入口只對 Python 層公開。
- **KB auto-build 自動偵測** —— `search` 只看 `ctx.kb is not None`；若 None 直接 grep。判斷 KB「ready」複雜度延到 Module 4 closing phase。
- **Workspace-level Budget token 紀錄到 `ExplorerState.budget_tokens_left`** —— `UsageTracker.session_total()` 讀取邏輯屬步驟 21 `explorer-budget-context`。本 change tool 不碰 budget。

**拒絕的設計**：

- **把 `mark_station` 做成一支 LLM call（像 Judge 那樣）** —— mark_station 是 Agent 意圖表達，不是 LLM 判斷；放一層 LLM 只是多花錢。
- **`search` 回 `list[str]`（純路徑）** —— 必須回 `SearchHit(path, snippet, score)` 保留 snippet；前端 LLM 能用 snippet 判要不要 read_file，省一輪。
- **`read_file` 不過 sanitizer，讓 TrackedProvider Pass 2 獨力擋** —— Pass 1 + Pass 2 兩道是刻意 D-015 設計；只靠 Pass 2 等於等到送 LLM 那刻才 redact，本地 `llm_calls.jsonl` 已落盤 raw，違反「LLM 看到的一定是 sanitize 過的」不變式。
- **Tool 失敗往外拋 exception** —— 違反 `docs/agent-core.md §九` 原則「不往上崩，最後至少給 partial result」；tool 錯誤包進 `ToolResult.error` 是 explorer-react-loop-p0 已落地的契約。

## Capabilities

### New Capabilities

- `explorer-tools`：Folder-mode Explorer 的四個 P0 具體工具（`search` / `list_dir` / `read_file` / `mark_station`）+ KB 整合 + Sanitizer Pass 1 對 `read_file` 輸出套用 + `tool_audit.jsonl` 每呼稽核（透過既有 ToolSandbox layer）。支撐 Module 4 Explorer 跑真 codebase（`docs/implementation-plan.md §第四階段 步驟 17`）。

### Modified Capabilities

- `tool-sandbox`：`ToolContext` 新增 optional `kb: KnowledgeBase | None` 與 `usage_tracker: UsageTracker | None` 欄位（additive）。既有紅隊測試 + ensure_in_workspace 契約不動。
- `agent-core`：`run_explorer` 的 `tool_specs` 來源改由 `tools.tool_specs()` 提供（非 caller 傳入）；`ExplorerTools` Protocol 補 optional `tool_specs()` method 供 Prompt render 用。

## Impact

**受影響 spec**：
- `openspec/specs/explorer-tools/spec.md`（新增 capability）
- `openspec/specs/tool-sandbox/spec.md`（modify：ToolContext 新增 optional 欄位）
- `openspec/specs/agent-core/spec.md`（modify：tool_specs 來源 + Protocol optional method）

**受影響 code**：
- `sidecar/src/codebus_agent/agent/tools/__init__.py`（新檔）
- `sidecar/src/codebus_agent/agent/tools/folder_tools.py`（新檔）
- `sidecar/src/codebus_agent/agent/tools/schemas.py`（新檔）
- `sidecar/src/codebus_agent/sandbox.py`（擴 ToolContext 欄位）
- `sidecar/src/codebus_agent/agent/explorer.py`（小改：`_think` 從 tools 取 tool_specs）
- `sidecar/src/codebus_agent/agent/protocols.py`（小改：Protocol 增 optional method）
- `sidecar/src/codebus_agent/agent/__init__.py`（re-export 新 tool class）

**受影響測試**：
- `sidecar/tests/agent/tools/__init__.py`（新檔）
- `sidecar/tests/agent/tools/conftest.py`（新檔，提供 `temp_workspace` / `mock_kb` fixture）
- `sidecar/tests/agent/tools/test_folder_tools.py`（新檔；happy path）
- `sidecar/tests/agent/tools/test_folder_tools_sandbox.py`（新檔；紅隊）
- `sidecar/tests/agent/tools/test_folder_tools_sanitizer.py`（新檔；Sanitizer Pass 1 套用驗證）
- `sidecar/tests/agent/tools/test_folder_tools_audit.py`（新檔；tool_audit.jsonl 稽核）
- `sidecar/tests/agent/test_explorer_loop_with_real_tools.py`（新檔；mini workspace integration）
- 既有 `sidecar/tests/agent/test_explorer_loop.py` 的 `_DummyTools` fixture 保留，不刪

**受影響文件**：
- `docs/agent-core.md`（§六 Tool 介面）—— 若 ToolContext 欄位差異需同步，回頭改
- `docs/agent-explorer-spec.md`（§三 工具表）—— 標 P0 四個 tool 為 implemented
- `docs/tool-sandbox.md`（§五 ToolContext）—— 新 optional 欄位納入
- `CLAUDE.md` §Repo 現況：archive 時間軸加入本 change；下一步指向步驟 18 `explorer-judge-golden`（或步驟 19 `explorer-tools-p1`）

**無新依賴 / 無新 env var**（`KnowledgeBase` / `UsageTracker` 已在 M2 / usage-tracker-dedup 落地）。
