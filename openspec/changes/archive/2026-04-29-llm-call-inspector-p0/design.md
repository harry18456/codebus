## Context

`llm_calls.jsonl` 是 D-022 鎖定的第六層稽核 JSONL，每筆 LLM call（chat / embed / pii_detection）寫一行完整 entry：`timestamp` / `role` / `provider_id` / `model` / `prompt_tokens` / `completion_tokens` / `sanitizer_pass2_applied` / `request`（dict，含 `messages: [{role, content}, ...]`）/ `response`（dict 或 null）。寫入端 `LLMCallLogger`（`sidecar/src/codebus_agent/providers/llm_call_logger.py`）已通電，且當 `emitter` wired 時並行 emit `llm_call` SSE event 帶 200-char preview（schema 見 `openspec/specs/explorer-sse/spec.md` Requirement「LLMCallLogger emits llm_call event carrying preview」）。

前端 `agent-console-p0` archive（2026-04-29）已落地 `pages/explorer/[task_id].vue` + 七 tab AuditPanel，但 `llm` tab 尚未接通—目前僅是 placeholder empty state。Mockup `design/v1/13-llm-call-inspector.html` 描繪 drawer overlay 詳情 view，含 4 tab（Wire payload / Response / Tokens & cost / Timeline）+ pre/post sanitize diff（line 374-411）+ status strip（role / module / model / sanitize ON / response code / latency / cost）+ prev/next 翻頁。

D-008 把 Trust Layer 四站定為敘事核心（R-01 / O-01 / O-04 / O-05）；O-04 = LLM Call Inspector。本 change 對應步驟 28.5、是 O-04 站的 P0 落地。

## Goals / Non-Goals

**Goals:**

- 提供 list + detail 完整流程，讓使用者可走訪每筆 LLM call 完整 wire payload + response + 計帳
- 建立 `read_audit_jsonl` IPC + `useAuditJsonl` composable 兩塊**橫切 infra**，後續 28.6 Sanitizer Diff / 30 Q&A kb_growth tab / 29 介入點都可 reuse；`audit_kind` enum 預先包含七層（不單為 llm 寫死）
- AuditPanel `llm` tab 與 `/audit/llm` page 共用同一個 inspector overlay 元件（單一 truth，避免兩處 detail 漂移）
- Live-tail：當 `/explorer/[task_id]` 在跑時，新到的 `llm_call` SSE event 即時 append 進 list（與 disk 既有 entries 自然合流）

**Non-Goals:**

- Pre-sanitize / post-sanitize diff（mockup 13 line 374-411）— defer 到獨立 `audit-unlock` capability（同樣支撐 28.6）
- LLM call replay / re-run（用同 prompt 重打）— 違反 audit append-only 原則
- 跨 ws / 跨 task aggregation 圖表
- Filter persistence、CSV export、cURL 複製 — Phase 2

## Decisions

### Tauri IPC 採單一 `read_audit_jsonl` command + `audit_kind` enum，而非每 kind 一支 command

**Approach**：一支 `read_audit_jsonl(workspace_root, audit_kind)` command，`audit_kind` 是 enum `"sanitize" | "tool" | "reasoning" | "token" | "llm" | "kb_growth" | "generator"`。後端 match enum → 拼路徑 `<ws>/.codebus/{filename}.jsonl`、過 `validate_audit_path` → 串流讀檔、parse JSONL、回 `Vec<serde_json::Value>`。

**Alternatives considered**：
- 七支獨立 command（`read_sanitize_audit` / `read_tool_audit` / ...）—— rejected：Tauri command surface 暴漲、code dup、註冊清單變長；single dispatch 後端只多一個 match。
- 完全 generic `read_workspace_file(workspace_root, relative_path)` —— rejected：失去 enum 約束，前端可亂傳 path；audit 是固定七檔，enum-bound 才能擋掉「假裝 audit 但實際讀任意檔」攻擊。

### Audit JSONL path validation 與 tutorial path 分開、不 reuse `tutorial::validate_path`

**Approach**：寫新的 `audit_files::validate_audit_path(workspace_root, audit_kind)`，要求 `<ws>/.codebus/<filename>.jsonl`，prefix 是 `.codebus/`（非 `codebus-tutorials/`）；副檔名只接 `.jsonl`；segment 紅線同 tutorial 那套（`..` / `.` / Windows ADS / 結尾 `.` 或空格 / Windows reserved name）。

**Alternatives considered**：
- 把 `validate_path` 抽 `(workspace_root, prefix, allowed_extensions)` 通用版 —— rejected：兩個 trust boundary 安全屬性不同（tutorial 允許 user 編輯 progress.json、audit 嚴格 read-only）；通用化後 prefix / extension 由 caller 傳，多一道誤用風險。Copy-paste validate 的 6 條紅線換 prefix 與 extension 是 5 行差異，cost 低於抽象風險。
- Reuse tutorial 的 `validate_path` 直接傳 `.codebus/foo.jsonl` 當 relative_path —— rejected：函數內硬寫 `prefix_with_slash = "{TUTORIALS_SUBDIR}/"`；不可能 reuse 而不重構。

### Live-tail 採 `useExplorerStream` 既有 SSE 不再開新通道

**Approach**：`useAuditJsonl(ws, 'llm', { task_id?: string })` 第三參數 optional；若帶 task_id，內部不開新 EventSource，而是讓 caller 自己構 `useExplorerStream(task_id)` 並把 stream 的 `llm_call` event 透過 callback / event bus push 進 audit list。Page `/explorer/[task_id]` 已有 stream 實例，只需多接一條 watch。

**Alternatives considered**：
- `useAuditJsonl` 自開 SSE — rejected：違反「page 持有 stream 唯一性」原則（`agent-console-p0` 設計決策「`useExplorerStream` 是唯一的 SSE 事件分派入口」）；多開一條會雙倍 explorer SSE 連線。
- 純 disk re-poll（每 2s 重讀） — rejected：append-only JSONL 雖然可 inotify / file size diff，但跨平台不一致 + race condition；既有 SSE 已是即時 push，沒理由再加 polling。

### `LlmCallInspector` overlay 與 `useAuditJsonl` 行為分離

**Approach**：`useAuditJsonl` 只負責 list 載入 + live-tail merge；overlay 元件接受 `:rows` + `:active-index` props 純展示，prev/next 翻頁透過 `@select-index` emit 回 caller。Page `/audit/llm.vue` 與 `pages/explorer/[task_id].vue` 兩處各持一個 `selectedIndex: ref<number | null>`，inspector 開合與內容兩處同款邏輯（也可後續 hoist 成 `useAuditInspector` composable 若再被第三處用到）。

**Alternatives considered**：
- Overlay 自己內建 navigation state + 直吃 composable — rejected：兩處 caller（`/audit/llm` page + explorer page）使用情境不同（前者 standalone navigate、後者疊在 console 旁），耦合進 overlay 後彈性差。

### Page `/audit/llm` 與 AuditPanel `llm` tab 採「**逐位置獨立 inspector**」而非「共享 navigation state」

**Approach**：`/audit/llm` 是 standalone full page（左 list + 右 inspector overlay 蓋在 list 上）；explorer page 的 AuditPanel 點 row 也彈 overlay，但兩處 selected index 互不同步（無 URL 表達）。

**Alternatives considered**：
- 用 query param 同步 `?row=N` — rejected：兩處情境不同（一個是 audit-explore mode、一個是 in-context inspect），用 URL 同步反而讓「在 explorer 看 console 順手點 LLM call」這個 flow 多一道意外的 page reload risk。
- 用 Pinia store 同步 — rejected：引入 Pinia 為一個 state 開銷過大；目前專案無 Pinia，加進來等於多一個依賴。

### `live-tail` 與 disk read 合流策略：append-only by timestamp

**Approach**：`useAuditJsonl` 內部 `entries: ref<LlmCallEntry[]>` 先載入 disk（按 timestamp asc），新到的 SSE `llm_call` 直接 push 末尾；UI 顯示前 `.slice().reverse()` 變 desc。每筆 entry 帶 `request_id` 作 dedup key（disk 與 SSE 都帶，避免重複；SSE 早到 disk 晚 flush 時也防雙列）。

**Alternatives considered**：
- 收到 SSE 後重讀整個 disk — rejected：浪費 IO 且導致 list 短暫閃爍；append 比 re-read 自然。
- 完全不 live-tail，disk-only — rejected：demo 看 explorer 跑時 LLM call 進來不刷新會 weird。

### `E_AUDIT_TOO_LARGE` cap 訂在 5 MB

**Approach**：`read_audit_jsonl` 在開檔時先 `stat().len()`，超過 `5 * 1024 * 1024` bytes 直接 return `Err("E_AUDIT_TOO_LARGE")`。前端 catch → 顯示「audit too large for inline view（{size} MB）」+ Phase 2 streaming 入口。

**Alternatives considered**：
- 串流 read + window 100 條 — rejected：UI 端 filter / sort / 翻頁複雜度大幅升高，超 P0 範圍。
- 不設 cap — rejected：Tauri main thread 讀 50MB JSON parse 會凍 frame，使用者體驗差於 explicit error。

### Vitest fixture 採同 `agent-console-p0` 套路

**Approach**：`web/tests/audit/fixtures/llm-calls.json` 是 JSON array，每元素是一條 `LlmCallEntry`；`useAuditJsonl` 測試 mock `Tauri::invoke('read_audit_jsonl', ...)` 回該 fixture；`LlmCallInspector` 測試直接 `mount` 並 prop-feed entry。

**Alternatives considered**：
- JSONL 字串 fixture + 測試裡 split — rejected：vitest import JSON 比 split 容易，且與 `agent-console-p0` 既有 fixture 風格一致。

## Risks / Trade-offs

- **[Risk] `LlmCallInspector` 沒有 pre-sanitize diff，demo 火力降低**：mockup 13 主秀就是 left/right diff，少了會弱化「sanitize 透明度」訊號。Mitigation：Wire payload tab 顯眼放 `sanitizer_pass2_applied: true` badge + tooltip「pre-sanitize 原值未儲存以滿足 D-015」；後續 `audit-unlock` capability 落地後 pre-sanitize 自然加回。
- **[Risk] Live-tail 與 disk 雙寫導致 race**：sidecar `LLMCallLogger.log()` 先 disk write、再 SSE emit；理論 disk flush 完才 SSE 不會 race，但 OS-level disk cache 不保證 sync。Mitigation：以 `request_id` 為 dedup key，merge 時 ignore 既有 request_id。
- **[Risk] `request` / `response` payload 含 sanitize-過的 placeholder 但 placeholder 數值意外洩漏 metadata**：placeholder format `<REDACTED:secret#1>` 帶 `kind` 與 index，若 kind 名直接命中業務領域字（如 `<REDACTED:internal-domain#3>`）對攻擊者仍透露「這 repo 有內部 domain」此事實。MVP 接受—is by design（D-015 認可「placeholder 表達 kind 是必要的，便於 audit / debug」），不額外脫敏 metadata。
- **[Trade-off] `audit_kind` enum 寫死七層、未來若加第八層稽核必須 bump**：enum 是強約束、不能 user-defined。CLAUDE.md `七層 Audit JSONL` 是 invariant；若真有第八層，那本身就是橫跨多 spec 的大改，順便 bump enum 不算負擔。
- **[Trade-off] `validate_audit_path` 與 `tutorial::validate_path` 重複代碼 ~70%**：選 copy 而非通用化的 cost，是審計安全邊界的 noise；接受。

## Open Questions

- AuditPanel `select-row` emit 的 payload 形狀：`(rowIndex: number)` 還是 `(row: AuditRow)`？傾向 index（簡單 + caller 可從 props.rows 取），但 row 可避免 caller 多 lookup。實作時定。
- `useAuditJsonl` 若是被 explorer page + `/audit/llm` page 同時 mount（左右兩個 tab 都打開），第二個 instance 是否 share state？暫定不 share（每個 caller 獨立 ref + 各自讀檔），效能 acceptable in P0；Phase 2 看數據再評估 module-level singleton。
