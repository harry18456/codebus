## 0. Vitest infra bootstrap（apply 階段 ingest 補錄；先於所有測試）

- [x] 0.1 `cd web && npm install --save-dev vitest @vue/test-utils happy-dom @vitest/coverage-v8`，並把 `package.json` 的 `test` script 從 stub 改成 `vitest run`、補 `test:watch` = `vitest`、補 `coverage` = `vitest run --coverage`
- [x] 0.2 寫 `web/vitest.config.ts`：`environment: 'happy-dom'` / `globals: true` / `setupFiles: ['tests/setup.ts']` / `include: ['tests/**/*.spec.ts']` / `coverage.provider: 'v8'`；確保 alias 與 nuxt.config.ts 同步可解析 `~/composables/...`
- [x] 0.3 寫 `web/tests/setup.ts`：註冊全域 `class FakeEventSource` mock 暴露 `_emit(type, data)` / `_simulateError()` / `_simulateOpen()`；提供 helper `installFakeEventSource()` / `restoreEventSource()`；最後加一個 `web/tests/sanity.spec.ts` 跑 `expect(1+1).toBe(2)` 與 `EventSource` mock 可被 instantiate，確認 vitest 設定通

## 1. Fixture 與測試骨架（先做）

- [x] 1.1 從 `tests/golden/demo-synthetic/expected.json` 衍生 vitest fixture `web/tests/console/fixtures/explorer-stream.json`（demo-synthetic 並無 live `reasoning_log.jsonl`，event payloads 是 spec-conformant placeholder），覆蓋 3 step ReAct + 1 `coverage_gaps` + 1 `budget_warning` + ≥3 `progress` + 1 終態 `done`，落實 spec「Vitest fixture covers a complete event sequence」與 design 決策「Vitest fixture 採 JSON array 而非 JSONL」
- [x] 1.2 為 fixture 補一份 sibling `web/tests/console/fixtures/README.md`（不污染陣列根層，spec 要求 root 純 envelope array）說明來源 fixture 路徑與哪些事件是 placeholder 補的，方便日後重新生成

## 2. useExplorerStream composable（測試先行）

- [x] 2.1 寫 RED 單元測 `web/tests/console/useExplorerStream.spec.ts` 覆蓋 spec「useExplorerStream is the single SSE dispatch entry for the console」全部 8 個 scenario：single EventSource、agent_thought upsert、isError 啟發式、progress overwrite、coverage_gaps latest-only、budget_warning per-kind latch、auditRows 200 上限、done flag 一次性翻轉
- [x] 2.2 實作 `web/app/composables/useExplorerStream.ts`：包 `useSseTask`、四個 reactive surface（`stepBuckets` / `progress` / `coverageBanner` / `budgetBanner`）+ `auditRows` rolling window + `isError` 啟發式 + `done` flag，落實 design 決策「`useExplorerStream` 是唯一的 SSE 事件分派入口」與「Bucket-fill 而非 flat event array」
- [x] 2.3 確認 step 2.1 測試全綠（`npm run test useExplorerStream`），補修任何被 spec scenario 揭發的實作細節

## 3. Console UI 元件（TDD：每元件 RED → GREEN）

- [x] 3.1 [P] 寫 + 實作 `web/app/components/console/StepCard.vue` 與 `web/tests/console/StepCard.spec.ts`，覆蓋 spec「StepCard renders ReAct three beats in arrival order」全部 scenario，並落實 design 決策「失敗的工具呼叫與成功者一視同仁進入 ACT 區」與「`tokens_used` 為 0 時 UI 顯示「—」而非 `0 tokens`」
- [x] 3.2 [P] 寫 + 實作 `web/app/components/console/ProgressStrip.vue` 與 `web/tests/console/ProgressStrip.spec.ts`，覆蓋 spec「ProgressStrip mirrors progress events without computing stations」全部 scenario（含 placeholder 空狀態與忽略 non-exploring phase）
- [x] 3.3 [P] 寫 + 實作 `web/app/components/console/CoverageBanner.vue` 與 `web/tests/console/CoverageBanner.spec.ts`，覆蓋 spec「CoverageBanner renders coverage_gaps and budget_warning events」全部 scenario，落實 design 決策「Coverage banner 與 budget warning 的展示策略」（steps > tokens 優先、四 skip_reason 各自字樣、null 時不渲染）
- [x] 3.4 [P] 寫 + 實作 `web/app/components/console/ConsoleTimeline.vue` 與 `web/tests/console/ConsoleTimeline.spec.ts`，覆蓋 spec「ConsoleTimeline iterates stepBuckets in step ascending order」全部 scenario（step asc 排序、`:key="bucket.step"` 穩定、空狀態 placeholder、late-arrive in-place upsert）

## 4. Page 整合與 AuditPanel 接線

- [x] 4.1 寫 page `web/app/pages/explorer/[task_id].vue`：route 校驗 `^explore_[0-9a-f]{8}$`、`onMounted` 建構 `useExplorerStream` 並開 SSE、`onBeforeUnmount` 關 SSE、layout 左側掛 `ConsoleTimeline` + `ProgressStrip` + `CoverageBanner`，右側掛既有 `AuditPanel`，落實 spec「Explorer console page mounts on `/explorer/{task_id}` route」與 design 決策「Page route 採 `/explorer/[task_id]` 而非掛在 R-01 tutorial 路徑下」
- [x] 4.2 在 page 內把 `useExplorerStream(task_id).auditRows` 接到 `<AuditPanel :rows="..." />`（僅當 `activeTab === 'reasoning'` 時供應），其他 tab 給空陣列；確認不違反 `frontend-shell` 既有 AuditPanel Requirement，落實 spec「AuditPanel reasoning tab consumes useExplorerStream auditRows」
- [x] 4.3 寫 page-level integration 測 `web/tests/console/explorer-page.spec.ts`：用 fixture 重放 SSE 事件確認三節拍 timeline + AuditPanel reasoning tab 同步、route 切換時舊 SSE 被關（覆蓋 spec「Explorer console page mounts on `/explorer/{task_id}` route」中的 route change scenario）

## 5. 文件同步

- [x] 5.1 `docs/agent-explorer-spec.md` §七「前端視覺化」補一段：Step Card 三節拍 + `useExplorerStream` bucket-fill state model 對應實作，引用本 change 與 `agent-console` capability
- [x] 5.2 `docs/decisions.md` D-008 後續清單：把「Agent console 元件（顯示 reasoning_log 即時 stream；前端 Stage 6 步驟 28 開鋸）」從 `[ ]` 改成 `[x]`，並補本 change 落地註記（archive 連結待 `/spectra-archive` 後補）
- [x] 5.3 `docs/implementation-plan.md` §二第六階段步驟 28 加註「✅ landed `agent-console-p0`」（與 26 / 26.5 / 27 同步格式），不改工期表

## 6. 整合驗證

- [x] 6.1 `cd web && npm run typecheck` 全綠（無 `any` 殘留、SseEvent / StepBucket / ProgressSnapshot 介面定義完整）
- [x] 6.2 `cd web && npm run test` 全綠（含本 change 新增的 sanity test + fixture 重放測 + composable 測 + 4 元件測 + page 整合測）
- [~] 6.3 **defer 至 Phase 7 demo prep**：手動 e2e（起 sidecar + OpenAI key + 進 `/explorer/<task_id>` 看三節拍）。理由：vitest 37/37 已用 fixture 重放證明八種 SSE 事件 → 三節拍 timeline + auditRows 同步 + route change 關 SSE 路徑全通；剩下 vitest 蓋不到的是 Tauri IPC handshake + 真 LLM 視覺渲染，這兩塊 Phase 7 demo prep（README §九 第五階段）反正都會跑一次完整 demo 路線，提前重複沒有額外信心增量。Apply 階段先標記 deferred (`[~]`) 而非 done，避免假 green 訊號
- [ ] 6.4 `pre-commit run --all-files` 全綠後 commit
