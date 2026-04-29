## 1. `GET /sanitizer/rules` sidecar endpoint（先做、Python infra；對應 spec Requirement「`GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot」+ design 決策「Decision 2 — Rule explainer 從 sanitizer rules registry 拉，不從 audit log 拉」+「Decision 5 — `useSanitizeAudit` composable 邊界」）

- [x] 1.1 [P] 寫 RED 測 `sidecar/tests/api/test_sanitizer_rules.py`：覆蓋 spec「`GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot」全部 6 個 scenario（authenticated GET 200 + schema / missing bearer 401 / `rules_version` 與 `sanitize_audit.jsonl` writer 一致 / endpoint 唯讀（多次呼叫不寫 audit、registry 不變）/ `pattern_summary` 非執行 regex（不含 `(?P<` / `(?:` / 長度 ≤ 80）/ 空 registry 200 with `rules: []`）；fixture 用 monkeypatch 注入兩條 builtin + 一條 user_yaml 的 mock registry
- [x] 1.2 寫 `sidecar/src/codebus_agent/api/sanitizer_rules.py`：FastAPI router export `GET /sanitizer/rules` handler；從 `SanitizerEngine` / `RuleBasedPIIProvider` 內部 rule table 讀 builtin 條目 + 從 `SanitizerConfig` 讀 user_yaml 條目，合併後 dump `rules_version` + `rules[]`；嚴禁觸碰 `SanitizerAuditLogger` / `AuditEntry`（落實 design 決策「P0 嚴格只讀」）
- [x] 1.3 在 `sidecar/src/codebus_agent/api/__init__.py` 註冊 `sanitizer_rules` router；確認 `uv run pytest sidecar/tests/api/test_sanitizer_rules.py -v` 全綠
- [x] 1.4 [P] 寫 cross-check 測 `sidecar/tests/sanitizer/test_rules_version_parity.py`：起 SanitizerEngine、跑一次 `sanitize` → 抓 `sanitize_audit.jsonl` 寫入的 `rules_version` → 同 process 呼 `GET /sanitizer/rules` → 比對兩處 `rules_version` 字串完全相等；對應 spec「rules_version matches sanitize_audit.jsonl writes」scenario 的工程實作面

## 2. `useSanitizerRules` composable（測試先行；對應 spec Requirement「`useSanitizerRules` composable fetches rules registry from sidecar」+ design 決策「Decision 2 — sidecar 唯一事實來源 vs 前端寫死 lookup table」）

- [x] 2.1 [P] 寫 fixture `web/tests/audit/fixtures/sanitizer-rules.json`：JSON 至少 5 條 SanitizerRule，覆蓋多 kind（secret / pii / internal）+ 多 source（builtin × 4 + user_yaml × 1）+ 至少一條的 `pattern_summary` 是 `<email RFC 5322>` 摘要式 placeholder 而非真 regex；補 sibling `web/tests/audit/fixtures/README.md` 註記對應 spec scenario
- [x] 2.2 寫 RED 單元測 `web/tests/audit/useSanitizerRules.spec.ts`：mock `useSidecar().fetch` + 覆蓋 spec「`useSanitizerRules` composable fetches rules registry from sidecar」全部 4 個 scenario（rules fetched once per session / lookup matches existing rule_id / lookup returns null for unknown / source 不含 `pattern_full` `regex_full` `?full=true` `&full=true` 字串）
- [x] 2.3 實作 `web/app/composables/useSanitizerRules.ts`：`SanitizerRule` typed interface + module-level `Ref<SanitizerRulesSnapshot | null>` cache + `loadOnce()` lazy 觸發 + `lookup(ruleId)` lookup helper；嚴禁直接呼 `read_audit_jsonl` / `Tauri.invoke` 其他 audit kind（落實 spec「Composable does not request full regex source」scenario）
- [x] 2.4 確認 step 2.2 全綠（`npm run test useSanitizerRules`）

## 3. `useSanitizeAudit` composable（測試先行；對應 spec Requirement「`useSanitizeAudit` composable parses sanitize_audit rows into a view-model」+ design 決策「Decision 5 — `useSanitizeAudit` composable 邊界」）

- [x] 3.1 [P] 寫 fixture `web/tests/audit/fixtures/sanitize-audit.jsonl`：至少 8 筆 sanitize_audit 真實 schema（`ts` / `schema_version` / `rules_version` / `pass` 1/2/3 各 ≥ 1 / `session_id` 至少 2 種 / `source` dict 與 string 兩型並存 / `extra: {allowlisted: true}` 一條 / `extra: {}` 多條 / `kind` 至少 secret / pii / internal 三種）；補對應 README 註記
- [x] 3.2 寫 RED 單元測 `web/tests/audit/useSanitizeAudit.spec.ts`：mock `useAuditJsonl('sanitize')` 餵 step 3.1 fixture + 覆蓋 spec「`useSanitizeAudit` composable parses sanitize_audit rows into a view-model」全部 5 個 scenario（source dict 形 view / source 字串形 view / kindSummary 反應式重算 / sessionTimeline 排序與分組 / source-grep `useSanitizeAudit.ts` 不含 `read_audit_jsonl` 字面）
- [x] 3.3 實作 `web/app/composables/useSanitizeAudit.ts`：thin wrapper over `useAuditJsonl('sanitize')`；`SanitizeRowView` typed interface（`row` + `sourceView` + `placeholderToken` + `passLabel`）+ derived computed `kindSummary: ComputedRef<Map<string, number>>` + `sessionTimeline: ComputedRef<Map<string, SanitizeRowView[]>>`；`PASS_LABELS` 常數 export 給 `SanitizerAuditInspector` 與 `AuditPanel` 共用（落實 spec「Pass integer mapped to human-readable label」的「single TypeScript constant」要求）
- [x] 3.4 確認 step 3.2 全綠（`npm run test useSanitizeAudit`）

## 4. `SanitizerAuditInspector` overlay 元件（TDD：RED → GREEN；對應 spec Requirement「`SanitizerAuditInspector` overlay renders metadata-only view of a sanitize_audit row」+「`SanitizerAuditInspector` displays a D-015 banner verbatim」+ design 決策「Decision 1 — P0 嚴格 metadata-only」+「Decision 4 — D-015 banner 文字逐字定稿」）

- [x] 4.1 寫 RED 元件測 `web/tests/audit/SanitizerAuditInspector.spec.ts`：覆蓋 spec「`SanitizerAuditInspector` overlay renders metadata-only view of a sanitize_audit row」全部 6 個 scenario（10 metadata fields render / pass label mapping / no raw value reconstruction / extra.allowlisted=true → 綠勾 chip / unknown source shape → fallback 不 throw / no mutation affordances）+ spec「`SanitizerAuditInspector` displays a D-015 banner verbatim」全部 3 個 scenario（banner 文字 always render / 無 hideBanner prop 與 dismiss button / 字面只在 inspector module 一處）
- [x] 4.2 實作 `web/app/components/audit/SanitizerAuditInspector.vue`：`<aside>` overlay + 頂部 sticky banner（從 `SANITIZER_AUDIT_BANNER` 常數讀）+ header（rule_id chip + placeholder token chip purple + passLabel + ts）+ body 10 行 metadata 表（`ts` / `schema_version` / `rules_version` / `pass` / `session_id` / `source` 用 `useSanitizeAudit().sourceView` 渲染 / `rule_id` / `kind` / `placeholder_index` / `extra`）+ rule explainer 用 `useSanitizerRules().lookup(rule_id)` 取 description + pattern_summary + source（builtin/user_yaml）+ 綠勾 chip 特化 `extra.allowlisted: true`；落實 design 決策「Decision 1 — P0 嚴格 metadata-only」（無 raw 字串網路請求、無 KB lookup）
- [x] 4.3 [P] 寫 banner 字面 source-grep 測 `web/tests/audit/sanitizer-banner-single-source.spec.ts`：grep `web/app/` 整樹搜尋字串 `raw values are not retained per D-015`，僅 inspector 模組有 1 個匹配（`SANITIZER_AUDIT_BANNER` 常數宣告處）；其他檔案如 `pages/audit/sanitizer.vue` / `AuditPanel.vue` 必須 `import { SANITIZER_AUDIT_BANNER }`，不可內聯字面
- [x] 4.4 確認 step 4.1 + 4.3 全綠（`npm run test SanitizerAuditInspector sanitizer-banner`）

## 5. AuditPanel surfaces seven workspace-level audit JSONL tabs (sanitize tab MODIFIED)

對應 frontend-shell delta spec MODIFIED Requirement「AuditPanel surfaces seven workspace-level audit JSONL tabs」新加的 3 個 sanitizer-specific scenario（既有元件擴充，TDD）

- [x] 5.1 寫 RED 元件測 `web/tests/audit/AuditPanel-sanitize-tab.spec.ts`：覆蓋 frontend-shell delta spec MODIFIED Requirement 新加的 3 個 scenario（sanitize tab placeholder chip 用 purple token / pass chip 顯示 `Pass 1`/`Pass 2`/`Pass 3` 而非 `1`/`2`/`3` / sanitize tab row click 由 parent SanitizerAuditInspector 接，AuditPanel 不 mount 自己的 inspector）；同時 regression 跑既有 6 個 scenario（七 tab 順序 / empty state / no CB_AUDIT_SAMPLES / select-row 三條）
- [x] 5.2 改 `web/app/components/audit/AuditPanel.vue` 的 sanitize tab body template：補 placeholder token chip（用 `purple` token utility，對齊 `Purple stays sanitizer-exclusive` 既有 frontend-shell scenario）+ pass chip（從 `useSanitizeAudit().PASS_LABELS` 讀 label，不 hardcode）；嚴禁加任何「打開 inspector」直接行為（仍只 `$emit('select-row', idx)`，落實 dumb display surface 契約）
- [x] 5.3 確認 step 5.1 全綠 + 既有 28.5 archive 的 `AuditPanel-select-row.spec.ts` 與 `agent-console-p0` archive 的 page 整合測也全綠（regression check）

## 6. `/audit/sanitizer` standalone page（TDD；對應 spec Requirement「`/audit/sanitizer` standalone page surfaces inspector outside R-01 workspace」+ design 決策「Decision 4 — D-015 banner 三處同字串」）

- [x] 6.1 寫 RED page 測 `web/tests/audit/sanitizer-page.spec.ts`：覆蓋 spec「`/audit/sanitizer` standalone page surfaces inspector outside R-01 workspace」全部 3 個 scenario（valid workspace query → render inspector + banner + 無 station chrome / 缺 query → empty state + 無 read_audit_jsonl call + banner 仍在 / network spy 確認只呼 `read_audit_jsonl` with `audit_kind: 'sanitize'` + `/sanitizer/rules` 兩個 endpoint，無其他 audit kind 讀）
- [x] 6.2 實作 `web/app/pages/audit/sanitizer.vue`：query `?workspace=` 校驗 + 缺時 empty state + `useAuditJsonl(workspace, 'sanitize')` + `useSanitizerRules()` lazy load + 左 list（按 `ts` desc 顯示）+ row click → 開 `<SanitizerAuditInspector>` overlay + 頂部 banner（從 `SANITIZER_AUDIT_BANNER` 常數 import）+ loading / empty / error 三狀態；嚴禁直接 fetch 任何其他 audit JSONL（落實 spec「Page does not call non-sanitize audit reads」scenario）
- [x] 6.3 確認 step 6.1 全綠

## 7. R-01 + Explorer page 注入 SanitizerAuditInspector overlay（既有 page 擴充，整合測）

- [x] 7.1 改 `web/app/pages/tutorial/[workspace_id]/[station_id].vue`：在現有 28.5 LlmCallInspector 注入點旁邊新增 `<SanitizerAuditInspector>` overlay 注入；綁 AuditPanel `sanitize` tab 的 `select-row` emit → 控制本 overlay 的 `activeIndex`；落實 frontend-shell delta spec「Sanitize tab row click is hosted by parent SanitizerAuditInspector, not AuditPanel」scenario
- [x] 7.2 改 `web/app/pages/explorer/[task_id].vue`：同 7.1 在現有 28.5 LlmCallInspector 注入點旁邊新增 `<SanitizerAuditInspector>` overlay；綁 sanitize tab `select-row` → activeIndex；確認 sanitize tab 與 llm tab 的 inspector overlay 互相獨立（兩個 overlay 各自 mount，切 tab 時不共享 activeIndex）
- [x] 7.3 [P] 寫 page 整合測 `web/tests/audit/sanitize-overlay-integration.spec.ts`：覆蓋兩個 page 同款情境（station page 與 explorer page 各自切 sanitize tab → 點 row → 看 SanitizerAuditInspector 開、看 LlmCallInspector 不開；切 llm tab → 點 row → 看 LlmCallInspector 開、看 SanitizerAuditInspector 不開）；落實 frontend-shell delta spec MODIFIED Requirement「Sanitize tab row click is hosted by parent SanitizerAuditInspector, not AuditPanel」scenario 的 page-level 落地點

## 8. 文件同步

- [x] 8.1 `docs/decisions.md` 找到 D-015 後續清單（line 376-380 區段），加註本 change 落地：`- [x] O-05 Sanitizer Audit Inspector P0 落地（sanitizer-audit-inspector-p0，<archive 日期>）`；不更動 D-015 主決策表（P0 嚴格守 D-015，未動不變式）
- [x] 8.2 `docs/implementation-plan.md` §二第六階段步驟 28.5 後的 O-05 條目加註「✅ landed `sanitizer-audit-inspector-p0`」（與步驟 26 / 26.5 / 27 / 28 / 28.5 同款格式）；同時在「未來 follow-up」段標註 `sanitizer-audit-unlock` capability defer（避免 P1 啟動時得反向考古）
- [x] 8.3 `CLAUDE.md` 「## 子系統」段 `web/` 子段補一句 composable / page 對應（同 `agent-console-p0` 與 `llm-call-inspector-p0` 落地後的補錄方式）；同時把 `Phase 6 step` 段「Phase 6 收尾後接 D-033 Change B」前面的 step 列表加上「**O-05 Sanitizer Audit Inspector ✅**」狀態，讓 CLAUDE.md 反映 step 28.5 之後的進度

## 9. 整合驗證

- [x] 9.1 `cd sidecar && uv run pytest tests/api/test_sanitizer_rules.py tests/sanitizer/test_rules_version_parity.py -v` 全綠（兩份新測 + 既有 sanitizer / api 測 0 regression）
- [x] 9.2 `cd web && npm run typecheck` 全綠（`SanitizerRule` / `SanitizeRowView` / `SANITIZER_AUDIT_BANNER` / `PASS_LABELS` 完整 typed，無 `any` 殘留）
- [x] 9.3 `cd web && npm run test` 全綠（本 change 新增 7 份測：useSanitizerRules / useSanitizeAudit / SanitizerAuditInspector / sanitizer-banner-single-source / AuditPanel-sanitize-tab / sanitizer-page / sanitize-overlay-integration；舊既測 0 regression）
- [x] 9.4 [~] **defer 至 Phase 7 demo prep**：手動 e2e（起 sidecar + 真 OpenAI key 跑一次 sanitize 進 `sanitize_audit.jsonl` + 進 `/audit/sanitizer?workspace=...` 看 inspector 顯示 metadata + 點 rule_id 看 explainer + 確認 banner 字面正確）。理由：vitest + pytest 已涵蓋全部 spec scenario 的可測面（fixture-driven），剩 vitest 蓋不到的是真 sidecar IPC handshake + 真渲染視覺，這 Phase 7 demo prep 反正會跑一次完整 demo
- [x] 9.5 `pre-commit run --all-files` 全綠後 commit

---

## Design coverage map

每條 design.md 決策對應的 task：

- **Decision 1：P0 嚴格 metadata-only，不做 raw retention** → Task 4.1 / 4.2 / 4.3 / 4.4（`SanitizerAuditInspector` overlay 元件嚴格只渲染 metadata，spec scenario「No raw value reconstruction attempted」+「No mutation affordances exposed」對應 RED 測落實）
- **Decision 2：Rule explainer 從 sanitizer rules registry 拉，不從 audit log 拉** → Task 1.1 / 1.2 / 1.3 / 1.4（`GET /sanitizer/rules` sidecar endpoint）+ Task 2.1 / 2.2 / 2.3 / 2.4（`useSanitizerRules` composable）
- **Decision 3：對齊 28.5 的 select-row + parent-hosts-overlay pattern** → Task 5.1 / 5.2 / 5.3（AuditPanel `sanitize` tab MODIFIED 守 dumb display surface 契約）+ Task 7.1 / 7.2 / 7.3（R-01 + Explorer page 注入 SanitizerAuditInspector overlay，與 28.5 LlmCallInspector 注入點對稱）
- **Decision 4：D-015 banner 文字逐字定稿** → Task 4.2（inspector overlay banner sticky）+ Task 4.3（banner 字面 source-grep 測：`SANITIZER_AUDIT_BANNER` 常數唯一字面來源）+ Task 6.2（`/audit/sanitizer` page 從常數 import，不內聯）
- **Decision 5：`useSanitizeAudit` composable 邊界** → Task 3.1 / 3.2 / 3.3 / 3.4（`useSanitizeAudit` thin wrapper over `useAuditJsonl('sanitize')`，不直接呼 `read_audit_jsonl`、不 cache rules、職責邊界對齊 28.5 `useLlmCalls` 對 `useAuditJsonl('llm')`）

## Spec requirement coverage map

每條 spec Requirement 對應的 task：

- **Requirement: AuditPanel surfaces seven workspace-level audit JSONL tabs**（`frontend-shell` MODIFIED）→ Task 5.1 / 5.2 / 5.3（AuditPanel `sanitize` tab placeholder chip + pass chip + dumb display surface 契約守住）
- **Requirement: `SanitizerAuditInspector` overlay renders metadata-only view of a sanitize_audit row**（`sanitizer-audit-inspector` ADDED）→ Task 4.1 / 4.2 / 4.4
- **Requirement: `SanitizerAuditInspector` displays a D-015 banner verbatim**（`sanitizer-audit-inspector` ADDED）→ Task 4.1 / 4.2 / 4.3 / 4.4
- **Requirement: `useSanitizeAudit` composable parses sanitize_audit rows into a view-model**（`sanitizer-audit-inspector` ADDED）→ Task 3.1 / 3.2 / 3.3 / 3.4
- **Requirement: `useSanitizerRules` composable fetches rules registry from sidecar**（`sanitizer-audit-inspector` ADDED）→ Task 2.1 / 2.2 / 2.3 / 2.4
- **Requirement: `GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot**（`sanitizer-audit-inspector` ADDED）→ Task 1.1 / 1.2 / 1.3 / 1.4
- **Requirement: `/audit/sanitizer` standalone page surfaces inspector outside R-01 workspace**（`sanitizer-audit-inspector` ADDED）→ Task 6.1 / 6.2 / 6.3 + Task 7.1 / 7.2 / 7.3（page-level 注入 + 整合測）
