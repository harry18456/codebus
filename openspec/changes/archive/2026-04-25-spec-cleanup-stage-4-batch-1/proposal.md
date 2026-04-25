## Why

`docs/reviews/2026-04-25-stage-4.md` 第一輪 review（commit `fd88031`）抓到 6 條 spec / code 不一致，全集中在 Stage 1-4 backend 的 archived spec。剛 archive 的 `golden-sample-baseline` 在落地過程也踩到兩處同類疏忽（`budget_warning` 邊界 + `should_follow_imports` 互斥），驗證了「spec 先 land、code 跟上」的潛伏 bug 風險。本 change 是 Cat 2 收尾 — 把 review 抓到的 6 條全部修掉，讓 Stage 5（Module 5 / 8）動工時不會繼承錯誤 spec。

對齊 `docs/reviews/2026-04-25-stage-4.md` Category 2 列表 + codebus 慣例「main spec 改動必走 Spectra ceremony」（CLAUDE.md 不變式段尾）。

## What Changes

**A. `usage-tracking` spec — 移除 M1 過時 wording**

Scenario `Sanitizer-ready field reserved` 仍寫「During M1 this field MUST be `false`」，但 `sanitizer-safety-chain` archive（2026-04-21）後 production 永遠 `true`、`llm-provider` spec Scenario `sanitizer_pass2_applied field set to true` 也已要求 true。兩 spec 互相衝突，本 change 把 M1 wording 刪除、保留欄位存在性與型別約束。

**B. `explorer-sse` spec — 合法化 `tokens_used: 0` P0 placeholder**

Requirement `Explorer loop emits agent_thought / agent_action_result / judge_verdict events` 寫 `agent_action_result` 帶 `"tokens_used": <int>`。Production `explorer.py:494` hardcoded `0`，內註 P0 placeholder。`<int>` 形式上允許 0，但 spec 沒解釋 placeholder 性質，未來 Module 5 接前端時容易被當 bug 修。本 change 加一句「P0 MAY emit 0 as placeholder; per-tool attribution lands when ToolResult carries `tokens_used`」+ Scenario。

**C. `agent-core` spec — 兩條澄清**

C-1. `ReasoningLogger appends one JSONL line per Step` Requirement：加 Scenario 明寫 prompt version stamp 是 logger 唯一寫入路徑，callers MUST NOT pre-stamp。動機：`explorer.py:601-603` coverage round Step 顯式塞 `explorer_prompt_version` / `judge_prompt_version`，但 `ReasoningLogger.write` 已用 `model_copy` 覆寫 — 兩處寫入容易漂移。Spec 鎖死 single source of truth。

C-2. `ReAct loop executes think-act-observe-judge-log-update each iteration` Requirement：加 Scenario 明寫 P0 Update step 在 `should_follow_imports=True` 時 `pending_queue.append(tool_name)` 是 placeholder 行為（tool_name 字串本身與後續工具語意無關，只用來 keep queue non-empty 以避開 `_MIN_STATIONS_FOR_CONVERGENCE=3` 的 `queue_empty` 提前停）。動機：spec 沒明寫 push 什麼進 queue，導致 `golden-sample-baseline` 寫測時踩到（5 iter run 與 `should_follow_imports=False` 互斥）。Spec 把 P0 placeholder 行為合法化，未來真符號路徑由 `explorer-tools-p2` `follow_reference` 落地。

**D. `sidecar-runtime` spec — Background task error containment 擴 `/explore`**

Requirement `Background task error containment` 列「`POST /scan?stream=true` 與 `POST /kb/build`」，但 `agent-sse-wiring` archive（2026-04-24）已加 `POST /explore` 且 `task_id format` Requirement 已擴 `^(scan|kb|explore)_[0-9a-f]{8}$`。error containment 沒同步擴。本 change 補一行涵蓋 `/explore` + Scenario。

**E. M1 `web/dist` `@trace` boilerplate cleanup（apply 階段擴大範圍）**

Review 原本只 flag 了 `llm-provider` / `qdrant-client` 兩 spec 共 4 處（agent spot-check 漏掃）。Apply 階段 grep 全部 `openspec/specs/` 發現 **17 處** `code:\n  - web/dist\n` 同模式 M1 boilerplate，分布在 8 個 spec：`llm-provider`(2) / `qdrant-client`(2) / `app-packaging`(3) / `repo-layout`(3) / `tauri-shell`(3) / `tool-sandbox`(2) / `usage-tracking`(2) / `sidecar-runtime`(5)。本 change **同模式同源同 commit 一次清光**，避免後續 review 又抓到。Cleanup 後對應 `@trace` block 只留 `source` / `updated` 兩欄（保留 audit trail），不留空 `code:` 欄。

**F. Production code refactor — 移除 `explorer.py:601-603` dead write**

`run_explorer` 在 coverage round Step 顯式塞 `explorer_prompt_version=EXPLORER_PROMPT_VERSION` / `judge_prompt_version=JUDGE_PROMPT_VERSION`，但 `ReasoningLogger.write` 立刻用 `model_copy` 覆寫成相同值（`reasoning_logger.py:46-50`）— 純 dead write。本 change 移除這 2 行（連同 import 若不再被用），讓 logger 是唯一 stamp 點。Behavior 等價，但消除 future drift 風險。

## Non-Goals

- **不重新 architect** ReasoningLogger / Explorer / SSE — 純文字 + dead code 修正。
- **不改 LLM 行為** — production code 改動只有 `explorer.py` 移 2 行 dead write，不影響任何 chat / embed / 工具呼叫 path。
- **不改既有測試的 baseline** — `tests/golden/demo-synthetic/expected.json` 與 `tests/golden/timeline-storage-adapter-synthetic/ideal-route.json` 不動。
- **不動 Cat 2.5（authorization_audit + audit 路徑統一）** — 那兩條已在 commit `06744bb` 處理（Cat 2.5-A 落地 + Cat 2.5-B 決策記錄）。
- **不動 Cat 3 latent risk** — `rules_version` 三處 hard-code、FolderTools SSE emitter、Folder-mode prompt 詞彙、chat `cost_usd=0`、Generator output Sanitize 留 backlog，等對應 Module 動工時處理。
- **不引入新 capability** — 4 個 spec 都是 MODIFIED，0 個 ADDED。
- **不動 archive folder 任何檔** — archive 是歷史，永遠 frozen（CLAUDE.md 不變式精神）。

**拒絕的設計**

- **「拆 6 個 small change」**：每條 review issue 一個 change。棄用 — 過度 ceremony 燒時間，6 條都是 review-driven cleanup 同源，bundle 一個 change 是 atomic「review 完一輪修一輪」，archive 也只 1 個目錄。
- **「先 spec、後 code」分兩 change**：F 條 production refactor 與 C-1 spec 改是同一件事的兩面，分開 land 必有一邊沒對齊。
- **「直接編輯 main spec 不走 Spectra」**：違反 codebus convention（CLAUDE.md「改 archive 過的 capability spec 必須走 propose」）。即使是純文字微調也走 ceremony，留 audit trail。

## Capabilities

### New Capabilities

（無 — 全是 MODIFIED）

### Modified Capabilities

- `usage-tracking`：MODIFIED Requirement `LLMCallLogger writes llm_calls.jsonl`（`Sanitizer-ready field reserved` Scenario 移除 M1 wording）
- `explorer-sse`：MODIFIED Requirement `Explorer loop emits agent_thought / agent_action_result / judge_verdict events`（合法化 `tokens_used=0` P0 placeholder + 新 Scenario）
- `agent-core`：MODIFIED 兩 Requirement
  - `ReAct loop executes think-act-observe-judge-log-update each iteration`（新 Scenario：P0 follow_imports placeholder 行為）
  - `ReasoningLogger appends one JSONL line per Step to workspace path`（新 Scenario：prompt version stamp 是 logger 唯一寫入路徑）
- `sidecar-runtime`：MODIFIED Requirement `Background task error containment`（涵蓋 `/explore`）

## Impact

**受影響 spec**：

- `openspec/specs/usage-tracking/spec.md`
- `openspec/specs/explorer-sse/spec.md`
- `openspec/specs/agent-core/spec.md`
- `openspec/specs/sidecar-runtime/spec.md`
- `openspec/specs/llm-provider/spec.md`（純 `@trace` cleanup，不動 Requirement）
- `openspec/specs/qdrant-client/spec.md`（純 `@trace` cleanup，不動 Requirement）
- `openspec/specs/app-packaging/spec.md`（`@trace` cleanup，3 處）
- `openspec/specs/repo-layout/spec.md`（`@trace` cleanup，3 處）
- `openspec/specs/tauri-shell/spec.md`（`@trace` cleanup，3 處）
- `openspec/specs/tool-sandbox/spec.md`（`@trace` cleanup，2 處）
- 注意：`usage-tracking` / `sidecar-runtime` 同時涵蓋 Requirement MODIFIED + `@trace` cleanup，已列在前面

**受影響 code**：

- `sidecar/src/codebus_agent/agent/explorer.py`：移 L601-603 兩個 dead write 欄位（連同 imports `EXPLORER_PROMPT_VERSION` / `JUDGE_PROMPT_VERSION` 若不再被用）

**受影響 docs**：

- `docs/reviews/2026-04-25-stage-4.md`：Cat 2 子項全部勾掉、進度狀態總表更新

**受影響測試**：

- 既有 28 個 golden 測 + 698 個 sidecar 測都應該全綠。`explorer.py` 修改是 dead write 移除，behavior 等價；既有 test 對 reasoning_log 的 prompt version assertion 改吃 logger 的 stamp 路徑（已是同樣值，本來就吃這條）。

**無新依賴**。**無 breaking change**（純 spec 文字修正 + dead code 移除）。
