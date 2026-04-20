# Q&A Agent Spec — 教材完成後的互動問答

> 使用者完成學習路線後可繼續問問題；Agent 走 RAG + 必要時即時補查並自動沉澱進 KB。
> 關聯決策：**D-016（Q&A 互動 + KB 自動成長）**、D-002（trait 抽象）、D-015（Sanitizer）、D-011（資安）、**D-029（stable station id 引用）**。
> 關聯文件：`agent-explorer-spec.md`（trait 介面）、`agent-core.md`（ReAct loop 實作）、`sanitizer.md`、`module-5-generator.md §7.4`（stable station id 規則）。

---

## 一、定位

教材產出後，使用者開啟對話式問答。Q&A Agent：

1. 先走 **RAG（KB 查詢）** 回答
2. KB 不夠 → 決策**在 workspace 內即時補查**（read_file / search / trace_import / find_callers）
3. 拿到新資料 → 判斷**是否值得沉澱**進 KB
4. 值得 → 過 Sanitizer → `add_to_kb` → KB 永久累積（附 stable station id 引用，跨 session 可追溯脈絡）

**核心賣點**：KB 不是一次性建好、之後凍結——**問答本身就是 KB 成長機制**。完美契合「持久化知識庫」賣點與「Agentic」敘事（使用者端持續可感）。

**Station 脈絡保留**（D-029）：當 Q&A 從教材站點的 `<QAEntry>` 發起、或問題明顯關聯某站時，`add_to_kb` 會把 stable station id（`s{NN}-slug`）寫進 chunk metadata；後續檢索可過濾到相關站點，UI 也能顯示「此 KB 記錄源自 s02-storage-contract」。

---

## 二、與 Explorer Agent 的關係

共用 ReAct Core（`agent-core.md` §四），但身份不同：

| 層面 | Explorer Agent | Q&A Agent |
|---|---|---|
| 入口 | 使用者給 task | 使用者輸入問題 |
| 收斂條件 | Coverage Checker 通過 | 答案充分 + 輸出完成 |
| 輸出 | stations list | 自然語言回答 + 引用 |
| Tool set | search / list_dir / read_file / trace_import / find_callers / mark_station / add_to_queue / stop | **共用前 5 個** + `kb_search` + `add_to_kb` |
| KB 寫入權 | ❌（只讀） | ✅（透過 `add_to_kb`） |
| 預算 | 大（探索式） | 小（單問題 max 10 步） |

**實作上透過 D-002 的 trait 抽象 reuse**：Python `Protocol` / ABC 層定義 `ExplorerTools` / `Judge` / `CoverageChecker`，Q&A Agent 換 prompt + 加 KB 寫入 tool 即可。

---

## 三、Tool set

### Reused from Explorer
- `search(query, scope)` — grep / regex 在 workspace 內
- `read_file(path, line_range)` — 讀檔案
- `list_dir(path)` — 列目錄
- `trace_import(symbol)` — 追 import
- `find_callers(symbol)` — 找呼叫者

### Q&A 專用新增

#### `kb_search(query, top_k=5)`
```python
@tool(name="kb_search", description="向量查詢 KB")
async def kb_search(args: KBSearchArgs, ctx: ToolContext) -> str:
    hits = await ctx.kb.query(args.query, top_k=args.top_k)
    return _format_hits(hits)  # file:line + snippet + score
```

#### `add_to_kb(chunks, source, reason)`

**Chunk schema**（Pydantic）：

```python
class AddToKBChunk(BaseModel):
    text: str
    source: str                                       # "src/foo.py:120-180"（必填）
    related_stations: list[str] = Field(default_factory=list)  # stable station ids，D-029
```

- `related_stations`: 0..N 個 stable station id（`s{NN}-slug` 格式，見 `module-5-generator.md §7.4`）
- Agent 何時該填：
  - 當 Q&A session 從 `<QAEntry>` 發起（前端會在 `/qa` request 夾 originating `station_id`，寫進 `QAState.originating_station_id`）
  - 當問題內容明顯關聯某站（e.g. 使用者問「剛剛 storage 那段...」）
  - 當 ReAct loop 中 Agent 從 `kb_search` hit 的 chunk metadata 看到既有 station 關聯，延伸 chunk 應繼承
- 格式驗證：每個 id 必須符合 `^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$`；不符則 tool 回 validation error，prompt 要求重填

```python
@tool(name="add_to_kb", description="將新資訊加入 KB 供未來查詢")
async def add_to_kb(args: AddToKBArgs, ctx: ToolContext) -> str:
    added: list[str] = []
    for chunk in args.chunks:
        # 1. 過 Sanitizer（D-015 三段式的 Pass 3）
        clean = ctx.sanitizer.scrub(chunk.text)
        if not clean.strip():
            added.append("skipped_empty")
            continue

        # 2. 驗 related_stations 格式（D-029）
        for sid in chunk.related_stations:
            if not STATION_ID_REGEX.match(sid):
                return f"invalid station_id: {sid}"

        # 3. 組 payload；embed + Layer 1 hash dedup + Layer 2 similarity dedup
        #    全部封裝在 KB 層（見 module-2-kb-builder.md §五、§七）
        file_path, line_start, line_end = _split_source(chunk.source)  # "path:start-end" → triple
        payload = KBPayload(
            source_kind="code",                      # 由 chunk.source 推斷 code / doc
            file_path=file_path,                     # bare path, e.g. "src/foo.py"
            line_start=line_start,
            line_end=line_end,
            text=clean,
            text_hash=sha256(clean.strip().encode()).hexdigest(),
            added_by="qa_agent",
            session_id=ctx.session_id,
            chunk_index=0,
            chunk_total=1,                           # add_to_kb 為單 chunk 上傳
            created_at=utcnow(),
            related_stations=chunk.related_stations, # D-029：stable station id 脈絡
            # sanitize_stats 由 KB 層或呼叫者填；若 scrub 不回 stats 用 default {}
        )
        try:
            point_id = await ctx.kb.upsert_chunk(clean, payload=payload)
        except KBGrowthExceeded as e:
            return f"budget exhausted: {e}"

        # 4. 寫稽核 log（kb_growth.jsonl，UI 可看 / rollback）
        await ctx.kb_growth_log.write(point_id, chunk, args.reason)
        added.append(point_id)
    return f"added {len(added)} chunks: {added}"
```

**寫入權限邊界**：`add_to_kb` 只寫本地 Qdrant 集合，不碰 codebase、不碰外部。Tool Sandbox 視為 read-only tool（對 code 而言），不需要新 exec 能力。

**檢索脈絡還原**（D-029）：`kb_search` 回傳的 hit 除 `file:line + snippet + score` 外，也一併帶 `related_stations`；Agent prompt 指示「若 hit 有 related_stations，回答時附『此資訊源自 [站名]』引用，並可在 UI 生成 `[text](../stations/{station_id}.md)` 反連」。

---

## 四、主迴圈

```python
async def run_qa(question: str, state: QAState, ...) -> QAAnswer:
    # 階段 1: RAG 先行
    initial_hits = await ctx.kb.query(question, top_k=8)

    if _hits_confident(initial_hits, question):
        return await _answer_from_hits(question, initial_hits, provider)

    # 階段 2: RAG 不夠 → 進 ReAct loop
    state.messages.append(_build_qa_prompt(question, initial_hits))

    while not _should_stop(state):
        thought, tool_calls = await _think(state, provider, tools)
        results = await _execute_tools(tool_calls, tools, state)
        _append_observations(state, tool_calls, results)
        await logger.write(Step(...))
        state.step_count += 1

    # 階段 3: 收尾 — Agent 自主決定是否 add_to_kb
    # （在迴圈內 Agent 看到值得沉澱的 chunk 自己會呼叫 add_to_kb tool，不用外層強制）

    return await _synthesize_answer(state, provider)
```

**Budget**：`max_steps=10`、`max_tokens=50_000`、`max_wall_seconds=60`（比 Explorer 緊很多，Q&A 不該走太長）。

**什麼算 `_hits_confident`**
- Top-1 相似度 > 0.75
- Top-3 平均 > 0.65
- Top-5 有涵蓋 question 關鍵實體（entity check）
- 都通過 → 直接 RAG 回答，跳過 ReAct 迴圈

---

## 五、「值得沉澱」判斷規則

Agent 呼叫 `add_to_kb` 前，prompt 要求它先自我確認。規則寫進 Q&A system prompt：

```
當你獲得新資訊並考慮加入 KB，必須滿足全部三項才呼叫 add_to_kb：

1. **可復用**：這個資訊對未來其他問題有用（不是只解這一次的問題）
   反例：使用者自身情境（「我用 Mac」）→ 不加
   正例：程式碼結構事實（「PaymentService 在 src/services/payment.ts」）→ 加

2. **Stable fact**：資訊是 repo 本身的結構 / 行為 / 設計
   反例：當前 bug 狀態、臨時 debug 輸出 → 不加
   正例：檔案間依賴、函式簽章、設計決策摘要 → 加

3. **非同義重複**：確認你拿到的是 KB 目前沒有的資訊
   若 kb_search 找到相似內容 → 不加
   （系統也會用向量相似度 > 0.95 自動去重，但你應先判斷）

Station 脈絡（D-029，選填但建議）：
- 若問題從某站 `<QAEntry>` 發起 → `related_stations` 填該站 `station_id`
- 若問題內容明顯關聯某站主題 → 加該站 `station_id`
- 若延伸自既有 KB hit 且 hit 帶 related_stations → 繼承
- 格式：`s{NN}-slug`（e.g. `["s02-storage-contract"]`）
- 不填不算錯；填錯格式會被 tool 擋下要求重填

呼叫格式：
add_to_kb(chunks=[{text, source, related_stations?}], reason="why worth keeping")
```

Dedup 交給兩層：Agent 判斷（粗）+ 向量相似度（細）。

---

## 六、稽核 — `kb_growth.jsonl`

每次 `add_to_kb` 寫一筆：

```json
{
  "ts": "2026-04-17T14:20:00Z",
  "session_id": "qa_sess_abc",
  "question": "PaymentService 怎麼處理退款？",
  "originating_station_id": "s04-payment-flow",
  "entry_id": "qdrant-id-xyz",
  "source": "src/services/payment.ts:120-180",
  "related_stations": ["s04-payment-flow"],
  "reason": "PaymentService.refund() 的 state machine 在 KB 沒涵蓋",
  "sanitize_stats": { "email": 0, "secret": 0 },
  "chunk_size_chars": 842,
  "dedup_skipped": false
}
```

- `originating_station_id`：session 發起點（前端從 `<QAEntry>` 帶入），為 null 表示從全域 QA 入口發起
- `related_stations`：chunk metadata 上的 D-029 station 引用（與 KBPayload 同步）

### UI 稽核頁（延伸 Sanitizer 那頁）

```
🛡️ 本次 session 稽核
├─ Sanitizer: 替換 N 筆
└─ KB Growth: 新增 M 筆
     - src/services/payment.ts:120-180 (reason: ...)
     - ...
     [Rollback] 可回滾該筆
```

**Rollback 機制**：使用者點 rollback → Qdrant delete entry + kb_growth.jsonl append `rollback` event。MVP 支援單筆 rollback；批次 rollback 留 Phase 2。

---

## 七、成長防呆

避免 Agent 暴走灌爆 KB：

| 防呆 | 預設 |
|---|---|
| 單 session 最多 add_to_kb 筆數 | 20 |
| 單 chunk 最大字元 | 2000 |
| 相似度 dedup 閾值 | 0.95 |
| 單 question 最多觸發 add_to_kb 次數 | 5 |
| KB 總大小警告閾值 | 100MB（UI 提示，不擋） |

超上限：tool 回 error 給 Agent，prompt 指示「budget 用完，請完成回答」。

---

## 八、前端聊天 UI 契約

### 基本元件
- 對話氣泡（使用者 / Agent）
- Agent 氣泡下方顯示**引用**（file:line，可點開 side panel 看原檔）
- Agent 氣泡下方可顯示 **Station 引用** badge（D-029）：若回答內容源自帶 `related_stations` 的 KB hit，顯示「📍 s02-storage-contract」等可點擊 badge，跳教材對應站
- 每輪對話可展開看 Agent 的 reasoning log（reuse Explorer Agent console）
- 輸入框 + 送出

### 訊息流（透過 Sidecar SSE）

```
POST /qa        → body: {question, originating_station_id?} → 建立 session + 回 task_id
GET /tasks/{id}/events  (SSE)
    ├─ {"type": "rag_hits", "hits": [{..., "related_stations": [...]}]}
    ├─ {"type": "agent_thought", ...}
    ├─ {"type": "agent_action_result", ...}
    ├─ {"type": "kb_growth", "entry_id": "...", "source": "...", "related_stations": [...]}
    ├─ {"type": "answer_stream", "delta": "..."}
    └─ {"type": "done"}
```

- `POST /qa` 新增 optional `originating_station_id`：前端從 `<QAEntry>` 點擊時夾帶所在 station 檔的 `station_id`，Agent 看到會在 system prompt 注入「本 session 源自 {station_id}」脈絡
- `rag_hits` / `kb_growth` 事件都附 `related_stations`，前端即可渲染 station badge

**KB growth 事件即時推給前端**，UI 在答案下方或側欄即時顯示「📚 KB 新增 1 筆」，使用者看得到 KB 在長。

### 提供的使用者動作
- 「看 reasoning」展開該輪 Agent 決策
- 「看來源」點 file:line 跳 side panel
- 「rollback 這筆 KB 新增」在 kb_growth 事件上

---

## 九、失敗處理

| 情況 | 處理 |
|---|---|
| RAG confident 但答案品質低（Agent 覺得 hit 誤導） | Agent 可主動觸發 ReAct loop 補查 |
| Budget 耗盡仍無答案 | 回傳「資訊不足，建議讀 X / Y 檔案」+ 已查過的 source 清單 |
| `add_to_kb` sanitize 後為空（全被替） | 不加、log warning |
| Qdrant 寫入失敗 | log error，答案仍回（KB 未加但不阻斷使用者） |
| 連續多 session KB 快滿 | UI 提示「KB 已 X% 滿，建議清理舊 entry」 |

---

## 十、MVP 不做

| 項 | 延後原因 |
|---|---|
| 跨 session 問題關聯（記憶使用者歷史問題） | Phase 2；MVP 單 session 獨立 |
| 主動 KB 補強（Agent 閒時自己梳理 KB） | Phase 3 |
| 批次 rollback / KB 清理 UI | Phase 2 |
| 多使用者共用 KB（同事共看） | Phase 3 |
| 外部 web 補查（Topic mode 融合） | Phase 2，走 D-002 Topic mode 時機 |
| 多輪 planning（使用者問完整大問題 → Agent 拆成多個 Q） | Phase 2 |

---

## 十一、實作順序（工期估 3-5d）

| 優先 | 項目 | 工期 |
|---|---|---|
| P0 | Q&A system prompt + `kb_search` tool | 0.5d |
| P0 | Q&A main loop（RAG 先行 → 必要時 ReAct） | 0.5d |
| P0 | `add_to_kb` tool（含 sanitize + dedup） | 0.5d |
| P0 | `kb_growth.jsonl` + 成長防呆 | 0.5d |
| P0 | 前端聊天 UI（基本對話氣泡 + SSE 接） | 1.5d |
| P1 | 引用顯示（file:line + side panel） | 0.5d |
| P1 | KB Growth 稽核 UI + rollback | 1d |

**合計 P0 約 3.5d，P0+P1 約 5d。**

---

## 十二、與其他文件的連動更新

實作 / 確認階段要 sync：

- [x] `sanitizer.md` §三觸發點：`add_to_kb` 寫入前必過 sanitize（Pass 3，同 pre-flight 路徑）
- [x] `agent-core.md` §十四：加 `agent/qa.py`
- [x] `agent-explorer-spec.md` §十二 trait 抽象：補「Q&A mode」共用 tool set（§12.6 表 + §12.7 `QATools` / `QASelfCheck`）
- [x] `sidecar-api.md`：新增 `POST /qa` + SSE 事件類型（`rag_hits` / `kb_growth` / `answer_stream`）
- [x] `interactive-tutorial.md`：完成教材後的「繼續問問題」入口連結到 Q&A（§四 `<QAEntry>` 元件）
- [x] `README.md` Module 清單加 Module 8（Q&A Agent）+ Phase 1 功能補
