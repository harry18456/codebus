## Why

D-022（LLM Call Inspector — 全 request/response 稽核，第六層 JSONL + UI 分頁）把「LLM 呼叫透明度」列為 demo 三大武器之一（與 R-01 / O-05 同列 Trust Layer 敘事核心）。對應 `docs/implementation-plan.md` §二第六階段步驟 28.5「LLM Calls 分頁」，也是 D-008（First-run UX 三個等待點）「Exploring 階段把 LLM 黑盒打開給使用者看」的延伸——`agent-console-p0` 已通電 timeline，但 LLM call 細節（完整 wire payload / 回應 / latency / cost）還只在 `llm_calls.jsonl` 落盤，前端沒入口。

Backend 完全就位：`LLMCallLogger` 已寫 `<ws>/.codebus/llm_calls.jsonl` 完整 entry（`timestamp` / `role` / `provider_id` / `model` / `prompt_tokens` / `completion_tokens` / `sanitizer_pass2_applied` / `request` / `response`），且 `explorer-sse` 已 emit `llm_call` SSE event 帶 200-char preview。本 change 補上前端入口：AuditPanel `llm` tab 餵 list、點 row 開 drawer overlay 看 detail。

## What Changes

- 新增 Tauri IPC command `read_audit_jsonl(workspace_root, audit_kind)` — 讀 `<ws>/.codebus/<file>.jsonl` 並回 parsed JSON entries 陣列，採 `interactive-tutorial` Tauri command 同款 `validate_path` 紅線（絕對 ws_root + `.codebus/` 前綴 + 副檔名 allowlist + 紅隊 `..` / symlink / Windows ADS / 長路徑）。`audit_kind` 是 enum `"sanitize" | "tool" | "reasoning" | "token" | "llm" | "kb_growth" | "generator"`，限定七層。本 P0 change 只實際使用 `"llm"`，但 enum 設計成可擴充給後續 28.6 / 30 / 29 共用。
- 新增 Nuxt page route `web/app/pages/audit/llm.vue` — list view 展示 `llm_calls.jsonl` rows（按 timestamp desc），含 filter chips（role: `reasoning` / `judge` / `chat` / `embed` / `pii_detection`、module: `kb_build` / `kb_query` / `reasoning` / `judge` / `chat` / `coverage` / `generate` / `qa_agent`）。row click → 開 drawer overlay。
- 新增 `web/app/components/audit/LlmCallInspector.vue` drawer overlay 元件 — mockup `13-llm-call-inspector.html` `<aside class="insp">` 對應；內含 4 tab：**Wire payload**（post-sanitize `request.messages` + sanitizer 旗標 badge）/ **Response**（response 物件 pretty）/ **Tokens & cost**（prompt/completion/total + cost_usd + latency_ms）/ **Timeline**（單筆 call 的 module + step + role 標）。drawer header 有 prev/next 翻頁（`N / total`）。
- 新增 `useAuditJsonl` composable — Tauri command 的 typed wrapper，`useAuditJsonl(workspace_root, 'llm')` 回 `Ref<LlmCallEntry[]>`；同時 watch `useExplorerStream(task_id).status` 等 explorer 在跑時把新到的 `llm_call` SSE event append 進來（live-tail 模式），不在跑時純讀 disk。
- 既有 AuditPanel `llm` tab 接通：`web/app/pages/explorer/[task_id].vue` 把 `useAuditJsonl(ws, 'llm')` 的 rows 餵給 AuditPanel；點 row 也彈 `LlmCallInspector` overlay（與獨立 page 共用同一個 overlay 元件）。
- `frontend-shell` capability 新增 Requirement：AuditPanel `llm` tab 點 row 必須觸發 `select-row` emit（沿用既有 `select-tab` 模式），父層綁 `LlmCallInspector` overlay 開合。
- 文件：`docs/decisions.md` D-022 後續清單把「LLM Calls 分頁」打勾、補本 change 連結；`docs/implementation-plan.md` §二第六階段步驟 28.5 加註 ✅ landed；`docs/sidecar-api.md` 不動（無新 endpoint）。

## Non-Goals

- **Pre-sanitize / post-sanitize diff（mockup 13 line 374-411 的 `.diff` 區塊）** — D-015 / D-022 鎖死「LLM 看到的一定是 sanitize 過的、原值不儲存」；要顯示 pre-sanitize 必須走 audit-unlock + session-only re-read disk 機制，是獨立的橫切 capability（同樣支撐 28.6 O-05 Sanitizer Diff），P0 不做。Wire payload tab 只渲染 post-sanitize（`llm_calls.jsonl` 既有內容），mockup 13 的 left diff column 在本 P0 直接不渲染、Pass 2 sanitize ON badge 仍顯示。
- **Slider replay / decision log replay 跨 step 動畫** — `agent-explorer-spec.md §九` P2，本 change 不做。
- **Filter persistence（記住使用者選的 role / module chip）** — 用 in-memory ref，不寫 progress.json，重整即重置；Phase 2 視需求再加。
- **多 workspace 跨 ws 比對** — Inspector 綁單 ws，不支援多 ws aggregation。Phase 2 Topic mode 再評估。
- **Cost aggregation 圖表（pie / bar）** — `usage_delta` SSE event 帶 `session_total_cost_usd`，已可看；inspector P0 只列單筆 cost，不做總覽圖。
- **Server-side filter / pagination** — `llm_calls.jsonl` 預期 < 1MB（demo 級），全載入 + client-side filter；超過 5MB 時 Tauri command 回 `E_AUDIT_TOO_LARGE` 錯誤、UI 顯示「audit too large for inline view」。
- **Edit / delete entries** — JSONL 是 append-only audit；UI 純 read。
- **Export / share entry**（複製為 cURL / 下載 JSON）— P0 沒有；右鍵複製 raw JSON 該夠 demo。

## Capabilities

### New Capabilities

- `llm-call-inspector`: 前端 LLM call 稽核入口 — 由 Tauri IPC command `read_audit_jsonl` 讀 `<ws>/.codebus/llm_calls.jsonl`，餵 AuditPanel `llm` tab list + page route `/audit/llm` 同款 list；點 row 開 `LlmCallInspector` drawer overlay 看 4 tab detail。同時 watch explorer SSE `llm_call` event 做 live-tail。

### Modified Capabilities

- `frontend-shell`: AuditPanel 新增 `select-row` emit（既有 `select-tab` emit 並列），讓父層綁定 row click → 開 inspector overlay；既有「七 tab 順序」「empty state」「rows 經 prop 注入」三條 Requirement 不變。

## Impact

- Affected specs:
  - New: openspec/specs/llm-call-inspector/spec.md
  - Modified: openspec/specs/frontend-shell/spec.md（AuditPanel `select-row` emit Requirement）
- Affected code:
  - New:
    - tauri/src-tauri/src/audit_files.rs（新 Tauri IPC module；`validate_audit_path` + `read_audit_jsonl` command）
    - tauri/src-tauri/tests/audit_path_safety.rs（紅隊 path-escape 測試，仿 tutorial 紅隊 14 case 模板）
    - web/app/composables/useAuditJsonl.ts
    - web/app/pages/audit/llm.vue
    - web/app/components/audit/LlmCallInspector.vue
    - web/tests/audit/useAuditJsonl.spec.ts
    - web/tests/audit/LlmCallInspector.spec.ts
    - web/tests/audit/llm-page.spec.ts
    - web/tests/audit/fixtures/llm-calls.json（vitest fixture，仿 `agent-console-p0` 套路）
  - Modified:
    - tauri/src-tauri/src/lib.rs（註冊 `audit_files::read_audit_jsonl` command）
    - tauri/src-tauri/Cargo.toml（無新 deps；audit_files 重用 dunce + serde + tokio）
    - web/app/components/audit/AuditPanel.vue（新增 `select-row` emit；不改既有七 tab 排序與 props）
    - web/app/pages/explorer/[task_id].vue（接通 `useAuditJsonl(ws, 'llm')` 餵 AuditPanel + 監聽 select-row 開 inspector overlay）
    - docs/decisions.md（D-022 後續清單打勾）
    - docs/implementation-plan.md（步驟 28.5 加註 ✅ landed）
- Affected runtime contracts:
  - 不改 `llm_calls.jsonl` schema（純消費端）
  - 不改 `explorer-sse` `llm_call` event schema
  - Tauri IPC 表面新增一支 command；Tauri commands 既有錯誤詞彙（`E_INVALID_PATH` / `E_WORKSPACE_INVALID` / `E_NOT_FOUND` / `E_DENIED` / `E_NOT_REGULAR_FILE` / `E_IO`）擴充 `E_AUDIT_KIND_INVALID` + `E_AUDIT_TOO_LARGE`
- Affected dependencies:
  - 不引新 npm 套件（vitest infra 已在 `agent-console-p0` archive）
  - 不引新 Rust crate（dunce / serde / tokio 既有）
