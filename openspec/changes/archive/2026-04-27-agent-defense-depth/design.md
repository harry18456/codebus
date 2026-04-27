## Context

Stage 5 Review #2（`docs/reviews/2026-04-26-stage-5.md`）Cat 2 列出 28 條 spec drift；其中 4 條（D2.12 / D2.14 / D2.15 / D2.19）必須改 production code + 補 test + 改 spec 才能收尾，無法走純 wording cleanup。本 change `agent-defense-depth` 把這 4 條一次收，避免拆 4 個 micro-change 重複 propose / apply / archive overhead。

**現況痛點**：

1. **D2.12**：`POST /kb/build` 走 200，但 `/explore` `/generate` `/qa` 全 202。前端 SSE 輪詢若用 exhaustive `if status === 202` 分支，會在 KB build 時走 fallback 路徑，徒增前端 if-else
2. **D2.14**：`agent/tools/folder_tools.py:450,630` 的 `read_file` / `find_callers` 把 `MessageSource(message_id="…")` + `pass_num=1` 寫進 `sanitize_audit.jsonl`。這違反 `docs/sanitizer.md §三` 的設計：「Pass 1 是 file-source（Scanner 入 KB 前 / Explorer read_file 等讀檔工具），Pass 2 是 message-source（Provider pre-flight 把 prompt 中的 chat message 過 sanitize）」。Trust Layer R-01 panel 預期按 source.type 分組顯示「檔案 redaction」vs「對話 redaction」會誤判
3. **D2.15**：`_search_via_grep`（`agent/tools/folder_tools.py:324-374`）的 fallback path 對 grep 命中的 snippet **直接回傳**，無 sanitize；KB path 因 Scanner Pass 1 已 sanitized。`SearchHit.snippet` 在沒命中 KB 時繞過防禦深度。violation 不變式 #3「LLM 看到的一定是 Sanitize 過的」（snippet 透過 ReAct loop 進入 LLM context）
4. **D2.19**：`agent/explorer.py:187,197` 兩處 error path `output=f"ERROR: {msg}" / output=f"ERROR: {exc}"` 把原始 exception message 寫進 `ToolResult.output`。若 tool args 裡含使用者 input（`read_file(path="C:/secret.env")` 的 path、`search(keyword="api_key=sk-…")` 的 keyword）就直接洩漏到下一輪 LLM context — Pass 2 sanitize 把不到這條 path

**Stakeholders**：

- Phase 6 前端（步驟 28 R-01 Agent console）— 直接消費 `ToolResult.output` + `sanitize_audit.jsonl`
- Phase 6 R-01 audit panel — 按 source.type 分組顯示稽核行（D2.14）
- 評審 demo — Trust Layer 敘事「LLM 看到的一定是 sanitized」是核心賣點，不能被任何一條 fallback / error path 繞過

## Goals / Non-Goals

### Goals

- 4 條 D2.x 全部以 minimal-diff 修法收尾，零新功能引入
- production code 改動範圍鎖在 3 個檔案（`api/kb.py` / `agent/tools/folder_tools.py` / `agent/explorer.py`）+ 4 個新 test 檔
- 5 個 capability spec MODIFIED Requirement 對齊改後 code 真值
- baseline 843 passed → 預期 +4 ~ +6（每條 D2.x 至少 1 新測）

### Non-Goals

- 不重構 sanitizer engine（現有 `SanitizerEngine.sanitize` API 不變）
- 不擴 sanitizer rules version（rules pattern 不動，rules_version 不 bump）
- 不引入新 SSE event type（D2.19 sanitize 後的 error 字串走既有 `agent_action_result.output` 欄）
- 不改前端任何檔（前端配合在 Phase 6 步驟 28 動工）
- 不改 KB build / query / Q&A 的 sanitize 行為（D2.15 只補 grep fallback path，不重做已 sanitized 的 KB path）

## Decisions

### Decision 1: D2.19 error path 走 Pass 2 sanitize（vs 其他三選項）

**選項**：

A. **Pass 2 sanitize 完整 error string**（採用）— 把 `f"ERROR: {msg}"` 整段過 `SanitizerEngine.sanitize` Pass 2，hits 寫 `sanitize_audit.jsonl` `pass_num=2` `source=MessageSource(message_id=f"explorer_step_{step_idx}_error")`
B. **Pass 1 sanitize**（拒絕）— Pass 1 是 file-source 設計，error message 不是檔案內容，違反 D2.14 修的不變式
C. **safe-list error message**（拒絕）— 維護 safe pattern list 隨 sanitizer rules drift，且 Python exception type 多樣（`OSError` / `PermissionError` / `ValueError` / 自訂）safe-list 無法窮舉
D. **完全不放 error 進 output**（拒絕）— 把 error 變成「無資訊」對 ReAct loop 自我修正能力傷害大；Agent 看不到「我剛剛為什麼失敗」就無法調整下一步

**理由**：error string 屬於 LLM 即將消費的 message-channel 內容（會被 `_observe` 寫進 `state.messages` 餵下輪 `_think`），與 Pass 2「Provider pre-flight」的設計初衷一致。`MessageSource(message_id=f"explorer_step_{step_idx}_error")` 讓 Trust Layer R-01 panel 可定位「第幾步的 tool error」。

### Decision 2: D2.14 source type invariant 寫進 `sanitizer` capability cross-cutting Scenario

**選項**：

A. **只在 `explorer-tools` capability 鎖**（拒絕）— D2.14 的根本問題是 source type 與 pass_num 對應的 cross-cutting invariant；只鎖 explorer-tools 後續 Module 5 / Module 8 加新 file-reading tool 容易再犯
B. **在 `sanitizer` capability 加 cross-cutting Scenario**（採用）— `sanitizer` capability 已是 audit logger 的權威，把不變式寫成 Scenario `pass_num to source-type invariant`：「`pass_num=1` MUST 對應 `source.type=="file"`，`pass_num=2` MUST 對應 `source.type=="message"`，`pass_num=3` MUST 對應 `source.type in {"file", "message"}`（Q&A `add_to_kb` 兩種都可能）」
C. **加 production-side runtime guard**（拒絕）— `SanitizerAuditLogger.append` 內加 assert 會在 production 端拋；過 sanitize 的 hot path 不該加防呆 assert，放 spec scenario + 新 defensive test 抓即可

**理由**：違反此不變式的 callsite 不多（目前只 D2.14 兩處），用 spec Scenario + 新 test grep 已足。production hot path 加 runtime check 對 P0 無收益。

### Decision 3: D2.15 grep snippet sanitize 不 cache

**選項**：

A. **每次 grep 命中都跑 sanitize**（採用）— 簡單、無 cache invalidation 風險。grep fallback 路徑本來就不熱（KB path 命中後就不走 grep），sanitize cost 可接受
B. **建 file-level snippet cache**（拒絕）— 引入 cache 後第一次寫得對、後續因 sanitizer rules version bump 後 cache 失效漏掉 — 不變式 #9（rules bump 必重取）的 invariant 翻車

**理由**：sanitize 是 pure 純算法、無 IO；對 grep 結果（≤100 hits / fallback 才走）的 overhead 可忽略。Cache 引入的隱性風險遠高於收益。

### Decision 4: D2.12 status code 改點選 endpoint 端、不動 task registry

**選項**：

A. **endpoint 端加 `status_code=status.HTTP_202_ACCEPTED`**（採用）— FastAPI router decorator 一行
B. **task registry 端統一返 202**（拒絕）— task registry 已 endpoint-agnostic，不該感知 HTTP status code

**理由**：FastAPI 慣例是 endpoint 端宣告 response status；task registry 是純 background work 抽象。

## Risks / Trade-offs

- **[D2.19 sanitize cost]**：error path 多一次 sanitize 呼叫，但 error path 是 cold path（非 happy path），cost 可忽略 → 接受
- **[D2.15 sanitize 改變 SearchHit.snippet]**：改後 grep 命中含密鑰時 snippet 出現 `<REDACTED:>` placeholder。對 LLM 端是好事（不外洩），但測試 fixture 若有 hard-code expected snippet 字面量會破 → Mitigation: 新測用 placeholder regex assertion，不硬寫 snippet 文字
- **[D2.12 改 200 → 202 是 wire-format breaking]**：但目前前端尚未 ship `/kb/build` consumer（Phase 6 步驟 28 才會接），此時改 zero blast radius → 接受
- **[D2.14 改 source type 影響稽核 join]**：若 R-01 panel 已 ship 並按 `source.message_id` 抓 read_file Pass 1 行 → 該 panel 也尚未 ship，前端工尚未動工 → 接受

## Migration Plan

不需要 migration — 純 production code drift 修正：

1. baseline 跑 `uv run pytest sidecar/tests/` 確認 843 / 19
2. 4 條 D2.x 各自 TDD（先寫 failing test，再改 code 通過）
3. 改 5 個 capability spec MODIFIED Requirement
4. final pytest 預期 ~847 / 19
5. 改 docs/reviews/2026-04-26-stage-5.md Cat 2 4 條 checkbox + 進度表 row
6. 改 CLAUDE.md archive 表加 row

無 schema 變動、無 SSE event type 變動、無 sanitizer rules version 變動 — rollback 只需 `git revert`。

## Open Questions

- **D2.14 既有 `MessageSource` 寫入是否該保留為「兼容 alias」**？答：不保留。`sanitize_audit.jsonl` 是 append-only，舊行已落盤的事實不可改；新行從本 change archive 後一律走 `FileSource`。R-01 panel 若要顯示舊行，按時間軸顯示「2026-04-27 之前的 read_file/find_callers Pass 1 行可能是 message-source」即可。decisions.md 不必新增 ADR（屬實作細節，非架構決策）
- **D2.19 sanitize 後 error string 仍含 placeholder（如 `<REDACTED:secret#7>`）**，下輪 `_think` 看到 placeholder 是否會誤導？答：不會。Agent 已習慣 sanitize placeholder（read_file 的 file content 也會出 placeholder），prompt template 在 `agent/prompts/explorer.py` 已寫「`<REDACTED:>` markers indicate redacted secrets — do NOT attempt to derive their values」
