# Agent Core — 自寫 ReAct Loop 實作 Spec

> Explorer / Judge / Coverage Checker 的**實作層**設計。
> 這份講「怎麼寫」；`agent-explorer-spec.md` 講「做什麼 / 為什麼」；`llm-provider.md` 講「LLM 呼叫抽象」。
> 關聯決策：**D-012（自寫 Agent Core + Instructor/Pydantic 輔助）**。

---

## 一、範圍與邊界

### 在這份 spec 內
- Python 資料結構（Message / ToolCall / Step / State）
- 主 ReAct 迴圈 pseudo-code
- 三層 Agent 組合模式（Explorer 呼叫 Judge / Coverage Checker）
- Tool 介面、註冊、執行器
- Prompt 模板管理
- 錯誤處理、重試、context 壓縮
- Budget 控制
- reasoning_log 寫入與 SSE 串流介面

### 不在這份 spec 內
- Agent 做什麼決策、何時收斂 → `agent-explorer-spec.md`
- 怎麼呼叫 LLM、PII 去識別化 → `llm-provider.md`
- 工具內部實作（grep / ast parse 細節）→ `modules/` 各 Module 自己的 spec
- HTTP endpoint 契約 → `sidecar-api.md`

---

## 二、選型結論（D-012）

| 項目 | 選擇 | 理由 |
|---|---|---|
| ReAct loop | **自寫** | 核心學習點；200-300 行；Demo 時可講每行決策 |
| Judge / Coverage Checker | **自寫**（one-shot call） | 一次 LLM 呼叫即可，不需框架 |
| Structured output | **Instructor + Pydantic** | Schema 驗證、重試、parsing 不值得自己刻 |
| LLM HTTP | **openai / anthropic SDK** | 原生 SDK，不是框架 |
| Prompt 模板 | Python f-string / `textwrap` | 直接看得到 prompt；複雜時再引 Jinja2 |
| Tool 註冊 | 自寫 decorator + registry | 簡單可控 |

**不用 LangChain / LangGraph / LlamaIndex**：抽象層太厚，debug 時找不到問題；學不到 Agent 原理；Demo 說「wrap LangChain」沒故事。

---

## 三、資料結構（`agent/types.py`）

```python
from pydantic import BaseModel, Field
from typing import Literal, Any
from datetime import datetime

class Message(BaseModel):
    role: Literal["system", "user", "assistant", "tool"]
    content: str
    tool_call_id: str | None = None
    tool_name: str | None = None

class ToolCall(BaseModel):
    id: str
    name: str
    arguments: dict[str, Any]

class ToolResult(BaseModel):
    tool_call_id: str
    tool_name: str
    output: str           # 給 LLM 看的字串化結果
    raw: Any = None       # 程式側用的原始結構
    error: str | None = None

class Step(BaseModel):
    """ReAct 迴圈的一輪（寫進 reasoning_log.jsonl 的單位）"""
    step: int
    ts: datetime
    thought: str
    tool_calls: list[ToolCall]
    tool_results: list[ToolResult]
    judge_verdict: "JudgeVerdict | None" = None
    tokens_used: int

class JudgeVerdict(BaseModel):
    """Relevance Judge 的輸出（Instructor 會驗這個 schema）"""
    relevance: float = Field(ge=0, le=1)
    should_follow_imports: bool
    should_add_station: bool
    reason: str

class ExplorerState(BaseModel):
    """Agent 整個 session 的狀態"""
    task: str
    messages: list[Message]
    visited_files: set[str] = set()
    pending_queue: list[str] = []
    stations: list["Station"] = []
    budget_steps_left: int
    budget_tokens_left: int
    step_count: int = 0
```

**原則**：所有 LLM 互動對象都是 Pydantic model；Instructor 直接拿 schema 驗證 LLM output。

---

## 四、主 ReAct 迴圈

```python
# agent/explorer.py — 本節 code 為 post-P0 目標形狀；目前 explorer-react-loop-p0 已落地
# Think→Act→Observe→Judge→Log→Update 六步骨架 + cancel_event + dormant coverage hook。
async def run_explorer(
    *,
    state: ExplorerState,
    provider: TrackedProvider,       # TrackedProvider-only（registry guard 擋原生 provider）
    tools: ExplorerTools,            # agent/protocols.py @runtime_checkable Protocol
    judge: Judge,
    coverage: CoverageChecker,
    logger: ReasoningLogger,
    cancel_event: asyncio.Event | None = None,
    tool_specs: list[dict] | None = None,
) -> ExplorerResult:
    while True:
        stop, reason = _should_stop(state, cancel_event)
        if stop:
            break

        # 1. Think — LLM 決定下一步
        thought, tool_calls = await _think(state, provider, tool_specs or [])

        # 2. Act — 執行工具（可並行；tool 錯誤包進 ToolResult.error 不往外拋）
        results = await _execute_tools(tool_calls, tools)

        # 3. Observe — 結果寫回 messages，讓下輪 LLM 看到
        _append_observations(state, tool_calls, results)

        # 4. Judge — 針對這輪看到的新內容評估
        verdict = await judge.evaluate(state, results)

        # 5. Log — 寫 reasoning_log（P0 sync；SSE emit 由 agent-sse-wiring change 注入）
        logger.write(Step(
            step=state.step_count,
            ts=datetime.now(timezone.utc),
            thought=thought,
            tool_calls=tool_calls,
            tool_results=results,
            judge_verdict=verdict,
            tokens_used=0,
        ))

        # 6. Update state — 更新 queue / visited / stations
        _update_state(state, tool_calls, results, verdict)
        state.step_count += 1
        state.budget_steps_left -= 1

    # 7. Coverage check + 遞迴補查 — P0 以 _COVERAGE_RECURSION_ENABLED=False 夾住,
    # 由 coverage-gap-recurse change 打開並補上遞迴深度上限。
    if _COVERAGE_RECURSION_ENABLED:
        gaps = await coverage.check(state)
        if gaps and state.budget_steps_left > 0:
            _enqueue_gap_investigation(state, gaps)
            return await run_explorer(...)  # 遞迴

    return ExplorerResult(
        stations=list(state.stations),
        log_path=str(logger.path),
        stopped_reason=reason or "budget_exhausted",
    )


def _should_stop(
    state: ExplorerState, cancel_event: asyncio.Event | None
) -> tuple[bool, str | None]:
    # 三分支 convergence：cancel → budget → queue_empty(with enough stations)
    if cancel_event is not None and cancel_event.is_set():
        return True, "cancelled"
    if state.budget_steps_left <= 0:
        return True, "budget_exhausted"
    if not state.pending_queue and len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE:
        return True, "queue_empty"
    return False, None
```

**關鍵特性**
- 一個 while 迴圈，所有決策透明，debug 時 stack trace 乾淨
- Think / Act / Observe / Judge / Log / Update 分函式，每段獨立可測
- Cancel 走 `asyncio.Event`，每輪迴圈開頭檢查一次
- Recursive call 用在 Coverage gap 補查，容易讀但要設遞迴深度上限（預設 3）；P0 以 flag 夾住不跑
- `_MIN_STATIONS_FOR_CONVERGENCE = 3` 是 P0 收斂下限常數（spec `Explorer loop stops...` 的 sensible P0 default）

---

## 五、Think 的實作（Instructor 結合處）

```python
class ExplorerAction(BaseModel):
    """Agent 一輪的輸出 schema，Instructor 驗證"""
    thought: str = Field(description="決策理由，要具體提到看到了什麼")
    tool_calls: list[ToolCall] = Field(default_factory=list)
    stop: bool = False  # Agent 主動宣告收斂

async def _think(state, registry, tools) -> tuple[str, list[ToolCall]]:
    prompt = render_explorer_prompt(state, tools.specs())

    # Explorer 屬 REASONING role（llm-role-routing，2026-04-20 落地）
    provider = registry.get(ProviderRole.REASONING)

    # Instructor 自動驗證 + retry
    action: ExplorerAction = await provider.chat_structured(
        messages=state.messages + [Message(role="user", content=prompt)],
        response_model=ExplorerAction,
        max_retries=2,
    )
    return action.thought, action.tool_calls
```

**Role 分派**：`run_explorer` 從 `ProviderRegistry` 取 `ProviderRole.REASONING`（Opus 等級）。Judge / Coverage 走 `ProviderRole.JUDGE`（Haiku 等級，見 §七）；四個 role 的 routing 與 config 見 `llm-provider.md` §二 / §五。

**為什麼 Instructor 值得引**
- Schema 驗不過自動重試（帶錯誤訊息再問一次）
- Pydantic 型別保證，不用手刻 JSON parse
- 程式碼讀起來就是「LLM 回 `ExplorerAction`」很直覺
- Instructor 只有一層薄封裝，不影響「Agent 邏輯自寫」的故事

**Provider 層補 `chat_structured`**（`llm-provider.md` §二 擴充）：
```python
async def chat_structured(
    self,
    messages: list[Message],
    response_model: type[BaseModel],
    max_retries: int = 2,
) -> BaseModel: ...
```
MVP 實作：Contest provider 內部呼叫 `instructor.from_openai(client)`。

---

## 六、Tool 介面

### 註冊

```python
# agent/tools/__init__.py
_registry: dict[str, Tool] = {}

def tool(name: str, description: str, schema: type[BaseModel]):
    def decorator(fn):
        _registry[name] = Tool(name=name, description=description, schema=schema, fn=fn)
        return fn
    return decorator

@tool(
    name="read_file",
    description="讀取檔案內容。回傳前 N 行或指定行範圍",
    schema=ReadFileArgs,
)
async def read_file(args: ReadFileArgs, ctx: ToolContext) -> str:
    # 實作
    ...
```

### Tool spec 餵給 LLM
```python
def specs() -> list[ToolSpec]:
    return [
        ToolSpec(
            name=t.name,
            description=t.description,
            parameters=t.schema.model_json_schema(),
        )
        for t in _registry.values()
    ]
```

### ToolContext — 跨工具共享狀態

**唯一真相：`tool-sandbox.md` §五**。所有欄位（`workspace_root` / `workspace_id` / `session_id` / `kb` / `kb_growth_log` / `sanitizer` / `audit_log` / `usage_tracker` / `allow_git_metadata`）與 `frozen=True` 約束都定義在那裡；本文件不複寫，避免 drift。

**Orchestrator 側的分工**：
- `ToolContext` = tool 執行期共享的 sandbox 基元（不可變、per-session 建立一次）
- `ExplorerState.visited_files` / `pending_queue` / `stations` / budget = orchestrator 的 session state（每輪更新），**不放進 ToolContext**
- 建 ctx：`ctx = ToolContext(workspace_root=..., workspace_id=..., session_id=..., kb=..., ...)`，之後 `_execute_tools(calls, tools, ctx)` 傳同一份

**路徑驗證（必做）**：任何 tool 接 path 參數必須先走 `ensure_in_workspace(args.path, ctx)`，詳見 `tool-sandbox.md` §三／§五。

### 執行器（可並行）
```python
async def _execute_tools(calls, tools, state) -> list[ToolResult]:
    ctx = ToolContext(...)
    tasks = [_execute_one(c, tools, ctx) for c in calls]
    return await asyncio.gather(*tasks, return_exceptions=False)

async def _execute_one(call, tools, ctx) -> ToolResult:
    try:
        tool = tools.get(call.name)
        args = tool.schema.model_validate(call.arguments)
        output = await tool.fn(args, ctx)
        return ToolResult(tool_call_id=call.id, tool_name=call.name, output=output)
    except Exception as e:
        return ToolResult(..., error=str(e), output=f"ERROR: {e}")
```

**錯誤不拋**，包成 ToolResult.error 字串回給 LLM，讓 Agent 自己處理「這個工具失敗」的決策。上面是骨架；正式版要額外攔 `PathEscapeError` 並寫 audit（完整實作見 `tool-sandbox.md` §五）。

---

## 七、Judge 與 Coverage Checker

**都是 one-shot LLM call，不進 ReAct 迴圈，各自獨立函式。**

**P0 落地形狀（`explorer-react-loop-p0`）**：`LLMJudge(provider_factory, workspace_root)` 在建構期呼一次 factory（shape 與 `app.state.llm_judge_provider(ws)` 一致）拿到 **workspace-scoped TrackedProvider**，之後 evaluate 只重用該 provider；這和 chat-provider-wiring 定的 per-workspace 稽核落地點對齊（`token_usage.jsonl` / `llm_calls.jsonl` 依 workspace 自動分流）。registry-based 寫法（下方 code block）是往後若要合併多 provider 的參考形狀，P0 未實作。`CoverageChecker` 在 P0 只有 Protocol 抽象，LLM impl 延到 `coverage-gap-recurse` change。

```python
class Judge:
    def __init__(self, registry: ProviderRegistry):
        self._registry = registry

    async def evaluate(
        self,
        state: ExplorerState,
        results: list[ToolResult],
    ) -> JudgeVerdict:
        prompt = render_judge_prompt(state.task, results)
        # Judge / Coverage 屬 JUDGE role（Haiku 等級，低溫）
        provider = self._registry.get(ProviderRole.JUDGE)
        return await provider.chat_structured(
            messages=[Message(role="user", content=prompt)],
            response_model=JudgeVerdict,
        )


class CoverageChecker:
    def __init__(self, registry: ProviderRegistry):
        self._registry = registry

    async def check(self, state: ExplorerState) -> list[Gap]:
        prompt = render_coverage_prompt(state.task, state.stations)
        provider = self._registry.get(ProviderRole.JUDGE)
        result: CoverageResult = await provider.chat_structured(
            messages=[Message(role="user", content=prompt)],
            response_model=CoverageResult,
        )
        return result.gaps
```

**為什麼不在 Explorer 迴圈內跑**：Judge 每步都跑，但它自己不是 ReAct；Coverage 只在收斂時跑一次。這樣邏輯分層乾淨，也方便 Phase 2 Topic mode 換同名 component。

---

## 八、Prompt 管理

### MVP 策略：Python module + f-string

```
agent/
  prompts/
    __init__.py
    explorer.py      # render_explorer_prompt()
    judge.py         # render_judge_prompt()
    coverage.py
    generator.py     # Module 5 用
```

**每個 prompt 一個 render function**，輸入是資料、輸出是 str：

```python
# prompts/explorer.py
EXPLORER_SYSTEM = """你是探索 codebase 的 Agent..."""

def render_explorer_prompt(state: ExplorerState, tool_specs: list) -> str:
    return textwrap.dedent(f"""
    任務：{state.task}
    已訪問檔案數：{len(state.visited_files)}
    目前路線站數：{len(state.stations)}
    ...
    可用工具：{_format_tools(tool_specs)}
    """)
```

### 版本化
- Prompt 本身是 Python 檔 = 走 git，改動進 PR 有 diff 可 review
- 重大改動打 constant `EXPLORER_PROMPT_VERSION = "v3"`，寫進 reasoning_log 讓我們之後能對齊「這批 golden sample 是 v3 prompt 跑的」

### Phase 2 才考慮
- 搬 Jinja2（當邏輯複雜到 f-string 難讀）
- Prompt hub（LangSmith / Humanloop 之類）—— 目前沒必要

---

## 九、錯誤處理策略

| 錯誤 | 處理 |
|---|---|
| Tool 執行錯 | 包進 `ToolResult.error`，回給 LLM 讓它自己換招 |
| LLM schema 驗證錯 | Instructor 自動重試 2 次；仍錯 → log + 回傳 fallback（空 ExplorerAction） |
| LLM rate limit (429) | Provider 層 exponential backoff 3 次；仍錯 → 往上丟，sidecar 回 SSE error event |
| Context overflow | 觸發 §十 壓縮策略；仍超過 → stop 並回 partial result |
| Cancel signal | `asyncio.Event` 每輪迴圈開頭檢查，cancel 時收斂成 partial 回傳 |
| Coverage 遞迴過深 | 上限 3 層，超過停止並記 log warning |

**原則**：不往上崩，最後至少給 partial result（已產出的 stations），讓 Module 5 可以試著產教材。

---

## 十、Context 壓縮

LLM context window 有限（Claude 200k、GPT-4 128k），長探索會超過。

**策略**
1. **`messages` rolling window**：保留 system + task + 最近 N 步（預設 8），超過的舊 tool results 摘要成「已訪問 X, Y, Z」塞進 system
2. **Tool result 截斷**：單檔讀太長（>3000 tokens）只給頭尾 + 中間 snippet，完整內容存 state.visited_files 供後續參照
3. **State snapshot 注入**：每輪前把 `len(visited_files)`、`len(stations)`、`pending_queue[:5]` 放進 system prompt，讓 LLM 不用翻 history 也知道進度

壓縮觸發點：token count 到 context window 的 75% 時。

---

## 十一、Budget 控制

```python
@dataclass
class Budget:
    max_steps: int = 40
    max_tokens: int = 200_000
    max_wall_seconds: int = 600     # 10 分鐘硬上限
```

- 每步遞減 `budget_steps_left`
- **Tokens 用量從 `UsageTracker.session_total()` 即時讀取**（D-021，不再 Explorer 內部估算）
- 到 80% 時 prompt 加提示「budget 快用完，開始收斂」
- 100% 強制 stop

**來自 D-007**：cost benchmark 做完再調預設值。MVP 先設保守值。

---

## 十二、reasoning_log 與 SSE

### 寫檔
每步 append JSONL 到 `{workspace}/reasoning_log.jsonl`：

**P0 落地形狀（`explorer-react-loop-p0`）**：`ReasoningLogger(path)` 純 sync 寫檔、只負責落地；`write(step)` 呼 `step.model_copy(update={"explorer_prompt_version": EXPLORER_PROMPT_VERSION, "judge_prompt_version": JUDGE_PROMPT_VERSION})` 後 append `model_dump_json() + "\n"`，寫失敗直接 raise（不靜默丟失）。**SSE emit 不在 `ReasoningLogger` 內**——檔案寫入與 wire 廣播是兩條獨立責任，由獨立的 `SSEEmitter` 注入處理（落地於 `agent-sse-wiring`）。

### SSE emit（`agent-sse-wiring` 落地形狀）
`codebus_agent.agent.emitter` 提供 `@runtime_checkable SSEEmitter` Protocol（單方法 `emit(event: dict) -> None`）+ 兩個具體 impl：`NullEmitter`（no-op，供 in-process 測試 / golden replay）與 `TaskHandleEmitter(handle)`（fan-out 到 subscriber queue，走既有 `sse-progress-skeleton` 機制）。

三條 emit 軌道並行，責任分開：

- **Explorer loop** — `run_explorer(..., emitter=None)`（`None` default 對既有 in-process 測試相容）在每輪 Think → Act → Judge 之後 emit `agent_thought` / 每個 ToolResult 對應 `agent_action_result`（`observation` 截到 500 字）/ `judge_verdict` / 每輪尾端一筆 `progress`（`total` 在迴圈前 snapshot）。
- **TrackedProvider** — `__init__(..., emitter=None)` + `set_emitter(emitter)`（endpoint 建完 per-task handle 後才晚綁）；成功 `chat` / `embed` 在 `tracker.record` 之後 emit `usage_delta`（失敗 path 不 emit，`session_total_cost_usd` 本地累加）。`phase` / `step` 從 `codebus_agent.agent.context_vars` 的 `ContextVar`（`current_phase_var` / `current_step_var`）讀，未設 → `None`。
- **LLMCallLogger** — 同樣 `__init__(..., emitter=None)` + `set_emitter(emitter)`；`log` / `log_failure` 寫檔成功後 emit `llm_call`（`preview` 取 `request["messages"]` 第一個 `role="user"` 的 `content[:200]`；無 user msg → 空字串）。

```python
# codebus_agent/agent/emitter.py
@runtime_checkable
class SSEEmitter(Protocol):
    def emit(self, event: dict) -> None: ...

class NullEmitter:  # in-process default
    def emit(self, event: dict) -> None:
        return None

class TaskHandleEmitter:
    def __init__(self, handle: TaskHandle) -> None:
        self._handle = handle
    def emit(self, event: dict) -> None:
        self._handle.emit(event)
```

`POST /explore`（`api/explore.py`）在建完 `TaskHandle` + `TaskHandleEmitter` 後：
```python
reasoning_provider = app.state.llm_reasoning_provider(ws)
judge = LLMJudge(app.state.llm_judge_provider, ws)
reasoning_provider.set_emitter(emitter)  # propagates to inner LLMCallLogger
judge.set_emitter(emitter)
await run_explorer(..., emitter=emitter)  # Explorer loop 端也塞 emitter
```

對應 `sidecar-api.md` §四 SSE schema。

---

## 十三、LLM 稽核：UsageTracker + LLMCallLogger（D-021 / D-022）

> 兩個並列的稽核元件，都掛在 `TrackedProvider` wrapper 內。一個收**聚合數字**（token / cost）、一個收**完整 payload**（request / response）。

### 13.1 UsageTracker（D-021）

統一收集 LLM token / cost，寫第五層稽核 JSONL + SSE 即時廣播 + Budget 真實化。

### 責任
- 攔截所有 `LLMProvider.chat` / `chat_structured` / `chat_stream` / `embed` 的 `Usage`
- 標記 `module`（explorer / judge / coverage / generator / qa / kb_build）+ `phase`（scan / kb_build / explore / generate / qa）+ `step_id`（若在 ReAct 迴圈內）
- 寫 `{workspace}/token_usage.jsonl`
- SSE emit `usage_delta`（給前端 Agent console 即時顯示）
- 提供 `session_total()` 給 Budget 控制查即時用量

### `session_id` 與 `phase` 定義（重要）

| 欄位 | 定義 | 誰決定 |
|---|---|---|
| `session_id` | **一個 workspace open → close 的完整生命週期**，貫穿 scan / kb_build / explore / generate / qa 全部呼叫 | Tauri 開 workspace 時產生一次，存進 sidecar state |
| `phase` | 當下跑在哪個生命週期階段（scan / kb_build / explore / generate / qa） | Sidecar endpoint 進入時透過 context var `current_phase` 設定 |
| `module` | 實際打 LLM 的**邏輯組件**（explorer / judge / coverage / generator / qa / kb_build / embed） | 各 Agent / Module 自行標記 |

**為什麼 `phase` 跟 `module` 分開**：
- `phase=explore` 期間會同時用 `module=explorer` 與 `module=judge`（Judge 每步都跑）——同一 phase 多個 module
- `phase=qa` 期間 Q&A Agent 可能即時補查 → 也走 `module=explorer` tools（D-016）——同一 module 可能出現在多個 phase
- 「這條路線跑完花多少錢」= `by_phase` 的 `scan + kb_build + explore + generate` 總和
- Q&A 是 session 後續的互動，跟初次跑路線的帳分開看較合理

### 介面

```python
Phase = Literal["scan", "kb_build", "explore", "generate", "qa"]
Module = Literal["explorer", "judge", "coverage", "generator", "qa", "kb_build", "embed"]


class UsageRecord(BaseModel):
    ts: datetime
    session_id: str              # workspace 生命週期
    provider: str                # "contest-openai"
    model: str
    call_type: Literal["chat", "chat_structured", "chat_stream", "embed"]
    phase: Phase                 # scan / kb_build / explore / generate / qa
    module: Module
    step_id: int | None
    prompt_tokens: int
    completion_tokens: int
    embed_tokens: int
    cost_usd: float | None
    estimated: bool              # True = tiktoken 本地估（provider 沒回）


class UsageTracker:
    def __init__(self, path: Path, session_id: str, sse_emitter: SSEEmitter): ...

    async def record(
        self,
        *,
        usage: Usage,
        phase: Phase,            # 通常讀 context var current_phase()
        module: Module,
        step_id: int | None = None,
    ) -> None:
        """寫 JSONL + SSE emit + 累計到 session"""

    def session_total(self) -> dict[str, int | float | dict]:
        """即時回：
        {
          prompt_tokens, completion_tokens, embed_tokens, cost_usd,
          by_module: { 'explorer': 0.08, 'judge': 0.02, ... },
          by_phase:  { 'scan': 0.0, 'kb_build': 0.005, 'explore': 0.10, 'generate': 0.015, 'qa': 0.0 }
        }
        """

    async def emit_summary(self) -> None:
        """Session 結束（或 phase 結束）時呼叫，SSE emit 一筆
        { type: 'usage_summary', by_module, by_phase, ... }"""
```

### 落地點

**1. Provider 層裝飾器**（`llm-provider.md` §二 補）

```python
class TrackedProvider(LLMProvider):
    """Wrapper：每次 call 後呼叫 tracker.record 與 call_logger.log。

    `role` 為建構期必填參數（llm-role-routing，2026-04-20 落地），
    `TrackedProvider` 自動把 role 向下傳給 `LLMCallLogger`，呼叫端
    簽章不變。module 仍由 context var（`current_module`）標記。
    """
    def __init__(
        self,
        inner: LLMProvider,
        *,
        tracker: UsageTracker,
        logger: LLMCallLogger,
        role: ProviderRole,
    ): ...
```

**2. ToolContext 第 9 欄**（`tool-sandbox.md §五`）
`usage_tracker: UsageTracker` — tool 自己如果呼叫 LLM（例如未來 `evaluate_source` tool）可直接用。

**3. Embedding tracking**
`embed()` 回 `EmbedResponse(vectors, usage)`，Module 2 收到後呼叫 `ctx.usage_tracker.record(usage=..., phase="kb_build", module="embed")`。若 provider API 沒回 token 數 → `tiktoken.encode(text)` 估算 + `estimated=True`。

### Budget 互動

```python
def _check_budget(state: ExplorerState, tracker: UsageTracker, budget: Budget) -> Signal:
    total = tracker.session_total()
    used = total["prompt_tokens"] + total["completion_tokens"]
    if used >= budget.max_tokens:
        return Signal.FORCE_STOP
    if used >= budget.max_tokens * 0.8:
        return Signal.CONVERGE_HINT   # prompt 注入「預算快用完，開始收斂」
    return Signal.CONTINUE
```

### `token_usage.jsonl` 範例

```json
{"ts":"2026-04-18T10:00:01Z","session_id":"sess_abc","provider":"contest-openai","model":"gpt-4o","call_type":"chat_structured","module":"explorer","step_id":3,"prompt_tokens":1240,"completion_tokens":180,"embed_tokens":0,"cost_usd":0.0042,"estimated":false}
{"ts":"2026-04-18T10:00:02Z","session_id":"sess_abc","provider":"contest-openai","model":"text-embedding-3-small","call_type":"embed","module":"kb_build","step_id":null,"prompt_tokens":0,"completion_tokens":0,"embed_tokens":8432,"cost_usd":0.00017,"estimated":false}
```

### SSE event

```json
{ "type": "usage_delta", "module": "explorer", "step": 3, "prompt_tokens": 1240, "completion_tokens": 180, "cost_usd": 0.0042, "session_total_cost_usd": 0.031 }
{ "type": "usage_summary", "total_tokens": 45200, "total_cost_usd": 0.12, "by_module": { "explorer": 0.08, "judge": 0.02, "generator": 0.015, "kb_build": 0.005 } }
```

---

### 13.2 LLMCallLogger（D-022）

記錄所有 LLM call 的完整 request / response。第六層稽核 JSONL `llm_calls.jsonl`，並透過 SSE `llm_call` event 餵給前端 UI 的 **LLM Calls 分頁**（D-022），作為稽核 trail + Demo 透明度武器。

### 關鍵：log 的是 wire payload

`request.messages` 為 **post-Sanitizer Pass 2** 版本 —— 就是實際送出去的內容。`<REDACTED:kind#N>` placeholder 原樣保留。**不保留 pre-sanitize 原文**，零額外隱私面積。

### 介面

```python
class LLMCallRecord(BaseModel):
    request_id: str                  # "llm_abc123"
    ts: datetime
    session_id: str
    module: str
    step_id: int | None
    role: ProviderRole               # llm-role-routing — 由 TrackedProvider 綁入
    provider: str
    model: str
    call_type: Literal["chat", "chat_structured", "chat_stream", "embed"]

    # 完整 payload（post-sanitize）
    request: dict                    # { messages, tools, temperature, response_format, ... }
    response: dict | None            # { content, tool_calls, finish_reason } / None if error
    usage: Usage | None              # 對齊 D-021（embed 的也記）

    latency_ms: int
    truncated: bool = False          # 單筆 > 100KB 被截
    error: str | None = None         # provider 失敗時填


class LLMCallLogger:
    def __init__(
        self,
        path: Path,
        session_id: str,
        sse_emitter: SSEEmitter,
        max_record_bytes: int = 100_000,      # 單筆上限 100KB
        max_session_bytes: int = 50_000_000,  # 單 session 上限 50MB
    ): ...

    async def record(
        self,
        *,
        record: LLMCallRecord,
    ) -> None:
        """
        1. 計算 size；超過 max_record_bytes 截斷 messages/response 並標 truncated
        2. 檢查 session 累計；超 max_session_bytes 輪替 llm_calls.1.jsonl / .2.jsonl
        3. 寫 JSONL
        4. SSE emit（完整版，不截斷 — SSE 另有 buffering 機制）
        """
```

### 落地點：TrackedProvider

```python
class TrackedProvider(LLMProvider):
    def __init__(
        self,
        inner: LLMProvider,
        *,
        tracker: UsageTracker,
        logger: LLMCallLogger,
        role: ProviderRole,          # llm-role-routing — 建構期必填
    ):
        self._inner = inner
        self._usage = tracker
        self._calls = logger
        self._role = role

    async def chat_structured(self, messages, *, response_model, **kw):
        req_id = f"llm_{short_uuid()}"
        t0 = time.perf_counter()
        try:
            result = await self._inner.chat_structured(messages, response_model=response_model, **kw)
            latency = int((time.perf_counter() - t0) * 1000)
            # 從 ChatResponse 補齊 usage / content
            await self._usage.record(usage=result._usage, module=current_module(), step_id=current_step())
            await self._calls.record(record=LLMCallRecord(
                request_id=req_id,
                request={"messages": [m.model_dump() for m in messages],
                         "response_format": response_model.model_json_schema(),
                         **kw},
                response={"content": result.model_dump()},
                usage=result._usage,
                latency_ms=latency,
                call_type="chat_structured",
                module=current_module(),
                step_id=current_step(),
                provider=self._inner.name,
                model=result._model,
                session_id=current_session(),
                ts=utcnow(),
            ))
            return result
        except Exception as e:
            # 錯誤也記（response=None, error=...）
            await self._calls.record(record=LLMCallRecord(..., error=str(e), response=None, latency_ms=...))
            raise
```

### `llm_calls.jsonl` 範例

```json
{"request_id":"llm_abc123","ts":"2026-04-18T10:00:01Z","session_id":"sess_xyz","module":"explorer","step_id":3,"role":"reasoning","provider":"contest-openai","model":"gpt-4o","call_type":"chat_structured","request":{"messages":[{"role":"system","content":"You are an Explorer..."},{"role":"user","content":"task: 新增 GoogleDrive Adapter ... <REDACTED:email#0>..."}],"tools":[{"name":"search"},...],"temperature":0.2,"response_format":{"type":"json_schema","json_schema":{...}}},"response":{"content":{"thought":"...","tool_calls":[{"name":"search","args":{"query":"IStorageService"}}],"stop":false}},"usage":{"prompt_tokens":1240,"completion_tokens":180,"cost_usd":0.0042,"estimated":false},"latency_ms":1842,"truncated":false,"error":null}
```

### SSE event

```json
{ "type": "llm_call", "request_id": "llm_abc123", "module": "explorer", "step_id": 3, "model": "gpt-4o", "call_type": "chat_structured", "latency_ms": 1842, "tokens": { "prompt": 1240, "completion": 180 }, "cost_usd": 0.0042, "preview": "task: 新增 GoogleDrive Adapter..." }
```
前端 list 先靠 preview 渲染；點開 detail 再打 `GET /tasks/{id}/llm_calls/{request_id}` 拿完整 payload（避免 SSE 大訊息塞爆 channel）。

### 與其他稽核的關係

| 場景 | 看哪份 |
|---|---|
| 我想知道 session 總共花多少錢 | `token_usage.jsonl`（D-021 聚合） |
| 我想知道 Agent 第 5 步為什麼那樣決定 | `reasoning_log.jsonl`（Agent 視角） |
| 我想知道 LLM 實際收到什麼 prompt | `llm_calls.jsonl`（D-022 wire payload） ⭐ |
| 我想證明沒偷打 API / 沒外洩 | `llm_calls.jsonl` UI 分頁 ⭐ |

---

## 十四、測試策略

### 單元測試
- `_should_stop` / `_update_state` / 壓縮函式 — 純函式，直接單測
- Tool 各個獨立測（mock ToolContext）
- Prompt render function 測輸出字串包含關鍵片段

### Mock provider
參考 `llm-provider.md` §七 `MockProvider`：
- 吐預設 ExplorerAction 序列（scripted）
- Explorer 迴圈整體 E2E 可重現測試

### Golden sample regression
`tests/golden/timeline-gdrive-adapter/` 下跑：
```bash
pytest tests/golden/ --provider=contest
```
- 讀 `ideal-route.md` 的 must_have 清單
- 跑 Explorer 得 stations，算 recall / noise / depth
- 分數退步 > 5% → fail

---

## 十五、檔案結構（實作時）

Monorepo 下的 Python sidecar（D-013 / D-014，uv workspace）：

```
codebus/
└── sidecar/
    ├── pyproject.toml          # uv init --package
    ├── uv.lock
    └── src/codebus_agent/
        ├── __init__.py
        ├── api/                # FastAPI routes (sidecar-api.md)
        │   └── main.py
        ├── agent/
        │   ├── types.py        # 資料結構
        │   ├── explorer.py     # 主 ReAct 迴圈（探索階段）
        │   ├── qa.py           # Q&A Agent（D-016，reuse ReAct core）
        │   ├── judge.py
        │   ├── coverage.py
        │   ├── budget.py
        │   ├── context.py      # compression
        │   └── logger.py       # reasoning_log + SSE + kb_growth
        ├── prompts/
        │   ├── explorer.py
        │   ├── judge.py
        │   ├── coverage.py
        │   └── generator.py
        ├── tools/
        │   ├── __init__.py     # registry + decorator
        │   ├── search.py       # grep / KB search / kb_search
        │   ├── fs.py           # read_file / list_dir
        │   ├── code.py         # trace_import / find_callers
        │   └── kb_write.py     # add_to_kb（Q&A 專用，D-016）
        ├── providers/
        │   ├── base.py         # Protocol (llm-provider.md)
        │   ├── contest.py
        │   ├── mock.py
        │   └── sanitizer.py    # D-011 去識別化
        └── modules/
            ├── scanner/        # Module 1
            ├── kb/             # Module 2 (Qdrant client)
            └── generator/      # Module 5
```

---

## 十六、MVP 不做（明確記錄）

| 項 | 延後原因 |
|---|---|
| Multi-agent coordination（多 Explorer 並行） | MVP 單一 Explorer 夠 |
| LangGraph / DAG 式流程編排 | 單 while 迴圈夠讀，不需 DAG |
| Prompt A/B test 框架 | 手動對比 golden sample 分數先夠 |
| 自動 prompt 優化（DSPy 類） | Phase 3 |
| Agent persistent memory（跨 session） | 目前 per-task session，kb 就是持久層 |
| Human-in-the-loop approval 節點 | 介入點在使用者層（D-006），不在 Agent 內 |

---

## 十七、實作順序建議

1. **Day 1-2**: types + mock provider + explorer 最小骨架（ReAct 跑得動、一個假工具）
2. **Day 3-4**: 真工具（search / read_file / list_dir）+ 串 KB
3. **Day 5**: Judge（先極簡 prompt）+ reasoning_log 寫檔
4. **Day 6**: trace_import / find_callers（最有戲的工具）
5. **Day 7**: Coverage Checker + gap 補查遞迴
6. **Day 8**: Context 壓縮 + Budget 控制
7. **Day 9**: SSE emit 串前端
8. **Day 10+**: Golden sample 跑起來 + prompt 調整
