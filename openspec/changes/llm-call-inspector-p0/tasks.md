## 1. Tauri IPC `read_audit_jsonl` command（先做、後端 infra；對應 spec Requirement「`read_audit_jsonl` Tauri command exposes seven workspace audit JSONLs by enum」+「Audit kind filename mapping defensive parity」+ design 決策「Tauri IPC 採單一 `read_audit_jsonl` command + `audit_kind` enum，而非每 kind 一支 command」+「Audit JSONL path validation 與 tutorial path 分開、不 reuse `tutorial::validate_path`」）

- [x] 1.1 寫 `tauri/src-tauri/src/audit_files.rs`：宣告 `_AUDIT_KIND_TO_FILENAME: &[(&str, &str)]` 七層 pair（順序對齊 spec「Audit kind filename mapping defensive parity」表格）+ `validate_audit_path` helper（六條紅線：absolute ws_root / `.codebus/` 前綴 / 無 `..` `.` / 無 Windows ADS / 無 reserved name / 副檔名 = `.jsonl` / canonical containment 含 symlink resolution）— 落實 design 決策「Audit JSONL path validation 與 tutorial path 分開」
- [x] 1.2 寫 `read_audit_jsonl(workspace_root, audit_kind)` async command：dispatch enum → 拼路徑 → `validate_audit_path` → `metadata().len()` cap 5MB → tokio fs read → split `\n` → `serde_json::from_str` per line（corrupt line `log::warn!` skip）→ 回 `Result<Vec<serde_json::Value>, String>`；落實 design 決策「`E_AUDIT_TOO_LARGE` cap 訂在 5 MB」
- [x] 1.3 [P] 寫 `tauri/src-tauri/tests/audit_path_safety.rs`：覆蓋 spec「`read_audit_jsonl` Tauri command exposes seven workspace audit JSONLs by enum」全部 7 個 scenario（valid llm kind / unknown audit_kind / missing file → empty vec / `..` 拒絕 / symlink escape 拒絕 / 5MB cap / corrupt line skip）+ tutorial 紅隊 14 case 同款 path-escape fixtures（dotdot / `.` / Windows ADS `:` / reserved name `con.jsonl` / 結尾 `.` 或空格 / dunce canonical UNC `\\?\` long-path）
- [x] 1.4 寫 `tauri/src-tauri/tests/audit_kind_filename_parity.rs`：source-grep `sidecar/src/codebus_agent/_audit_paths.py` `_<NAME>_FILENAME` 七常數值 → 與 Rust `_AUDIT_KIND_TO_FILENAME` 七 pair filename 比對 → 任一不符即 fail；對應 spec「Audit kind filename mapping defensive parity」全部 2 個 scenario
- [x] 1.5 在 `tauri/src-tauri/src/lib.rs::invoke_handler!` 註冊 `audit_files::read_audit_jsonl`；確認 `cargo test --tests audit_path_safety audit_kind_filename_parity` 全綠
- [x] 1.6 `cd tauri/src-tauri && cargo build` 全綠（無 warnings · clippy clean）

## 2. `useAuditJsonl` composable（測試先行；對應 spec Requirement「`useAuditJsonl` composable wraps the Tauri command with optional live-tail」+ design 決策「Live-tail 採 `useExplorerStream` 既有 SSE 不再開新通道」+「`live-tail` 與 disk read 合流策略：append-only by timestamp」+「Vitest fixture 採同 `agent-console-p0` 套路」）

- [x] 2.1 [P] 寫 `web/tests/audit/fixtures/llm-calls.json`：JSON array 至少 5 條 LlmCallEntry，覆蓋多 role（reasoning / judge / chat）/ 多 module / `sanitizer_pass2_applied: true` 與 `false` 並存 / `response: null` 一條 / `cost_usd: null` 一條 / 重複 `request_id` dedup 測試用一對；補 sibling `web/tests/audit/fixtures/README.md` 註記 fixture 對應的 spec scenario（落實 design 決策「Vitest fixture 採同 `agent-console-p0` 套路」）
- [x] 2.2 寫 RED 單元測 `web/tests/audit/useAuditJsonl.spec.ts`：mock `@tauri-apps/api/core::invoke` + 覆蓋 spec「`useAuditJsonl` composable wraps the Tauri command with optional live-tail」全部 5 個 scenario（initial load / live-tail append / live-tail ignore non-llm / dedup by request_id / E_AUDIT_TOO_LARGE error surface）
- [x] 2.3 實作 `web/app/composables/useAuditJsonl.ts`：export `AuditKind` literal union + `LlmCallEntry` typed interface（對齊 `LLMCallLogger._base_entry` schema）+ `UseAuditJsonlApi`；內部 `Tauri.invoke` 一次 + optional watch `liveTailFromExplorerStream` 的 SSE event chain（不開新 EventSource — 落實 design 決策「Live-tail 採 `useExplorerStream` 既有 SSE 不再開新通道」）+ `request_id` dedup（落實 design 決策「`live-tail` 與 disk read 合流策略：append-only by timestamp」）
- [x] 2.4 確認 step 2.2 全綠（`npm run test useAuditJsonl`）

## 3. `LlmCallInspector` overlay 元件（TDD：RED → GREEN；對應 spec Requirement「`LlmCallInspector` overlay renders four tabs and prev/next navigation」+ design 決策「`LlmCallInspector` overlay 與 `useAuditJsonl` 行為分離」）

- [x] 3.1 寫 RED 元件測 `web/tests/audit/LlmCallInspector.spec.ts`：覆蓋 spec「`LlmCallInspector` overlay renders four tabs and prev/next navigation」全部 6 個 scenario（activeIndex null hides / 4 tab canonical order / prev-next clamped / sanitize banner with D-015 / cost null em-dash / Escape key emits close）
- [x] 3.2 實作 `web/app/components/audit/LlmCallInspector.vue`：`<aside>` overlay + header（title / req_id / step_id / timestamp / prev-next + N/total / close）+ status strip（role / module / model badges + Pass 2 sanitize ON purple badge + latency / cost）+ 4 tab switcher + per-tab body；Wire payload tab 顯眼放 D-015 banner（無 pre-sanitize column）；Tokens & cost tab 用 em-dash for null；落實 design 決策「`LlmCallInspector` overlay 與 `useAuditJsonl` 行為分離」（純 props in / emits out）
- [x] 3.3 確認 step 3.1 全綠（`npm run test LlmCallInspector`）

## 4. AuditPanel `select-row` emit（既有元件擴充，TDD；對應 frontend-shell delta spec「AuditPanel surfaces seven workspace-level audit JSONL tabs」MODIFIED Requirement）

- [x] 4.1 寫 RED 元件測 `web/tests/audit/AuditPanel-select-row.spec.ts`：覆蓋 frontend-shell delta spec「AuditPanel surfaces seven workspace-level audit JSONL tabs」新加的 4 個 scenario（select-row payload index / fire for every tab / no internal inspector mount / 不破既有「empty rows show empty state」「No CB_AUDIT_SAMPLES literal」「七 tab order」三個既有 scenario）
- [x] 4.2 改 `web/app/components/audit/AuditPanel.vue`：補 `defineEmits<{(e: 'select-row', index: number): void}>()` + row `<div>` 加 `@click="$emit('select-row', idx)"`；不改既有 `select-tab` emit、不改 7 tab 順序、不改 props shape
- [x] 4.3 確認 step 4.1 全綠 + 既有 `agent-console-p0` archive 的 page 整合測也全綠（`npm run test`）

## 5. `/audit/llm` standalone page（TDD；對應 spec Requirement「`/audit/llm` page surfaces the inspector standalone」+ design 決策「Page `/audit/llm` 與 AuditPanel `llm` tab 採「**逐位置獨立 inspector**」而非「共享 navigation state」」）

- [x] 5.1 寫 RED page 測 `web/tests/audit/llm-page.spec.ts`：覆蓋 spec「`/audit/llm` page surfaces the inspector standalone」全部 4 個 scenario（missing ws_path → no IPC + error / row click opens inspector with correct underlying index / filter chip narrows list + inspector receives filtered subset / empty entries → empty state, no inspector）
- [x] 5.2 實作 `web/app/pages/audit/llm.vue`：query `?ws_path=` 校驗 + `useAuditJsonl(ws_path, 'llm')` + 左 list（timestamp desc 顯示，但 caller 端維持 underlying index → display-to-underlying 翻譯函數）+ filter chips（role × 5 + module × 8）+ row click → 開 `<LlmCallInspector>` overlay + loading / empty / error 三狀態
- [x] 5.3 確認 step 5.1 全綠

## 6. Explorer page reuse inspector（TDD 整合；對應 spec Requirement「Explorer console page reuses the same inspector overlay」）

- [x] 6.1 寫 RED page 整合測 `web/tests/audit/explorer-page-llm-tab.spec.ts`：覆蓋 spec「Explorer console page reuses the same inspector overlay」全部 3 個 scenario（llm tab 接 useAuditJsonl 含 live-tail / row click 開同一個 inspector overlay / missing ws_path 時 SSE 仍開但 llm tab 顯 ws_path required fallback）
- [x] 6.2 改 `web/app/pages/explorer/[task_id].vue`：讀 `?ws_path=` query；`useAuditJsonl(ws_path, 'llm', { liveTailFromExplorerStream: stream })`；`activeTab === 'llm'` 時餵這份 audit list 給 AuditPanel；綁 `select-row` → 開 `<LlmCallInspector>` overlay；ws_path 缺時 llm tab 顯 fallback、其他 tab 不影響
- [x] 6.3 確認 step 6.1 全綠 + `agent-console-p0` archive 的既有 5 個 explorer-page scenario 仍全綠（regression）

## 7. 文件同步

- [x] 7.1 `docs/decisions.md` D-022 後續清單（搜尋「LLM Calls 分頁」）：把對應 `[ ]` 改 `[x]`，補本 change 名稱與 archive 落地日期 placeholder
- [x] 7.2 `docs/implementation-plan.md` §二第六階段步驟 28.5 加註「✅ landed `llm-call-inspector-p0`」（與步驟 26 / 26.5 / 27 / 28 同款格式），不改工期表
- [x] 7.3 `CLAUDE.md` 「## 子系統」段 `web/` 子段補一句 composable / page 對應（同 `agent-console-p0` 落地後的 `useExplorerStream` 補錄方式）

## 8. 整合驗證

- [x] 8.1 `cd web && npm run typecheck` 全綠（`AuditKind` literal union / `LlmCallEntry` interface 完整、無 `any` 殘留）
- [x] 8.2 `cd web && npm run test` 全綠（12 files / 68/68 tests，本 change 新增 5 份 + `agent-console-p0` archive 既有 7 份；舊既測 0 regression）
- [x] 8.3 `cd tauri/src-tauri && cargo test` 全綠（audit_path_safety 12 case + audit_kind_filename_parity 2 case 兩份新測 + tutorial 既有 19 case 紅隊；總 33 case）
- [~] 8.4 **defer 至 Phase 7 demo prep**：手動 e2e（起 sidecar + OpenAI key + 進 `/explorer/<task_id>?ws_path=...` 切 llm tab + 看 inspector）。理由：vitest 12 files / 68 tests + cargo 33 case 已涵蓋 Tauri IPC 七層 enum + audit_path_safety + 跨語言 parity + composable live-tail dedup + 元件 4 tab + page row click + AuditPanel select-row + Explorer 整合 fallback 全路徑；剩 vitest 蓋不到的是真 sidecar IPC handshake + 真 LLM cost 數字 + 視覺渲染，這 Phase 7 demo prep（README §九 第五階段）反正會跑一次完整 demo
- [x] 8.5 `pre-commit run --all-files` 全綠後 commit
