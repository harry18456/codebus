## Context

**M1 基建現況**（`openspec/specs/` + M1 archive）：
- `TrackedProvider` 裝飾器已接 `UsageTracker`（`token_usage.jsonl`）與 `LLMCallLogger`（`llm_calls.jsonl`）；欄位 `sanitizer_pass2_applied` 已存在，M1 預設寫 `false`。
- `ToolSandbox` 的 `ensure_in_workspace(path, ctx)` 已實作（`sidecar/src/codebus_agent/sandbox.py`），紅隊 fixture 在 `sidecar/tests/sandbox/`；但尚未落 `tool_audit.jsonl`。
- Sidecar FastAPI 只 bind `127.0.0.1:0`（ephemeral）、bearer token 記憶體常駐，所有 endpoint 過 bearer middleware。
- Registry guard 在實例化階段拒絕未被 `TrackedProvider` 包裹的 provider；zero outbound 不變式靠 `respx` / socket patch 守門。

**下游觸發點**：
- 步驟 13 Module 1 Scanner 產 `ScanResult` → 步驟 14 Module 2 KB Builder 吃 chunk → **Pass 1 必須介於兩者之間**（Scanner 把檔案 chunk 後、Module 2 embed 前）。
- 步驟 16 Explorer ReAct 第一次呼叫 `LLMProvider.chat` → **Pass 2 必須在 dispatch 之前**。
- 步驟 17 第一個真工具（search / list_dir / read_file / mark_station）→ **tool_audit.jsonl 必須同步寫入**。

**參考文件**：`docs/sanitizer.md` §三（三段式觸發點）、§四（placeholder / Agent 理解 / 白名單）、§五（config schema）、§六（失敗處理）、§八（效能與測試）、§九 P0 項目；`docs/security.md` §四（稽核 JSONL 格式）；`docs/decisions.md` D-011 / D-015 / D-021 / D-022；`docs/implementation-plan.md` §二 第二階段。

## Goals / Non-Goals

**Goals:**

- 在任何 LLM call 離開 sidecar 之前，Pass 2 已不可繞過——以 `TrackedProvider` 為唯一 choke point，避免任何 provider 實作者漏接。
- Pass 1 為純函式介面，Module 1 Scanner 與 Module 8 Q&A `add_to_kb`（Pass 3 日後）都能共用同一個 `SanitizerEngine` 物件。
- `sanitize_audit.jsonl` / `tool_audit.jsonl` 的 schema 是穩定附加式（append-only），欄位只加不改；M2+ 延伸欄位以新增 key 而非更動語意。
- Sanitizer 失敗採 fail-closed：engine 丟 exception 時 caller 必須中止該次 Pass，不得 fallback 送原文。
- `rules_version` 語意版號從 Day 1 就記錄在 `sanitize_audit.jsonl` 每行；後續 `authorization` change 讀取版號觸發重授權。

**Non-Goals:**

- 不在本 change 做稽核 UI、授權 modal、rules 熱更新。
- 不在本 change 接 Pass 3（Q&A `add_to_kb`）— engine 介面為 Pass 3 留插槽，實作在 Q&A change。
- 不在本 change 做高熵 suspect 等級的使用者 review 回饋（`flagged` 欄位 MVP 恆 0）。
- 不在本 change 寫跨 workspace `rule_stats` 累計（per-session counter 僅 in-memory）。

## Decisions

### Pass 2 hook point — TrackedProvider 裝飾器層，而非 base LLMProvider

**選擇**：將 Pass 2 sanitize 邏輯放在 `TrackedProvider.chat` / `embed` 的 `_pre_dispatch` 階段，緊接在 `role` 驗證之後、底層 `self._inner.chat(...)` 之前。`sanitizer_pass2_applied=True` 與 `llm_calls.jsonl` 的 post-sanitize payload 由同一層寫入。

**理由**：
- 不變式「所有 provider 必包 TrackedProvider」由 registry guard 強制（M1 已落地），hook 在 TrackedProvider 即等於覆蓋所有 provider。
- base `LLMProvider` Protocol 只定義 shape、不含行為；塞 sanitize 會把資料規則耦合進 protocol，破壞 D-003 抽象。
- 裝飾器層也是 `UsageTracker` / `LLMCallLogger` 現有 wiring 所在，集中 pre-dispatch pipeline 便於測試。

**Alternatives rejected**：
- 放 registry `get(role)` 層 — 會讓 engine 依賴 role，但同一訊息在不同 role 應走相同規則，engine 不應 role-aware。
- 放各 provider 實作內 — 容易漏接、違反 DRY、無法一處測試完整。

### sanitize_audit.jsonl schema — 固定 10 欄位 + `extra` 擴充欄位

**選擇**：每行 JSON 固定欄位：
```
{
  "ts": "2026-04-20T12:34:56.789Z",
  "schema_version": 1,
  "rules_version": "2026-04-20-1",
  "pass": 1,                         // 1 | 2 | 3
  "session_id": "<uuid4>",           // 跨單次 pass 關聯
  "source": "file:src/app.py" | "message:chat_req_<id>",
  "rule_id": "pii_email_v1",
  "kind": "email",
  "placeholder_index": 1,
  "extra": {}                        // 預留擴充；M2+ 可加 "subtype" / "chunk_offset" 等
}
```
**不含**：原值、替換後字串、周邊 context、檔案完整路徑以外的資訊。

**理由**：
- 稽核鏈的核心價值是「可審」，而可審要求 schema 穩定。版號加 `schema_version` 與 `rules_version` 雙欄位讓 M2+ 能同時 bump 兩者而不破舊 consumer。
- `extra: {}` 讓小幅擴充不需要 schema bump；`subtype: "high_entropy_suspect"` 等將來走 extra。
- `source` 走 prefix + id 形式（`file:` / `message:`）而非兩個欄位，避免 Pass 1 / 2 / 3 欄位差異。

**Alternatives rejected**：
- 存 before/after diff → 破壞「原值不儲存」不變式。
- 每 Pass 一個獨立 JSONL → 多檔 I/O 複雜，且跨 Pass 關聯（session_id）難做。

### tool_audit.jsonl schema — 呼應 ToolSandbox `ensure_in_workspace` 結果

**選擇**：每行 JSON 固定欄位：
```
{
  "ts": "2026-04-20T12:34:56.789Z",
  "schema_version": 1,
  "workspace_type": "folder",       // D-002 雙模 discriminator
  "tool_name": "read_file",
  "args_summary": {"path": "src/app.py"},   // 只記 shape 不記原值
  "resolved_path": "D:/proj/src/app.py",    // 僅當 allowed=true
  "allowed": true,
  "denial_reason": null,            // 若 false，例：path_escape | symlink_outside | unc_path
  "session_id": "<uuid4>"
}
```

**理由**：
- `args_summary` 的語意是「參數 shape + 鍵名」，不逐鍵寫原值（避免把 `search` query 原文寫入 audit — 那是 Pass 1 沒掃到的地方）。具體「哪些 key 進 summary」由各 tool 註冊時宣告，tool-sandbox spec 定介面即可。
- `denial_reason` 用 closed enum，便於前端審計頁分類統計。
- `workspace_type` 日 1 必入，配合 D-002 雙模不變式，M2 Topic mode 落地時不用改 schema。

### Sanitizer config — 兩層覆蓋 + Pydantic strict 驗證

**選擇**：
```
~/.codebus/sanitizer.local.yaml          # 全域預設
{workspace}/sanitizer.local.yaml         # workspace 覆蓋（整份覆蓋，非 merge）
```
載入順序：若 workspace file 存在，讀它；否則 fallback 全域；兩者都不存在時使用 built-in defaults。Pydantic model 採 `model_config = ConfigDict(extra="forbid")` 強制嚴格 schema，未知欄位 raise。`rules_version` 為必填字串，格式 `YYYY-MM-DD-N`（由維護者 bump）。

**理由**：
- 整份覆蓋比 deep merge 簡單，使用者心智一致（看 workspace file 就知規則）；MVP 夠用。
- Strict 拒絕未知欄位可早期捕捉 typo；對 MVP 使用者（開發者）友善。
- `rules_version` 為字串而非 semver，讓 rules 小改（如新增一條 PII regex）也能 bump，避免 semver 語意爭論。

**Alternatives rejected**：
- Deep merge：MVP 過度工程，且讓使用者難以預測最終規則集。
- `extra="allow"`：容忍 typo，debug 成本高。

### Placeholder index — 單檔 scope、session-less、in-memory

**選擇**：engine 在單次 Pass 1 呼叫內維護 `{(kind, original_value): index}` dict；Pass 1 每檔 reset。Pass 2 每次 message batch reset。**跨檔不共用**。

**理由**：
- 跨檔共用會建立檔案間敏感資料關聯（即便是 placeholder），違反資訊最小化原則。
- Per-file / per-message scope 讓「同檔同 email 變同一個 `#1`」成立（agent 推理需要），同時避免跨檔 leak。

### Fail-closed 失敗處理

**選擇**：`SanitizerEngine.sanitize(text) -> SanitizedResult` 在遇到不可預期錯誤（regex 異常、detect-secrets crash、placeholder index 溢出等）時 raise `SanitizerError`；caller 必須：
- Pass 1（Scanner）：跳過該檔、寫 `sanitize_audit.jsonl` 一行 `pass=1, source=file:..., extra={"error": "sanitizer_failed"}`、Scanner 標記該檔為 `errored` 不進 KB。
- Pass 2（TrackedProvider）：中止該次 LLM call、raise 給 Agent 層；`llm_calls.jsonl` 不寫（因為根本沒 call 出去）；Agent 可決定重試或報錯。

**理由**：fail-open 會讓錯誤 case 直接送原文到 LLM，違反安全鏈核心不變式。寧可誤殺（整個檔跳過）也不要誤送。

**Risk**：Pass 2 fail-closed 可能讓使用者在 demo 時看到 Agent 中斷。**Mitigation**：engine 本體只做純 regex / detect-secrets 呼叫，失敗面極窄；並搭配 unit test 覆蓋已知 crash pattern。

## Risks / Trade-offs

- [**detect-secrets 首次掃描慢**（`.git/` pack 檔 / 大 binary 可能 100+ ms）] → MVP 已由 Module 1 Scanner 的 binary/encoding 過濾前置擋掉；Sanitizer 只收 text chunk。`docs/sanitizer.md §八` 效能目標 <50 ms / chunk，超標時 rules 進入「停用評估」而非 fallback。
- [**placeholder index 在同檔大量重複命中會讓 index 爆炸**（如 log file 有 10k 個 email）] → 單檔 scope 內 index 用 `int`，Python int 無上限；但 audit JSONL 會暴增。**Mitigation**：MVP 不處理，靠 Module 1 scanner 的檔案大小上限；post-MVP 可加 per-rule per-file 上限截斷。
- [**Pass 2 sanitize 後 message 變短，LLM context window 使用更保守**] → 實務上 placeholder 短於原值（`<REDACTED:email#1>` < `verylongname@example.com`），M2 若遇 truncation 再議。
- [**tool_audit.jsonl 與 `args_summary` 的「哪些 key 寫 summary」由各 tool 宣告**，新增 tool 時可能忘記設] → 在 `ToolSandbox` 註冊介面加 `audit_fields: list[str]` 必填欄位，未設即 raise；由 spec 強制。
- [**workspace 覆蓋整份 config 而非 merge**，使用者在 workspace 忘記 copy 全域的 allowlist] → MVP 接受；config 載入時在 sidecar startup log 明確印出「使用 workspace config / 使用全域 config」讓使用者可見。
- [**rules_version 不 bump 但改 rule pattern**] → 由 `docs/sanitizer.md §十一` 的 rule 維護流程規範；本 change 的 `rules_version` 必填讓稽核鏈至少有版號可追，實際治理靠後續 `authorization` change 強制重授權。

## Migration Plan

- **無既有資料需遷移**：M1 archive 尚未有任何 `sanitize_audit.jsonl` 或 `tool_audit.jsonl` 檔案；本 change 是新增。
- **既有 `llm_calls.jsonl` 欄位 `sanitizer_pass2_applied`**：M1 archive 時為常數 `false`，本 change 完成後自動翻 `true`；consumer（前端 LLM Call Inspector，尚未實作）尚無相容問題。
- **回滾**：若此 change 被 revert，`TrackedProvider` 的 Pass 2 hook 移除、`sanitize_audit.jsonl` / `tool_audit.jsonl` 產出停止；已寫的 JSONL 檔保留不刪。因為不動 schema 現有欄位，降級安全。

## Open Questions

- **暫無**：所有決策點已在 Decisions 章節敲定。若 apply 階段遇到 `detect-secrets` API 與 Instructor / Pydantic 版本衝突，回到本 design 新增 Decision 並同步 `docs/decisions.md` 新 D-XXX 再進。
