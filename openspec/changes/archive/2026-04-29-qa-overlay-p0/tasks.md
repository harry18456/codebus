## 0. Pre-flight 確認 `llm-call-inspector-p0` 已 apply 完並 archive

- [x] 0.1 跑 `spectra list --json` 確認 `llm-call-inspector-p0` 不在 active list 中且 `openspec/changes/archive/*-llm-call-inspector-p0/` 存在；確認 `web/app/composables/useAuditJsonl.ts` + Tauri `audit_files::read_audit_jsonl` command 都已落地（`grep -r "useAuditJsonl" web/app/composables/` 與 `grep "read_audit_jsonl" tauri/src-tauri/src/lib.rs` 各回 ≥ 1 行）。本 change 對 `useAuditJsonl` 的擴充（新增 `liveTailFromQaSession` opt）必須建立在 base composable 已存在的基礎上

## 1. Vitest fixture（先做、被後續 task 依賴；對應 spec Requirement「`useQaSession` is a module-level singleton with one SSE dispatch entry」全部 7 個 scenario 與 design 決策「Vitest fixture 採同 `agent-console-p0` 套路」隱含）

- [x] 1.1 [P] 寫 `web/tests/qa/fixtures/qa-stream.json`：JSON array of `{ type, data }` 五種 SSE event 的完整 sequence — 1 個 `rag_hits`（含 3 hits，含 `related_stations`）+ 2 個 `agent_thought` + 3 個 `agent_action_result`（一個 `tokens_used: 0` placeholder、一個 `isError: true` 透過 `error:` 前綴啟發式）+ 2 個 `kb_growth`（一個 entry_id 與 fixture 1.2 重複用以測 dedup）+ 1 個 `qa_answer`（含 2 citations、含 station chip）+ 1 個 `done`；補 sibling `web/tests/qa/fixtures/README.md` 註記
- [x] 1.2 [P] 寫 `web/tests/qa/fixtures/kb-growth.json`：JSON array 至少 4 條 KbGrowthEntry（mirror sidecar `KBGrowthLogger.log_add` 的落盤格式：`event_type: "add"` / `entry_id` / `source` / `related_stations` / `originating_station_id` / `question` / `timestamp`）；含一個 entry_id 與 1.1 重複用以測 disk + SSE dedup

## 2. `useQaSession` composable（測試先行；對應 spec Requirement「`useQaSession` is a module-level singleton with one SSE dispatch entry」+ design 決策「`useQaSession` 採 module-level singleton 而非 per-mount instance」+「Multi-turn 視覺骨架但每筆 question 是獨立 POST /qa」+「前一 turn 未 done 不允許送下一筆，靠前端 send button disabled 而非 backend race」）

- [x] 2.1 寫 RED 單元測 `web/tests/qa/useQaSession.spec.ts`：mock `useSidecar().fetch` + 覆蓋 spec「`useQaSession` is a module-level singleton with one SSE dispatch entry」全部 7 個 scenario（two callers same singleton / start appends pending turn / 409 marks errored no SSE / rag_hits populates ragHits / kb_growth dedup by entry_id / turns FIFO cap 50 / done flips status once）+ 補 `_resetForTest()` reset hook（design 決策「`useQaSession` module-level singleton 在 vitest 多測之間殘留 state」mitigation）
- [x] 2.2 實作 `web/app/composables/useQaSession.ts`：module-scope `_state` ref + 全部 type export（`QaTurn` / `RagHit` / `Citation` / `KbGrowthEvent` / `UseQaSessionApi`）+ `start()` 流程（POST /qa → 解 task_id → useSseTask → 5 event dispatch + dedup）+ `openDrawer()` / `close()` + `_resetForTest()` debug-only export；落實 design 決策「`useQaSession` 採 module-level singleton 而非 per-mount instance」
- [x] 2.3 確認 step 2.1 全綠（`npm run test useQaSession`）

## 3. `<QaCitations>` 元件（TDD：RED → GREEN；最 leaf 元件、最先做）

- [x] 3.1 [P] 寫 RED 元件測 `web/tests/qa/QaCitations.spec.ts`：覆蓋 spec「`<QaCitations>` renders citation list with station emit」全部 3 個 scenario（empty 不渲染 / station chip click emit `navigate-to-station` / file:line 不可點）
- [x] 3.2 [P] 實作 `web/app/components/qa/QaCitations.vue`：`defineProps<{ citations: Citation[] }>()` + `defineEmits<(e: 'navigate-to-station', stationId: string) => void>()` + 純 v-for 渲染 file:line + 每筆 `related_stations` 出 station chip + 點 chip → emit
- [x] 3.3 確認 step 3.1 全綠

## 4. `<QaTurnCard>` 元件（TDD：RED → GREEN；依賴 QaCitations）

- [x] 4.1 寫 RED 元件測 `web/tests/qa/QaTurnCard.spec.ts`：覆蓋 spec「`<QaTurnCard>` renders four phases per turn」全部 5 個 scenario（4 phases all render / empty ragHits hides RAG section / streaming pulse badge / error message surfaces / kb_growth omits rollback button）
- [x] 4.2 實作 `web/app/components/qa/QaTurnCard.vue`：`defineProps<{ turn: QaTurn }>()` + 4 段 conditional render（user / RAG hits with hit cards / ReAct steps with `<QaKbGrowthBlock>` 子元件 inline / answer with `<QaCitations>`）+ status badge（pending/streaming/done/error 各 visual treatment）+ kb_growth block 不渲染 rollback
- [x] 4.3 確認 step 4.1 全綠

## 5. `<QAOverlay>` 元件（TDD：RED → GREEN；依賴 useQaSession + QaTurnCard；對應 spec Requirement「`<QAOverlay>` drawer renders Q&A turns and listens for keyboard shortcuts」+ design 決策「Drawer width 固定 480px、不可拖曳」）

- [x] 5.1 寫 RED 元件測 `web/tests/qa/QAOverlay.spec.ts`：覆蓋 spec「`<QAOverlay>` drawer renders Q&A turns and listens for keyboard shortcuts」全部 8 個 scenario（closed renders nothing / Cmd+K opens / Cmd+K no-op when open / Escape closes / dim layer click closes / aside click does not close / send button disabled while streaming / empty turns shows Cmd+K placeholder）
- [x] 5.2 實作 `web/app/components/qa/QAOverlay.vue`：`useQaSession()` 注入 + `v-if="open"` 才掛 aside + dim layer + header（title + session badge + origin chip）+ body（v-for QaTurnCard，empty 顯示 placeholder）+ composer（input + send 含 disabled gate + meta strip）；keyboard listener 註冊在 `onMounted` `window.addEventListener('keydown', handleKey)` + `onBeforeUnmount` cleanup（落實 design 決策「`Cmd+K` / `Ctrl+K` 全域召喚」）
- [x] 5.3 確認 step 5.1 全綠

## 6. `<QAEntry>` 改 imperative + page-level provide（TDD；對應 spec Requirement「`<QAEntry>` mdc element invokes `useQaSession` imperatively」+ design 決策「`<QAEntry>` mdc 改 imperative 但保留 mdc 元件契約」+「`currentStationId` 透過 page-level `provide` 注入而非 prop drill」+「Drawer 不走 vue-router、無 URL 表達」）

- [x] 6.1 寫 RED 元件測 `web/tests/qa/QAEntry-imperative.spec.ts`：mock `useQaSession()` → 覆蓋 spec 全部 3 個 scenario（click invokes start with prompt + injected stationId / missing inject 退回 null / 不呼 router.push）
- [x] 6.2 改 `web/app/components/content/QAEntry.vue` —— `<QAEntry>` mdc element invokes `useQaSession` imperatively：砍掉 `useRouter` + `router.push` placeholder，改 `useQaSession().start(props.prompt, inject<string | null>('currentStationId', null))`；保留 button shape + Tailwind classes + prop shape；註解寫明「imperative 呼 useQaSession 是 trigger 不是 fetch—frontend-shell invariant 仍滿足」
- [x] 6.3 改 `web/app/pages/tutorial/[workspace_id]/[station_id].vue`：`setup()` 內 `provide('currentStationId', stationId.value)`；不破既有 `r-01-station-board` archive 任一 scenario
- [x] 6.4 確認 step 6.1 全綠 + 既有 R-01 archive 5 個 mdc 契約測（QAEntry 行為部分）regression check 全綠

## 7. Layout-level mount + Cmd+K listener

- [x] 7.1 改 `web/app/layouts/default.vue`：在 layout 樹底加 `<QAOverlay />` + 全域 `Cmd+K` / `Ctrl+K` keyboard listener（`onMounted` register / `onBeforeUnmount` removeListener）；handler 偵測 `(e.metaKey || e.ctrlKey) && e.key === 'k'` → preventDefault + `useQaSession().openDrawer()`；已開不切換不關（落實 design 決策「`Cmd+K` / `Ctrl+K` 全域召喚」)
- [x] 7.2 page integration 測 `web/tests/qa/qa-overlay-page-integration.spec.ts`：mount 整個 layout-default，模擬 R-01 station page mount → trigger Cmd+K → drawer opens → 模擬 SSE event chain（reuse fixture 1.1）→ assert turn 4 phase 全渲染 + AuditPanel kb_growth tab 同步出現新 entry（reuse fixture 1.2 + live-tail）+ Escape 關 drawer

## 8. `useAuditJsonl` 擴充 `liveTailFromQaSession` opt（對應 spec Requirement「`useAuditJsonl` supports kb_growth live-tail from useQaSession」+ design 決策「`useAuditJsonl` 擴充：第二個 live-tail kind」）

- [x] 8.1 寫 RED 單元測 `web/tests/qa/useAuditJsonl-kb-growth.spec.ts`：覆蓋 spec「`useAuditJsonl` supports kb_growth live-tail from useQaSession」全部 3 個 scenario（kb_growth live-tail appends / dedup by entry_id / liveTailFromQaSession ignored for non-kb_growth kind）+ regression：既有 `liveTailFromExplorerStream` 對 `kind === "llm"` 不破
- [x] 8.2 改 `web/app/composables/useAuditJsonl.ts`：`opts` 介面新增 `liveTailFromQaSession?: UseQaSessionApi`；內部 `kind === 'kb_growth'` 條件下 watch session SSE event chain 並 dedup append by `entry_id`；不破既有 llm 那條 live-tail 邏輯
- [x] 8.3 確認 step 8.1 全綠 + `llm-call-inspector-p0` archive 既有 5 個 useAuditJsonl scenario 全綠（regression）

## 9. Explorer page kb_growth tab 接通（對應 design 決策「`kb_growth` AuditPanel tab 採 dual-source merge：disk read + SSE live-tail」）

- [x] 9.1 改 `web/app/pages/explorer/[task_id].vue`：除既有 `useAuditJsonl(ws, 'llm', { liveTailFromExplorerStream: stream })` 外，再加 `useAuditJsonl(ws, 'kb_growth', { liveTailFromQaSession: useQaSession() })`；`activeTab === 'kb_growth'` 時餵這份給 AuditPanel；其他 tab 行為不變
- [x] 9.2 page integration 測（reuse step 7.2 的 spec 檔加 case）：在 explorer page 模擬 QA SSE `kb_growth` event → AuditPanel kb_growth tab 同步顯示新 row

## 10. 文件同步

- [x] 10.1 `docs/decisions.md` D-016 後續清單（搜尋「前端聊天 UI」）：把對應 `[ ]` 改 `[x]`，補本 change 名稱與 archive 落地日期 placeholder
- [x] 10.2 `docs/qa-agent.md §八`「前端聊天 UI 契約」段補一段「P0 drawer overlay 模式（archive `qa-overlay-p0`）」：闡明 drawer overlay 而非 page、`useQaSession` 是 module-level singleton、Cmd+K 召喚、ESC 關閉、turns FIFO cap 50、citation file:line 不可點屬 P1
- [x] 10.3 `docs/implementation-plan.md` §二第六階段步驟 30 加註「✅ landed `qa-overlay-p0`」（與步驟 26 / 26.5 / 27 / 28 / 28.5 同款格式），不改工期表
- [x] 10.4 `CLAUDE.md` 「## 子系統」段 `web/` 子段補 composable / page 對應（同 `agent-console-p0` 落地後 `useExplorerStream` 補錄方式）

## 11. 整合驗證

- [x] 11.1 `cd web && npm run typecheck` 全綠（`QaTurn` / `RagHit` / `Citation` / `KbGrowthEvent` / `UseQaSessionApi` 介面完整、無 `any` 殘留）
- [x] 11.2 `cd web && npm run test` 全綠（含本 change 新增 7 份測 + `agent-console-p0` 既有 7 份 + `llm-call-inspector-p0` 既有 5 份 = 19+ files；舊既測 0 regression）
- [x] 11.3 手動 e2e（Phase 7 demo prep 順帶驗、本 change 不擋）：起 sidecar + OpenAI key + Qdrant → R-01 station page 點 `<QAEntry>` 或按 Cmd+K → drawer 出 → 問題送出 → 看 RAG hits + ReAct steps + kb_growth event card + answer with citations 全到位 → 切 AuditPanel `kb_growth` tab 看 live-tail row → 點 station chip 測 navigate emit → ESC 關 drawer；估算成本 < $1
- [x] 11.4 `pre-commit run --all-files` 全綠後 commit
