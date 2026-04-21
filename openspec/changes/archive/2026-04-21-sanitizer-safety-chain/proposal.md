## Why

關聯 D-011（資安與合規）、D-015（三段式 Sanitizer）。依 `docs/implementation-plan.md §一` 的五條強制規則，**Sanitizer Pass 1 + 2 必須在第一次 LLM call（步驟 16 Explorer ReAct）之前**落地——一旦 code / doc 進入 LLM 呼叫鏈而未經 sanitize，未去識別化的 secret / PII / 內部識別符就有機會寫進 `llm_calls.jsonl` 並離開本機；補做要改每個 call site，retrofit 成本極高。M1 archive 已備妥 LLMProvider Protocol、TrackedProvider、LLMCallLogger 骨架，但 Pass 2 pre-flight 掛點尚未接上、Sanitizer 本體尚未實作、`sanitize_audit.jsonl` 與 `tool_audit.jsonl` 兩條稽核管線仍缺。本 change 對齊 `docs/implementation-plan.md §二` 第二階段（步驟 9-12，~3d）與 `docs/sanitizer.md §九` P0 全部項目，在 Module 1 Scanner 啟動前補齊安全鏈。

## What Changes

- **新增 `sanitizer` capability**：實作 `SanitizerEngine`，支援 Secret（`detect-secrets` 整合）、PII（email / 台灣手機 / 台灣身分證 regex）、內部識別符（RFC1918 / RFC4193 / link-local IP、`.local` / `.internal` / `.corp` / `.lan` TLD）三類偵測；替換為 `<REDACTED:kind#index>` 佔位符（index 以單檔 scope 累增，跨檔不共用）；無 reverse mapping，原值不儲存。
- **新增 `Pass 1`**：Scanner 入 KB 前逐檔掃描，輸出清理版 chunk 給 Module 2 embed；原始檔不 copy、原處不動。
- **新增 `Pass 2` Provider pre-flight**：所有 `LLMProvider.chat` / `embed` 在 dispatch 前先過 Sanitizer；`llm_calls.jsonl` 記錄 post-sanitize 版本；M1 既有 `sanitizer_pass2_applied` 欄位翻為 `true`（原型別 / 語意不變）。
- **新增 `sanitize_audit.jsonl` logger**（workspace-level）：記錄每次替換的 `rule_id` / `kind` / `placeholder_index` / `source`（Pass 1 / Pass 2 / Pass 3）/ `file_or_message_id` / `line_or_position` / `pass_session_id`；**不含原值、不含周圍 context**；JSONL append-only 寫入 `{workspace}/.codebus/sanitize_audit.jsonl`。
- **新增 `tool_audit.jsonl` logger**（workspace-level，`tool-sandbox` 修訂）：ToolSandbox 每次工具呼叫寫一行，含 `tool_name` / `workspace_type` / `resolved_path`（僅當路徑成功過 `ensure_in_workspace`）/ `allowed`（bool）/ `denial_reason`（若 deny）；JSONL append-only。
- **新增 Sanitizer config schema**：Pydantic model 覆蓋 `~/.codebus/sanitizer.local.yaml` 與 `{workspace}/sanitizer.local.yaml`（workspace 覆蓋全域）；欄位含 `rules_version`（語意版號字串，配合 D-015 rule pattern 改動必 bump）、`path_allowlist` / `filename_allowlist` / `pattern_allowlist`（MVP 結構先定義、執行 MVP 內僅必要子集——見 Non-Goals）。
- **不變式守護**：Sanitizer 不自行儲存原值；placeholder 單向；`llm_calls.jsonl` 永遠記 post-sanitize；rules 改動 bump version（由使用者授權層在後續 change 讀取）。

## Non-Goals (optional)

- **不做** 中文姓名 / 信用卡 / 地址 / 員工編號偵測（需人工清單或 ML，誤殺高）。
- **不做** ML-based PII（Presidio），依賴重，延後評估。
- **不做** Reverse mapping（placeholder → 原值），違反單向不變式。
- **不做** 高熵 suspect 等級 UI review（P2），MVP 只到 detect-secrets 內建的 pattern 層偵測。
- **不做** 稽核報告 UI（P1）— 本 change 只做 JSONL 落盤，前端展示留到 Trust Layer R-01 / O-05 mockup 實作的 change。
- **不做** 首次授權 modal（O-01；P1）— rules version 讀取 / 使用者重授權由後續 `authorization` 相關 change 承接；本 change 僅定義 `rules_version` 欄位，不接 UI。
- **不做** 檢疫區（quarantine）機制（P1）— MVP 失敗處理採 fail-closed（Sanitizer 丟 exception 則 abort 該次 Pass，caller 決定整體行為），複雜檢疫流程延後。
- **不做** Sanitizer 規則熱更新（要求改 config 重啟 App）。
- **不做** Pass 3 Q&A `add_to_kb` 寫入前的 sanitize 掛點 — 依賴 Module 8 Q&A Agent（步驟 25），本 change 結束時 Pass 3 插槽留白但 `SanitizerEngine` 介面已可複用。

## Capabilities

### New Capabilities

- `sanitizer`: 三段式 Sanitizer 的 Pass 1（Scanner 入 KB 前）與 Pass 2（Provider pre-flight）實作、`<REDACTED:kind#index>` 佔位符規則、`sanitize_audit.jsonl` 稽核管線、`sanitizer.local.yaml` config schema + 載入 / 驗證。

### Modified Capabilities

- `llm-provider`: 新增「所有 provider 呼叫在 dispatch 前經 Sanitizer Pass 2」需求；M1 既有 `sanitizer_pass2_applied` 欄位在 Pass 2 執行後應為 `true`。
- `tool-sandbox`: 新增「ToolSandbox 每次工具呼叫寫入 `tool_audit.jsonl`」需求，欄位含 `tool_name` / `workspace_type` / `resolved_path` / `allowed` / `denial_reason`。

## Impact

- Affected specs:
  - 新增 `openspec/specs/sanitizer/spec.md`
  - 修訂 `openspec/specs/llm-provider/spec.md`（Pass 2 pre-flight 需求）
  - 修訂 `openspec/specs/tool-sandbox/spec.md`（tool_audit.jsonl 需求）
- Affected code:
  - 新增 `sidecar/src/codebus_agent/sanitizer/`（`engine.py` / `rules.py` / `config.py` / `audit.py` / `__init__.py`）
  - 修訂 `sidecar/src/codebus_agent/providers/tracked.py`（Pass 2 hook in chat / embed）
  - 修訂 `sidecar/src/codebus_agent/sandbox.py`（寫 `tool_audit.jsonl`）
  - 新增 `sidecar/tests/sanitizer/`（unit + integration；含合成 secret / PII fixture）
  - 修訂 `sidecar/tests/providers/test_tracked_provider.py`（Pass 2 wiring + `sanitizer_pass2_applied` 欄位翻 true 驗證）
  - 新增 `sidecar/tests/sandbox/test_tool_audit.py`
- Affected deps:
  - 新增 `detect-secrets`（Yelp；Python）— sidecar `pyproject.toml`
- Affected docs:
  - 待 archive 時同步 `docs/sanitizer.md` 的 rules version 欄位定義（由 archive 流程處理，非本 change scope）
