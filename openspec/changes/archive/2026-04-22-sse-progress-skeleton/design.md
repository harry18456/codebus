## Context

`docs/sidecar-api.md §四` 已經把 SSE event schema 與 `GET /tasks/{id}/events` 端點規格定下來，`§七` 規定 sidecar 同時只跑一個 task（single FIFO queue）。本 change 要把這個契約落到實作，而 Module 1 / 2 兩個生產者狀態不對稱：

- **Module 2 KB Builder** 已有 `KBProgressEvent` + `ProgressCallback` Protocol（`module-2-kb-builder-p0` 落地），`KnowledgeBase.build()` 在 chunking / embedding / upserting / done 四 phase emit；但**沒有 HTTP 入口**——`POST /kb/build` 不存在。
- **Module 1 Scanner** `POST /scan` 已上線且**同步**回傳 `ScanResult`；服務層**沒有任何 progress callback hook**，需 retrofit。

關聯約束：

- bearer middleware + `127.0.0.1` 綁定既定，新端點不能繞過（`sidecar-runtime`）。
- 雙模 discriminator（`workspace_type: "folder" | "topic"`）從 schema day 1 起算，task store / event 不該以 `workspace_type` 做型別分支（D-002）。
- 既有 `POST /scan` 同步契約（`scanner-skeleton` archived 在 2026-04-21）**不能 break**——只能新增 opt-in `?stream=true`，而非預設改 async。
- 後續 Agent / Q&A change（implementation-plan 步驟 22+）會在同一個 `GET /tasks/{id}/events` 通道擴增 `agent_thought` / `judge_verdict` / `rag_hits` / `kb_growth` / `usage_delta` / `usage_summary` / `llm_call` 等 event type，本 change 設計的 task store + event channel 必須是 event-type-agnostic 的。

## Goals / Non-Goals

**Goals:**

- 把 spec §四 的 `progress` / `done` / `error` 三類 event 在 sidecar 端落實成可訂閱的 SSE stream。
- Module 1 Scanner / Module 2 KB Builder 透過同一個 task store + event channel 抽象 emit 進度。
- 為後續 Agent / Q&A change 預埋 event-type-agnostic 的 task / event 基礎設施，不被本 change 鎖死。
- 維持既有 `POST /scan` 同步契約，新增 opt-in async 模式不破壞現有 client。

**Non-Goals:**

- 多 task 並行 / task pool / scheduler。
- task 持久化、跨 sidecar restart 還原、`Last-Event-ID` reconnect。
- task 取消端點 `DELETE /tasks/{id}`。
- Agent / Q&A 專屬 event type（屬步驟 22+）。
- 前端 / Tauri UI 訂閱（屬 Module 7）。
- 修 `docs/sidecar-api.md §四` 既有 event schema。

## Decisions

### Single-slot task store over dict-based pool

採 `class TaskRegistry` 持單一 `Optional[TaskHandle]` 而非 `Dict[task_id, TaskHandle]`。新請求進來時若 `current is not None and current.status == "running"` → 端點回 `409 Conflict`、body `{"code":"TASK_IN_FLIGHT","running_task_id":"..."}`。task 跑完（done / error）後 handle **保留**（讓 `GET /tasks/{id}/result` 拿得到終局）直到下一個 task 寫入時被覆蓋。

**Why over dict pool**：spec §七 single FIFO 既定，dict 會引誘「順手實作多 task」造成資源失控（embed 是 IO + LLM token bill）。single-slot 也讓 task lifecycle 推理變單純：只有「沒有 / running / 結束」三狀態。

**Alternative considered**：`Dict[task_id, TaskHandle]` + max-size 1。被拒——額外 size check 是雜訊；single-slot 用 `Optional` 表達意圖更直接。

### task_id 用前綴 + 8 字 hex random

格式 `{kind}_{rand8}`，例：`scan_abc12345` / `kb_xyz98765`。`kind ∈ {"scan", "kb"}` 由產生端點決定；`rand8` 從 `secrets.token_hex(4)` 取 8 字 hex。

**Why**：spec §四 範例用 `scan_abc123` / `kb_xyz789` / `explore_def456`，前綴讓人讀 log 一眼分類。`secrets.token_hex(4)`（32-bit entropy）對 single-slot store 完全夠（同一時間頂多一個 active），且不需 UUID 套件。

**Alternative considered**：純 UUID4。被拒——對 single-slot store 太重；前綴對 log 與 audit 更友善。

### `asyncio.Queue` 作 event channel；每位訂閱者自帶 `asyncio.Queue` 副本

`TaskHandle` 持一個 `list[asyncio.Queue[dict]]`。emit 時 fan-out 寫入所有 subscriber queue。SSE endpoint 連線時：`queue = asyncio.Queue(); handle.subscribers.append(queue)`，斷線時從 list 移除。

**Why**：spec §四 雖然只規定一個訂閱者（前端），但 `claude-in-chrome` debug、CLI tail、未來多 view（main UI + audit panel）都可能多訂閱。fan-out 成本（list copy）對 progress event 量級（每 task 數十至數百筆）可忽略，但帶來「不同訂閱者進度不互相影響」的乾淨語意。

**Alternative considered**：`asyncio.Event` + 共享 buffer。被拒——多訂閱者語意難寫對；且斷線重連的 buffer 重發語意（Last-Event-ID）本 change 已 non-goal。

### 背景 task 用 `asyncio.create_task` + `app.state` 引用

不引入 Celery / RQ / arq 之類 task queue。`POST /scan?stream=true` 與 `POST /kb/build` 端點：
1. 同步建立 `TaskHandle`、寫入 `app.state.tasks`
2. `asyncio.create_task(_run_scan(handle, body))` 啟背景 coroutine
3. 立即回 `{"task_id": handle.id}`（200 OK）
4. 背景 coroutine 內呼叫 `service.scan(on_progress=handle.emit)` 或 `kb.build(on_progress=handle.emit)`，全程把 exception 攔成 `error` event 送進 channel，最後 emit `done` 並寫 `handle.result`

**Why**：sidecar 是 single-process FastAPI，`asyncio.create_task` 已足夠；引入 task queue 會破壞 D-001「sidecar 是輕量子程序」的定位、增打包複雜度。

**Risk**：sidecar crash → 跑一半的 task 連同 result 一起遺失。本 change 接受此風險（Non-Goal「task 持久化」），但 error 路徑必須 `finally` 收斂讓 SSE 訂閱者收到 `error` event 而非永久 hang。

### Module 1 / 2 phase 名稱對應

spec §四 規定 `phase ∈ scanning / embedding / exploring / generating`。本 change 翻譯如下：

| Module | 來源 phase                                            | spec §四 phase | 觸發點                                |
| ------ | ----------------------------------------------------- | -------------- | ------------------------------------- |
| 1      | walk                                                  | `scanning`     | 每 N=50 個檔案 emit 一次              |
| 1      | sanitizer Pass 1                                      | `scanning`     | 每 N=50 個檔案 emit 一次              |
| 2      | chunking / embedding / upserting / done（KB internal）| `embedding`    | KB 已有 callback，全部翻成 `embedding` |

**Why 把 KB 三 phase 都翻成 `embedding`**：spec §四 `phase` 列舉只有四值（per-module 概念），不細分 sub-phase。前端只需要「KB 階段在跑、進度 X/Y」即可；如果未來要分 sub-phase，加 `current_file` 或新欄位（`sub_phase`）即可，不破壞既有 schema。

**Alternative considered**：擴 spec §四 加 `sub_phase`。被拒——本 change non-goal「修 spec §四 既有 event schema」；且前端 P0 進度條不需要這個資訊。

**Risk**：KB 內 `chunking` 階段沒有 batch 數量、emit 出去的 `current/total` 會跟 `embedding` 階段不連續。Mitigation：KB internal 翻譯時 `chunking` 與 `upserting` 各 emit 一筆 `progress`（current=0/total=total_chunks 與 current=total_chunks/total=total_chunks），讓前端進度條從 0% 跳到 100% 的中間有 embedding 階段填滿；不引入跨 phase 的「總進度」概念。

### `POST /scan?stream=true` opt-in，不預設改 async

`POST /scan`（無 query string）：維持 M1 同步行為（`scanner-skeleton` 既有契約）。
`POST /scan?stream=true`：切到 async path，回 `{"task_id": "scan_..."}`。

**Why**：M1 archived spec 已上線，client（測試 + 未來 Tauri 殼）可能依賴同步回傳。opt-in 讓既有 client 無痛延續，新功能只對主動帶 query 的 client 生效。

**Alternative considered**：用 `Accept: text/event-stream` header 自動切換。被拒——header 隱藏意圖、難 grep；query string 顯式且 OpenAPI doc 容易標。

### `POST /kb/build` 預設 async（無同步路徑）

`POST /kb/build` body `{workspace_root, scan_result_id?}` 直接回 `{"task_id": "kb_..."}`，**沒有同步版本**。

**Why**：KB build 涉及 LLM embed（即使是 mock），latency 量級遠高於 scan；同步等待沒意義。client 拿到 `task_id` 後 SSE 訂閱比 long-polling 簡單。

**Risk**：scan_result 怎麼餵給 `/kb/build`？兩個選項：
1. **inline payload**：`{workspace_root, scan_result: {...完整 JSON...}}` — 簡單、無耦合，但 large repo 的 scan_result 可能上 MB。
2. **task_id 串接**：先 scan 拿 `scan_task_id`，再 `/kb/build` body `{scan_task_id}`，server 端從 task store 撈 result — 但 single-slot store 在 scan done 後若有新 task 進來會被覆蓋。

**Decision**：本 change 採 **inline payload**（option 1），simple wins。large repo 後續優化（option 2 + multi-slot result cache）屬未來 change。

### error event 安全性

背景 task 內 `try/except Exception` 攔截後，emit `{"type":"error", "code":"<sanitized>", "message":"<sanitized>"}`，**不**直接把 `repr(exc)` 或 `traceback` 送進 SSE。`code` 從預定義表挑（`SCAN_FAILED` / `KB_EMBED_FAILED` / `INTERNAL_ERROR`），`message` 是人讀的安全字串。完整 stack 寫進 sidecar logger（不流到 client）。

**Why**：spec §五 §四「不 log token 或完整 prompt 到 stdout/stderr」+「無 token 或錯 token → 401，不揭露 sidecar 內部」既有約束的延伸；error event 是新通道但同等敏感。

## Risks / Trade-offs

- **Single-slot store 限制**：[兩個 client 同時想 scan + build 會被擋] → spec §七 既定，前端需序列化 UI 操作；client error message 標明 `running_task_id` 讓使用者知道誰在跑。
- **背景 task 失火連帶 SSE hang**：[`asyncio.create_task` 例外若沒 await 會被吞] → 在 `_run_scan` / `_run_kb` 外層用 `try/except/finally`，`finally` 一定 emit `done` 或 `error` 並 close subscribers。
- **inline scan_result payload 大小**：[大 repo 上 MB 跨 HTTP 兩次] → 接受，本 change 範圍內無更好解；未來改 multi-slot result cache 時對齊。
- **subscriber list 不上鎖 + asyncio 單執行緒**：[多訂閱者同時 emit + 訂閱可能 race] → asyncio 單執行緒、無 await 點時 list 操作 atomic；emit / subscribe / unsubscribe 都不 await，安全。
- **KB phase 翻譯損失資訊**：[chunking / upserting 細節不傳給前端] → 接受；未來如需細分用 `current_file` 或新欄位增量加，schema 向前相容。
- **task_id 8-hex 碰撞**：[`token_hex(4)` 32-bit space] → single-slot store 同時最多一個 active，舊 task done 後即使碰撞也只是 result lookup 拿到舊的 → 風險可忽略；若未來多 slot 改 16-hex。
- **沒實作 `Last-Event-ID` reconnect**：[client 斷線重連會錯過事件] → 接受 non-goal；UI 端需在斷線時提示使用者「需重新開始」，本 change 不負責 UI 行為。
