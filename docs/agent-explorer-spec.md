# Explorer Agent Spec

> CodeBus 的 Agentic 核心：像工程師一樣探索 codebase，而不是無腦 RAG。

---

## 一、設計目標

**Explorer Agent 要做什麼**

給定任務描述與 codebase，Agent 自主決定讀哪些檔案、追哪些依賴、什麼時候停、哪裡還有 gap，最後產出學習路線與完整決策紀錄。

**為什麼需要 Agent，不是 RAG pipeline**

- RAG：任務 → embedding 查相似 chunk → 塞進 prompt → 答。**沒有自主決策**。
- Explorer Agent：任務 → 自己選策略、追線索、評估相關度、發現 gap 補查、決定何時收斂。**每步都是決策**。

**Demo 魔法**：每步決策印出 thought 給前端顯示，觀眾看得到 Agent 的思考過程。

---

## 二、Agent 狀態（State）

```python
# 策略層的示意定義；完整 Pydantic schema 在 agent-core.md §三
from typing import Literal
from pydantic import BaseModel
from pathlib import Path

class ExplorerState(BaseModel):
    task: str                        # 使用者任務描述
    goal_keywords: list[str]         # Agent 解析任務抽出的關鍵字
    queue: list["Target"]            # 待探索清單（以 priority 欄位排序）
    visited: set[str]                # 已讀過的相對路徑
    findings: list["Finding"]        # 建立的知識
    reasoning_log_path: Path         # 決策紀錄 JSONL（前端 SSE 串接）
    budget: "Budget"                 # 剩餘步數 / tokens / wall seconds

class Finding(BaseModel):
    path: str                        # 相對 workspace_root
    relevance: int                   # 1-5
    role: Literal["入口點", "依賴", "配置", "次要"]
    why: str                         # Agent 為何覺得重要
    depends_on: list[str] = []

class Decision(BaseModel):
    step: int
    thought: str
    action: "ToolCall"               # 見 agent-core.md §三
    observation: str

class Target(BaseModel):
    kind: Literal["search", "read_file", "trace_import", "find_callers"]
    args: dict
    priority: int = 0
```

> 本節是**策略層**示意，類別定義權威版在 `agent-core.md` §三（D-012 / D-013 決定的 Python + Pydantic + Instructor 實作）。

---

## 三、Agent 可用工具（Tool Use）

| 工具 | 簽名 | 用途 | 狀態 |
|---|---|---|---|
| `search` | `(keyword: str) -> list[SearchHit]` | KB query（優先）或 grep fallback | ✅ P0 landed（`explorer-tools-p0`） |
| `list_dir` | `(path: str) -> list[DirEntry]` | 看目錄結構（一層；`.codebus` 排除） | ✅ P0 landed |
| `read_file` | `(path: str, line_range: tuple[int,int] \| None = None) -> str` | 讀檔（Pass 1 sanitize + >12k truncate） | ✅ P0 landed |
| `trace_import` | `(symbol: str) -> str \| None` | 追某個 import 的來源 | ✅ 步驟 19 landed（`explorer-tools-p1`） |
| `find_callers` | `(symbol: str) -> list[FileMatch]` | 找誰呼叫這個符號 | ✅ 步驟 19 landed（`explorer-tools-p1`） |
| `mark_station` | `(path: str, role: str, why: str)` | 把檔案標為學習站（`relevance=0.8` P0） | ✅ P0 landed |
| `add_to_queue` | `(target: Target, priority: int, why: str)` | 把新目標加入探索清單 | ⏳ 後續（Explorer 迴圈 `_update_state` 代勞中） |
| `stop` | `(reason: str)` | 決定探索夠了，收斂 | ⏳ 後續（`ExplorerAction.stop` 欄位代勞中） |

**關鍵設計**：`mark_station` 和 `add_to_queue` 都要 Agent 給 `why`，才能在前端顯示決策理由。

**P0 落地細節**（`explorer-tools-p0`，2026-04-24 archive）：四個 P0 tool 都透過 `codebus_agent.agent.tools.folder_tools.FolderTools` 實作；`search` 的 KB 走 `ctx.kb.query(keyword)`，回傳 `SearchHit(path, snippet, score)` 的 path 相對 workspace_root；grep fallback 走 text-file 副檔名 (`.py` / `.md` / `.ts` 等) + 512KB 上限，結果 cap 100。`read_file` 的輸出**一律**過 `ctx.sanitizer` Pass 1；`ctx.sanitizer=None` 時 fail-loud raise `ValueError`。每次 tool 呼叫都透過 `codebus_agent.sandbox.append_tool_audit_line` 共用 writer 寫一行 `tool_audit.jsonl`（含 allow/deny）。

**P1 落地細節**（`explorer-tools-p1`，步驟 19）：`trace_import` / `find_callers` 掛在同一 `FolderTools` 類別上；用 language-neutral 正則（Python / TS / JS / Go / Rust 的 def / class / function / struct / enum / trait），`re.escape(symbol)` 防注入。`trace_import` 依 `(path_depth, relative_path)` 排序候選，第一個命中即回 relative path，或 `None`；symlink 指向 workspace 外者被 `ensure_in_workspace` 拒絕並寫 `tool_audit.jsonl` `allowed=false` 行。`find_callers` 用 `\b<escaped_symbol>\b` whole-word 匹配，per-file ≤ 5、global ≤ 100、`(path_depth, path, line)` 排序；每行過 Pass 1 sanitize（截到 200 字），命中寫 `sanitize_audit.jsonl` `pass_num=1`；`ctx.sanitizer=None` 時 fail-loud；排除 `trace_import` 回的定義行以避免重覆。

---

## 四、主決策迴圈（ReAct 風格）

```
INIT:
  parse_task(task) → goal_keywords
  queue.push(每個 keyword 的 search)

LOOP (while !done && budget > 0):

  [1] Think
    LLM(state) → {
      thought: "我目前知道...，下一步要...",
      action: <工具呼叫>
    }

  [2] Act
    執行 action → observation

  [3] Judge（子 Agent，僅 read_file 後觸發）
    LLM(file_content, task) → {
      relevance: 1-5,
      role: ...,
      should_trace_imports: bool,
      should_find_callers: bool,
      reasoning: ...
    }
    若 relevance >= 3 → mark_station
    若 should_trace_imports → add_to_queue 追依賴

  [4] Update state + log decision

  [5] Converge check（每 N 步觸發一次）
    Coverage Checker LLM:
      - goal_keywords 是否都有 finding？
      - findings 之間依賴關係完整嗎？
      - 有明顯 gap 嗎？
    若 OK → stop
    若有 gap → 把 gap 變成新 target 丟進 queue
```

### 收斂條件（三擇一）

1. Agent 自己呼叫 `stop()`（信心夠）
2. Coverage Checker 判定覆蓋完整
3. Budget 用完（fallback：用已有 findings 產路線）

---

## 五、Prompt 架構（三層 Agent）

### Layer 1 — Explorer（主決策）

```
你是在陌生 codebase 找線索的偵探。

任務：{task}
目前已知（摘要）：{findings_summary}
Queue 前 5 個目標：{queue_top5}
已用步數 / 預算：{used} / {budget}

思考：現在最該看什麼？為什麼？
用 JSON 輸出：
{
  "thought": "一句話說明你的判斷",
  "action": { "tool": "...", "args": {...} }
}
```

### Layer 2 — Relevance Judge（讀完檔案後）

```
任務：{task}
剛讀了：{file_path}
內容（前 N 行）：{file_content}

評估：
1. relevance (1-5)
2. role（入口 / 依賴 / 配置 / 次要 / 無關）
3. 要不要追 imports？列出符號
4. 要不要找 callers？列出符號
5. 理由（一句話）

輸出 JSON。
```

### Layer 3 — Coverage Checker（收斂時）

```
任務：{task}
已蒐集的 findings：{findings}

判斷：
- 要產學習路線，還缺什麼？
- 有哪些「使用者一定會問但還沒解答」的 gap？
- 依賴關係完整嗎？

輸出 JSON:
{
  "verdict": "continue" | "done",
  "gaps": [...],        // 若 continue，列出新 targets
  "reasoning": "..."
}
```

---

## 六、狀態機

```
    IDLE
     │
     ▼
  PLANNING ─── parse_task
     │
     ▼
  EXPLORING ◄──────┐
     │             │
     ▼             │
  JUDGING          │ (gap found)
     │             │
     ▼             │
  CONVERGE_CHECK ──┘
     │ (verdict=done)
     ▼
    DONE
```

---

## 七、前端視覺化（Demo 核心）

學習介面右側開一個 console，即時顯示 Agent 的每步決策：

```
🎯 任務：修改 OTA 功能
💭 解析任務關鍵字：["ota", "firmware", "update"]

[步驟 1] 🔍 search("ota")
         → 找到 3 個檔案
         💭 先從最像入口的 src/ota/manager.py 開始

[步驟 2] 📖 read src/ota/manager.py (1-80)
         🧠 relevance=5, role=入口點
         💭 這檔案 import 了 MQTTClient，OTA 走 MQTT，要追進去

[步驟 3] 🔗 trace_import("MQTTClient")
         → src/mqtt/client.py
         💭 加入 queue（優先級高）

[步驟 4] 📖 read src/mqtt/client.py
         🧠 relevance=4, role=依賴
         💭 標記為「核心站」— 任務必懂

[步驟 5] 🔍 search("firmware_update")
         → 0 筆
         💭 關鍵字可能不叫這個，改查 "upgrade"

...

[步驟 18] ✅ Coverage OK — 已涵蓋 OTA 流程、MQTT 通訊、版本檢查
         停止探索，進入路線規劃
```

**視覺化設計重點**

- 每步動畫展開，像偵探辦案
- 「💭」後的 thought 是 agentic 最硬的證據
- 失敗的嘗試**也要顯示**（步驟 5 的 0 筆），展示 Agent 會換策略
- 可以回放（slider）讓使用者重看決策歷程

---

## 八、資料格式

### Decision Log（寫入 reasoning_log.jsonl）

```json
{"step": 1, "action": {"tool": "search", "args": {"keyword": "ota"}}, "thought": "先從任務關鍵字找入口", "observation": "找到 3 檔案", "ts": "..."}
{"step": 2, "action": {"tool": "read_file", "args": {"path": "src/ota/manager.py"}}, "thought": "...", "observation": "...", "judge": {"relevance": 5, "role": "入口點"}, "ts": "..."}
```

### Final Findings（輸入給 Markdown Generator）

```json
{
  "task": "修改 OTA 功能",
  "findings": [
    {
      "path": "src/ota/manager.py",
      "relevance": 5,
      "role": "入口點",
      "why": "OTA 流程的主入口，協調整個更新流程",
      "depends_on": ["src/mqtt/client.py", "src/version.py"]
    }
  ],
  "reasoning_log_path": "reasoning_log.jsonl"
}
```

---

## 九、實作優先序

| 優先級 | 項目 | 理由 |
|---|---|---|
| P0 | Layer 1 + 4 個工具（search / read_file / list_dir / mark_station） | 能跑最小 loop |
| P0 | reasoning_log + 前端 console | Demo 靈魂 |
| P0 | Budget 收斂 + fallback | 保底，不會跑飛 |
| P1 | Layer 2 Relevance Judge | 品質關鍵 |
| P1 | trace_import / find_callers | 差異化武器 |
| P2 | Layer 3 Coverage Checker | 沒它可以用 budget 收斂 |
| P2 | Gap 補探索 loop | Phase 2 強化 |
| P2 | Decision log 回放 UI | Demo 加分 |

---

## 十、風險與對策

| 風險 | 對策 |
|---|---|
| Agent 無限迴圈 / 跑飛 | 硬性 budget 上限 + 每步檢查是否卡在同目標 |
| LLM 輸出格式跑掉 | Instructor + Pydantic schema 驗證（D-012）+ 重試 |
| 探索路徑不穩定 | 固定 seed + 溫度調低 + 記錄 prompt 做 regression |
| Context 爆掉 | findings 只存摘要；完整內容另存；read_file 限行數 |
| 相關度判斷不準 | 人工標 10 個 golden case，調 Judge prompt 到準確率 > 80% |

---

## 十一、評估方式

為了避免「反覆調 prompt」流於感覺，MVP 就要有：

1. **Golden samples**：選 2 個熟悉的 repo + 3 個任務，人工寫「理想路線」（Phase A 落地：`tests/golden/demo-synthetic/`、`tests/golden/timeline-storage-adapter-synthetic/`，待打磨期擴第二份語言 fixture）
2. **自動評分指標**（landed at `sidecar/tests/golden/scoring.py`，可重複用於未來 Explorer / Q&A / Generator fixture）：
   - 核心檔案召回率 — `station_recall(produced, must_have)`：`len(p & m) / len(m)`，空 must_have raise `ValueError`
   - 雜訊率 — `station_noise(produced, must_have, nice_to_have)`：`len(extras - nice_to_have) / len(extras)`，空 extras 回 `0.0`（合法 clean output）
   - 加權合分 — `composite_score(recall, noise, depth, weights=None)`：D-006 公式 `0.5 * recall + 0.3 * (1 - noise) + 0.2 * depth`，default weights 寫死可注入 override（缺 key 必 raise `KeyError`）
   - 依賴完整度 `depth` —— P0 暫回 `1.0` placeholder，等 Module 5 Generator 把 station `depends_on` 從教材 MOC 圖反向填回後再開新 change 實作 dep-chain 解析
   - `IdealRoute` Pydantic schema：四欄 `task` / `must_have` / `nice_to_have` / `noise_paths`，`tests/golden/<fixture>/ideal-route.json` 為機器讀的真相
3. **人工評分 rubric**（1-5 分）：
   - 路線順序合理性
   - 站牌粒度適中
   - 新人能看懂

**Live LLM snapshot replay** 待打磨期（D-006 後續清單 `[ ] 真 LLM snapshot`）；目前 `sidecar/tests/golden/test_timeline_synthetic_replay.py` 與 `test_explorer_replay.py` 走 scripted MockProvider，介面 LLM-agnostic — 換 `OpenAIChatProvider` 一行替換即可接真 LLM。

---

## 十二、Topic Mode 延伸

**核心論點**：Topic mode **不是重寫，是換工具集**。主迴圈、狀態、視覺化、收斂條件、self-review 概念完全共用。

```
ExplorerCore (ReAct loop) ← 共用
  ├─ FolderMode: FolderTools + FolderJudge + FolderCoverage     (Phase 1)
  ├─ QAMode:     QATools     + QASelfCheck                      (Phase 1, D-016)
  └─ TopicMode:  TopicTools  + TopicJudge  + TopicCoverage      (Phase 2)
```

### 12.1 為什麼 Topic mode 更 agentic

Codebase 是「存在的事實」—— Agent 只要找到就好。
Web 是「可能錯的意見」—— Agent 要判斷**品質、時效、衝突、覆蓋深度**，決策點更多。

### 12.2 Topic 模式的工具集

| 工具 | 簽名 | 用途 |
|---|---|---|
| `web_search` | `(query: str) -> list[Result]` | 通用搜尋 |
| `fetch_page` | `(url: str) -> Content` | 讀網頁內容 |
| `search_docs` | `(tech_name: str) -> str` | 直接找官方文件 URL |
| `search_youtube` | `(query: str) -> list[Video]` | 找教學影片 |
| `search_github` | `(query: str) -> list[Repo]` | 找範例 repo |
| `evaluate_source` | `(url: str, content: str) -> Quality` | 判斷品質 |
| `mark_material` | `(url: str, role: str, why: str)` | 標記納入教材 |
| `add_subtopic` | `(topic: str, why: str)` | 發現前置知識 → 開子主題 |
| `stop` | `(reason: str)` | 收斂 |

### 12.3 Topic mode 特有的決策點

1. **source 優先序**：官方文件先、還是部落格先？新手要影片入門、進階要 RFC
2. **品質判斷**：作者權威性、時效性（React 18 vs 19）、深度是否符合使用者程度
3. **衝突處理**：兩個來源講法不同 → 採信哪個？交叉驗證？
4. **陌生術語 → 子主題**：讀 MQTT 看到 QoS，要不要先補「pub/sub 模式」？
5. **廣度 vs 深度**：Agent 自己取捨

### 12.4 Topic Judge Prompt（Layer 2，對應 Folder 的 Relevance Judge）

```
Topic：{topic}
使用者程度：{level}
剛讀了：{url}
內容摘要：{content}

評估：
1. relevance (1-5)
2. source_quality (1-5)  ← 作者權威性、時效性
3. level_match（符合使用者程度？）
4. role（入門教學 / 深入解析 / 參考 / 次要 / 過時）
5. 同來源其他內容要不要繼續挖？
6. 發現陌生術語 → 列出要開的子主題
7. 理由（一句話）

輸出 JSON。
```

### 12.5 Topic Coverage Checker（Layer 3）

```
Topic：{topic}
使用者程度：{level}
已蒐集 materials：{materials}

判斷：
- 核心概念覆蓋了嗎？
- 難度梯度合理嗎？（新手 → 進階漸進）
- 有重複來源？要合併？
- 有前置知識 gap（假設會 X 但沒教）？
- 有版本衝突？

輸出 JSON:
{
  "verdict": "continue" | "done",
  "gaps": [...],
  "redundant_pairs": [...],
  "reasoning": "..."
}
```

### 12.6 共用 vs 不共用（一目瞭然）

Q&A mode（D-016 / Module 8）也走同一套抽象，跟 Folder / Topic 並列。

| 組件 | Folder | Topic | Q&A | 共用？ |
|---|---|---|---|---|
| 主 ReAct loop | ✅ | ✅ | ✅（RAG 不夠時才進） | **共用** |
| ExplorerState 結構 | ✅ | ✅ | ✅ | **共用**（findings → materials / answer chunks） |
| reasoning_log 格式 | ✅ | ✅ | ✅ | **共用** |
| 前端 console 視覺化 | ✅ | ✅ | ✅ | **共用** |
| 狀態機 | ✅ | ✅ | ✅ | **共用** |
| Budget / 收斂機制 | ✅ | ✅ | ✅（收斂條件不同） | 機制共用，條件各自 |
| 工具集 | grep / read_file / trace_import | web_search / fetch_page / evaluate_source | Folder 前 5 個 + `kb_search` + `add_to_kb` | 各自 |
| Judge prompt | Relevance Judge | Quality + Level Judge | Self-check（add_to_kb 前自我確認） | 各自 |
| Coverage Checker prompt | 依賴完整度 | 難度梯度 + 衝突檢核 | 無（改用「答案充分 + 輸出完成」判準） | 各自 |
| KB 寫入權 | ❌ | ❌ | ✅（`add_to_kb`） | 各自 |

### 12.7 Protocol 抽象設計（Phase 1 就要做對）

Phase 1 只實作 Folder，但 Protocol 抽象 day 1 就設計好（D-012 / D-013：Python + Pydantic + Instructor），Phase 2 加 Topic 不用動核心：

```python
from typing import Protocol

class ExplorerTools(Protocol):
    async def primary_search(self, query: str) -> list[SearchHit]: ...
    async def fetch(self, target: Target) -> Content: ...
    async def follow_reference(self, symbol: str) -> list[Target]: ...

class Judge(Protocol):
    async def judge(self, task: str, content: Content) -> JudgeVerdict: ...

class CoverageChecker(Protocol):
    async def check(self, task: str, findings: list[Finding]) -> CoverageVerdict: ...

# 共用 Explorer 核心 —— 透過 DI 注入 tools / judge / coverage
async def run_explorer(
    state: ExplorerState,
    tools: ExplorerTools,
    judge: Judge,
    coverage: CoverageChecker,
    provider: LLMProvider,
) -> ExplorerResult:
    """主 ReAct loop — 完整實作詳見 agent-core.md §四"""
    ...

# Phase 1 — Folder mode
class FolderTools:       # search / read_file / trace_import / find_callers / list_dir
    ...
class FolderJudge: ...
class FolderCoverage: ...

# Phase 1 — Q&A mode（D-016 / Module 8，MVP 必做）
class QATools:           # 重用 FolderTools 的 5 個 + kb_search + add_to_kb
    ...
class QASelfCheck: ...   # add_to_kb 前的 Judge（見 qa-agent.md §六）
# Q&A 不用 CoverageChecker；收斂條件由主迴圈判斷「答案充分 + 輸出完成」

# Phase 2 — Topic mode
class TopicTools:        # web_search / fetch_page / evaluate_source / ...
    ...
class TopicJudge: ...
class TopicCoverage: ...
```

**為何用 Protocol 而非 ABC**：Protocol 是 structural typing（duck typing 的靜態化），不需要具體類別 `inherit`，golden sample 與 MockProvider 測試都能自然滿足介面；對 Pyright / mypy 友善。

### 12.8 Topic mode 特有風險

| 風險 | 對策 |
|---|---|
| 搜到過時文件（React 17 vs 19） | Judge 把時效列為必評項；fetch 時帶日期 metadata |
| 付費牆 / 登入牆 | fetch_page 偵測後跳過，不浪費 budget |
| 來源衝突 | Coverage Checker 偵測到就觸發「交叉驗證」子任務 |
| web fetch 速率 | 全域節流 + retry with backoff |
| 搜尋結果噪音 | evaluate_source 設高門檻，低分不納入 |
| 版權問題 | 教材只引用 + 連結，不整頁貼 |

### 12.9 對外話術（常見問答用）

**Q：Topic mode 在 Phase 2，那 agentic 故事是不是打折？**

A：不是。架構 day 1 就是**雙模式設計**，Trait 抽象已經支援，切換只是換工具集和 Judge prompt。Phase 2 是**實作工期**問題（爬蟲節流、HTML parse、來源白名單都要時間），不是**設計**問題。spec 已完整寫好在 `docs/agent-explorer-spec.md` 第十二章。
