## Context

Trust Layer 四站第三站 O-05 要對 `sanitize_audit.jsonl` 開 inspector。28.5 LLM Call Inspector（archive 2026-04-29）已建立 row-click → overlay + parent-hosts-overlay + select-row emit 的標準 pattern，本 change 直接套用。

特殊性在於 D-015 嚴格界線 —— 設計師 v0 mockup（`design/v0/o-05-sanitizer-diff.html`）畫了 3-pane raw / sanitized / placeholder 並排的 unlock-with-grant flow，但 `AuditEntry` 真實 schema (`sidecar/src/codebus_agent/sanitizer/engine.py:71`) 只記 metadata（`rule_id` / `kind` / `placeholder_index` / `source` / `extra`），無 raw 字串、無 line number、無上下文。要做 mockup 那種 diff 必須先建 raw retention store，這已經跨越 D-015「不存 reverse mapping / KB / reasoning_log / 教材全存清理版」邊界，且 28.5 archive 時已經對同款訴求做過判決（`docs/decisions.md` line 724）：「Pre/post sanitize diff defer ⋯⋯ 需另開 audit-unlock capability」。本 design 對齊這個判決，同樣 defer。

利益相關者：
- 使用者（Trust Layer 對外承諾的見證人）—— 需要 metadata 可驗
- D-015 ADR（不可逆替換不變式）—— 邊界守護
- 28.5 LLM Call Inspector 既有 pattern —— 對齊源
- 未來 P1+ `sanitizer-audit-unlock` capability —— 本 change 的延伸

## Goals / Non-Goals

**Goals:**

- 把 `sanitize_audit.jsonl` 的 metadata（10 欄）以 inspector overlay 形式暴露，使用者可逐筆看 `rule_id` / `kind` / `pass` / `rules_version` / `session_id` / `placeholder_index` / `source` / `extra.allowlisted` / `ts`。
- 提供 rule explainer：點 `rule_id` 從 sanitizer rules registry 拉 description + pattern（人類可讀），與 audit log 解耦。
- 提供 session timeline：同 `session_id` 多筆 row 按 `ts` 排序串成 sanitize pass 全景。
- 提供 placeholder summary：`kind: count` 統計（用 `purple` token，sanitizer 專屬色）。
- `/audit/sanitizer` standalone page 對齊 `/audit/llm` 的 reviewer-not-required-to-be-in-workspace 模式。
- 顯眼 D-015 banner，文字明確說明「metadata only · raw values not retained · placeholder reveal requires future audit-unlock capability」。
- 整體規模對稱 28.5（~25 task、純前端為主、最多一支 sidecar 唯讀 endpoint）。

**Non-Goals:**

詳見 `proposal.md` Non-Goals 段落。摘要：3-pane raw/sanitized diff、unlock-with-grant flow（modal / scope picker）、auto-relock countdown、raw value retention、`audit_session_id` chain、locate-back-to-KB-chunk、修改 sanitizer 既有寫入邏輯 — 全部 defer 到 P1+ `sanitizer-audit-unlock`。

## Decisions

### Decision 1：P0 嚴格 metadata-only，不做 raw retention

選擇 metadata-only 而非 mockup v0 的 3-pane raw diff。

**替代方案 A**：在 sidecar 加短期 raw value cache（process 記憶體 only，process 死即清，磁碟絕不落地）—— 看似合 D-015 字面（「儲存」可解讀為磁碟），但精神有爭議：
- 任何 process 記憶體都可能被 dump（OS swap / coredump）
- D-015 line 368「不存 reverse mapping」明確是「不存」而非「不持久化」，從嚴解讀記憶體也算
- 即使技術上能做，需要先有 ADR 評估 threat model，這個工期不在 P0

**替代方案 B**：sidecar 額外加密落地 `~/.codebus/<ws>/raw_values.encrypted`，與 `authorization_audit` 同層 —— 直接違反 D-015，需要 ADR 推翻原決策。

**選定方案**：P0 不做 raw retention；將整套 raw retention + unlock + auto-relock + audit chain 收斂到單一 P1+ capability `sanitizer-audit-unlock`，未來啟動時整體決策（包括 D-015 ADR review）。

理由：
- 28.5 archive note 已對 LLM Call Inspector 同款訴求做過判決（`docs/decisions.md` line 724）
- 守住 D-015 不變式比追 mockup 視覺等量更重要 —— Trust Layer 的核心承諾不能因 demo 圖好看而鬆動
- 範圍小、可獨立交付、不擋 demo（metadata 已經是強有力的「audit log 真的記了什麼」見證）
- P1+ 啟動時可以一次把 unlock 鏈完整實作，不需要先補丁再改架構

### Decision 2：Rule explainer 從 sanitizer rules registry 拉，不從 audit log 拉

`sanitize_audit.jsonl` row 只記 `rule_id` 字串（如 `aws_access_key`、`email_rfc5322`），沒記 description / pattern。Inspector 要顯示「這是什麼規則」必須另外取。

**替代方案 A**：把 rule description / pattern 也寫進 audit row —— 違反 D-015 line 371「`sanitize_audit.jsonl` 記類別數量，不記原文」精神（pattern 雖然不是「使用者原文」，但是 sanitizer rule 的內部結構，audit log 應該保持薄）+ 增加 audit log 體積（規則庫變大時影響顯著）。

**替代方案 B**：前端把 rules registry 寫死成 lookup table —— `~/.codebus/sanitizer.local.yaml` 是使用者本地擴充（D-015 line 364），前端寫死無法反映本地規則；違反 single-source-of-truth。

**選定方案**：sidecar 新增 `GET /sanitizer/rules` 唯讀 endpoint，回傳當前 effective rules registry snapshot（內建 + 使用者 yaml 合併後）。前端 `useSanitizerRules` composable 在 inspector mount 時呼叫一次、cache 整個 session（rules 在 sidecar process 生命週期內 immutable，安全 cache）。

Endpoint 形狀（最小可用）：

```
GET /sanitizer/rules
→ 200 OK
{
  "rules_version": "<semver>",
  "rules": [
    {
      "rule_id": "aws_access_key",
      "kind": "secret",
      "description": "AWS access key (static credential)",
      "pattern_summary": "AKIA[0-9A-Z]{16}",  // 人類可讀摘要，非完整 regex
      "source": "builtin"  // "builtin" | "user_yaml"
    },
    ...
  ]
}
```

`pattern_summary` 是「易讀化」字串，非執行用 regex，避免使用者誤把它當編譯規則用。完整 regex 可在未來決定是否暴露（mockup v0 line 446 有 toggle，本 P0 先不做）。

### Decision 3：對齊 28.5 的 select-row + parent-hosts-overlay pattern

`AuditPanel.vue` 的 `select-row` emit 在 28.5 已經是 dumb display surface 契約（既存 `frontend-shell` Requirement 「row-click → overlay 為 parent-layer concern」）。本 change 不擴展 `AuditPanel`，只在 R-01 station page、Explorer console page、`/audit/sanitizer` standalone page 三個 parent 注入 `<SanitizerAuditInspector>` 對 `sanitize` tab 的 select-row 監聽，與 28.5 的 `<LlmCallInspector>` 對 `llm` tab 完全對稱。

`AuditPanel.vue` 的唯一變動：在 `sanitize` tab 的 row template 補 placeholder kind chip（用 `purple` token），row body 顯示 `<REDACTED:kind#index>` + `pass` 1-2-3 chip + `source` 縮寫。chip 顏色嚴格收 `purple`（既存 `frontend-shell` 不變式：Purple 保留給 sanitizer / privacy 語意）。

### Decision 4：D-015 banner 文字逐字定稿

避免實作期反覆 wording。Banner 顯示位置：inspector overlay header、`/audit/sanitizer` 頁頂、AuditPanel `sanitize` tab 頂部 sticky。三處同字串（單一文案來源，i18n 後也好維護）：

```
Audit metadata only · raw values are not retained per D-015.
Placeholder reveal requires a future audit-unlock capability.
```

對齊 28.5 LLM Call Inspector 的 D-015 banner 風格（`llm-call-inspector` spec 既有「always shows post-sanitize wire payload, not pre-sanitize raw」公告）。

### Decision 5：`useSanitizeAudit` composable 邊界

不直接從 Tauri IPC 讀 `sanitize_audit.jsonl` —— 那是 28.5 既有 `useAuditJsonl('sanitize')` 的責任。`useSanitizeAudit` 是 thin wrapper，只負責：

- 解析每筆 row 的 `source`（dict / 字串兩型統一成 `{ pass: 'scanner' | 'provider' | 'add_to_kb' | null, label: string, path: string | null, message_id: string | null }` view-model）
- 計算 `kind: count` summary
- 按 `session_id` 分組產 timeline
- 計算 placeholder identifier `<REDACTED:kind#index>` 字串供 chip 顯示

不負責：read JSONL、live-tail（28.5 `useAuditJsonl` 已處理）、`rules_version` lookup（`useSanitizerRules` 處理）。職責邊界對齊 28.5 `useLlmCalls` 對 `useAuditJsonl('llm')` 的關係。

## Risks / Trade-offs

- **[Risk] 使用者看到 metadata 不夠，仍想看 raw → 解鎖機制誘惑**：本 P0 顯眼 banner 寫明「reveal requires future audit-unlock capability」，但若使用者期望落差大可能影響 demo 評估。
  → **Mitigation**：banner 配合 mockup v0 的「audit metadata only」徽章（lock icon + 文字），demo 腳本明確把「P0 守 D-015、P1 啟動 unlock 鏈」當賣點之一（「我們的不變式不為 demo 視覺鬆動」）。

- **[Risk] `GET /sanitizer/rules` endpoint 引入新 sidecar API surface**：新 endpoint 即新攻擊面，且需要 bearer + loopback gate。
  → **Mitigation**：endpoint 嚴格唯讀、無參數、不接觸 audit log、不接受 user-controlled 路徑，只 dump rules registry snapshot。對應測試 `sidecar/tests/api/test_sanitizer_rules.py` 必須覆蓋（a）bearer 缺失 401、（b）非 loopback 連入 403（如有 middleware）、（c）rules_version 與 sanitizer engine 內部一致。

- **[Risk] Rules registry 在 sidecar process 生命週期內 immutable 的假設**：使用者改 `~/.codebus/sanitizer.local.yaml` 後不重啟 sidecar 會看到舊 rules。
  → **Mitigation**：D-015 line 364 已規定 yaml 為 user-local config，且 `docs/sanitizer.md §六` 規定 yaml 變更 → bump rules_version → 使用者依版本重取同意。重啟 sidecar 在這個流程裡是預期行為（不重啟也不會用到新 rules，因為 SanitizerEngine 也是同樣的 cache）。前端 cache rules 整個 session 是與 sidecar 行為一致的。

- **[Trade-off] 不支援 rule pattern 完整 regex 顯示**：mockup v0 有 regex toggle（line 446、468、509），本 P0 只暴露 `pattern_summary`。
  → **Trade-off**：完整 regex 對使用者除錯有價值，但會放大 rules registry 體積、且 regex 對非工程使用者意義有限。P1 視回饋決定是否補。

- **[Trade-off] `extra.allowlisted` 是目前 schema 唯一的 extra 欄位**：未來 sanitizer 演進可能加新 extra 欄位（例如 LLM-based PII 偵測的 confidence score），inspector 渲染時若死寫 `allowlisted` 一欄會無法平滑接住。
  → **Mitigation**：spec 規範 `extra` 渲染為「key: value 列表」泛用形式，遇到 `allowlisted` 特化 chip（綠色 ✓ 標記），其他 key 走預設 mono key-value row。
