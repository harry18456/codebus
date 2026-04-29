## Why

Trust Layer 四站之一 O-05 要把 D-015 的「Sanitizer 三段不可逆替換」從文件承諾變成使用者親眼可驗的 UI。`sanitize_audit.jsonl` 已經被 Pass 1 (Scanner 入 KB 前) / Pass 2 (Provider pre-flight) / Pass 3 (Q&A `add_to_kb`) 共同寫入，但 R-01 工作區 `AuditPanel` 對 `sanitize` tab row 點下去沒有任何 inspector，使用者看不到 placeholder 對應的 `rule_id` / `kind` / `pass` / `rules_version` 等 metadata，也看不到 sanitizer rules registry 的人類可讀說明。28.5 LLM Call Inspector（剛 archive，2026-04-29）已把同款 row-click → overlay 的 pattern 跑通；本 change 跟著這條軌跡把 `sanitize` 那一格補完，剛好收尾 Trust Layer 第一輪四站視覺。

**範圍邊界由 D-015 嚴格定義**：`AuditEntry` 真實 schema 只有 `rule_id` / `kind` / `placeholder_index` / `source` / `extra`（`source` 是 `"file:<path>"` / `"message:<id>"` / `{"pass": "scanner", "path": ...}` 三型，`extra` 目前只用到 `{"allowlisted": true}`），沒有 raw 字串、沒有 pre/post 並排、沒有 line number。本 P0 嚴格只做 metadata viewer；mockup `design/v0/o-05-sanitizer-diff.html` 中的 3-pane raw/sanitized diff、unlock-with-grant flow、auto-relock countdown、raw value retention、`audit_session_id` chain **全部 defer 到 P1+ 獨立 capability `sanitizer-audit-unlock`**，與 28.5 archive note（`docs/decisions.md` line 724）對 LLM Call Inspector 採取的判決一致 —— 「P0 只渲染 post-sanitize wire payload 並顯眼放 D-015 banner」。

對齊 `docs/implementation-plan.md §二第六階段` 步驟 28.5 後的 O-05 條目；引用 D-015（三段 Sanitizer 不變式）、D-022（`llm_calls.jsonl` 記 post-Pass 2，不還原 pre-sanitize）作為邊界。

## What Changes

- 新 capability `sanitizer-audit-inspector` 規範 `SanitizerAuditInspector.vue` overlay 元件契約：左欄 metadata 表（`ts` / `pass` 1-2-3 chip / `rules_version` / `session_id` / `source` / `rule_id` / `kind` / `placeholder_index` / `extra.allowlisted`），右欄 rule explainer（從 sanitizer rules registry / `~/.codebus/sanitizer.local.yaml` 拉 description + pattern），下方 session timeline（同 `session_id` 多筆 row 按 `ts` 排序）。所有 placeholder token 用 `purple` token highlight（D-015 sanitizer 專屬色，既有 `frontend-shell` 不變式）。
- 新 capability 同時規範 `/audit/sanitizer` 獨立頁路由（對齊 `/audit/llm` standalone reviewer 模式，rationale 一致）。
- 顯眼 D-015 banner（對齊 28.5）：「Audit metadata only · raw values not retained per D-015. Placeholder reveal requires a future audit-unlock capability.」
- 修改 `frontend-shell` 既有 Requirement `AuditPanel surfaces seven workspace-level audit JSONL tabs`：在 `sanitize` tab 綁定 `SanitizerAuditInspector` overlay（既有 `select-row` emit + parent-hosts-overlay 契約不變，本 change 只新增「`sanitize` tab 預設綁哪一支 inspector」+「purple token 在 placeholder chip / rule kind label 的 sanitizer-only 用途」兩條細節 scenario）。
- 新 composable `useSanitizeAudit`（thin wrapper over 既有 `useAuditJsonl('sanitize')`）負責解析 `sanitize_audit.jsonl` 每筆 row 的 metadata、計算 placeholder kind/count summary、按 `session_id` 分組產 timeline 視圖資料。
- 新 composable `useSanitizerRules` 從 sidecar 既有 sanitizer config endpoint 拉 rules registry（rule_id → description / pattern），inspector rule explainer 用；如該 endpoint 尚未存在，本 change 範圍包含「sidecar 補一支 `GET /sanitizer/rules` endpoint 回傳 rule registry snapshot」最小變更（只讀，不寫；不變動 sanitizer 既有邏輯）。

## Non-Goals

明確 defer 到 P1+ `sanitizer-audit-unlock` capability，本 change 不做：

- **3-pane raw / sanitized side-by-side diff**（mockup `design/v0/o-05-sanitizer-diff.html` line 384-517）—— 需要 sidecar 在 sanitize 當下另開 raw value retention store（記憶體或加密本地檔），違反 D-015「KB / reasoning_log / 教材全存清理版；不存 reverse mapping」；P1+ 要做必須先有獨立 ADR 評估 retention 機制 + threat model。
- **Unlock-with-grant flow**（mockup line 520-560）—— 整套 modal / scope picker (`file` / `all_placeholders`) / 確認 raw 暴露 / 寫 `audit_unlock` event。需要與 O-01 grant capability 對齊新 grant_kind（`raw_value_reveal`），P1+ 範圍。
- **Auto-relock countdown**（mockup line 343-355、562-570）—— 15 分鐘無操作自動 relock、`countdown.warn` 30 秒前 toast、「延長一次」寫新 audit_unlock；需要 frontend timer + sidecar TTL 同步機制，P1+ 範圍。
- **Raw value retention**（任何形式持久化原值）—— 違反 D-015 不變式；若 P1+ 要做必須先以 ADR 形式評估「短期記憶體 only / process-死即清 / 磁碟絕不落地」是否仍合 D-015 字面與精神。
- **`audit_session_id` chain**（mockup line 346）—— `audit_unlock` 事件鏈、跨 unlock 的 session 串接、unlock 行為自身的稽核（reason / scope / 觸發者），全部 P1+ 範圍。
- **`/audit/sanitizer` 上的「locate」跳回 KB chunk** —— 需要 sanitize_audit row 帶 KB point_id 反查（目前 schema 沒有），P1+ 評估是否補欄位。
- **修改 sanitizer 既有寫入邏輯 / `AuditEntry` schema** —— P0 嚴格只讀；任何 schema 變動必須走獨立 change（且需 D-015 ADR review）。

## Capabilities

### New Capabilities

- `sanitizer-audit-inspector`: `SanitizerAuditInspector.vue` overlay 契約 + `/audit/sanitizer` standalone 頁面 + `useSanitizeAudit` composable 的 row 解析 / kind summary / session timeline 責任邊界 + `useSanitizerRules` composable 的 rules registry 取得契約 + sidecar `GET /sanitizer/rules` 唯讀 endpoint（如尚未存在）+ D-015 banner 文案與顯示時機規範。

### Modified Capabilities

- `frontend-shell`: 既有 `AuditPanel surfaces seven workspace-level audit JSONL tabs` Requirement 的 `sanitize` tab 接 `SanitizerAuditInspector` overlay（既有 select-row emit 與 parent-hosts-overlay 契約已是現有契約，本 change 只新增「`sanitize` tab 預設 inspector 綁定」+「purple token 在 placeholder chip / rule kind label 的 sanitizer-only 用途」兩條細節 scenario）。

## Impact

- Affected specs:
  - 新 `openspec/specs/sanitizer-audit-inspector/spec.md`
  - 修改 `openspec/specs/frontend-shell/spec.md`（既有 AuditPanel Requirement 增 sanitize-tab 綁定 + purple-token-sanitizer-exclusive scenario）
- Affected code:
  - New:
    - `web/app/components/audit/SanitizerAuditInspector.vue`
    - `web/app/composables/useSanitizeAudit.ts`
    - `web/app/composables/useSanitizerRules.ts`
    - `web/app/pages/audit/sanitizer.vue`
    - `sidecar/src/codebus_agent/api/sanitizer_rules.py`（如 `GET /sanitizer/rules` endpoint 尚未存在）
    - `sidecar/tests/api/test_sanitizer_rules.py`（endpoint 對應測試）
  - Modified:
    - `web/app/components/audit/AuditPanel.vue`
    - `web/app/pages/tutorial/[workspace_id]/[station_id].vue`
    - `web/app/pages/explorer/[task_id].vue`
    - `sidecar/src/codebus_agent/api/__init__.py`（如新 endpoint，需註冊 router）
    - `docs/implementation-plan.md`（O-05 進度條打勾 + Non-Goals 列 P1+ `sanitizer-audit-unlock` follow-up）
  - Removed: 無
- Dependencies:
  - 不新增 npm 套件
  - 不動 Tauri Rust（`read_audit_jsonl` Tauri command 已支援 `sanitize` enum，28.5 落地時已驗）
  - sidecar 視 `GET /sanitizer/rules` 是否已存在決定要不要動 Python（規模一支唯讀 endpoint + handler；不動 SanitizerEngine / AuditEntry / AuditLogger 任何邏輯，符合 P0 嚴格只讀邊界）
- Follow-up capability：未來 `sanitizer-audit-unlock`（P1+）會把 mockup v0 的 3-pane diff + unlock flow + auto-relock + raw retention + audit_session_id chain 一次實作齊全；本 change 在 design 與 spec trace 留下明確 forward link，避免 P1 啟動時得反向考古。
