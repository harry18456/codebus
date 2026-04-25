## 1. 前置驗證

- [x] 1.1 確認 baseline 測試全綠（`uv run pytest sidecar/tests/` 698 passed / 19 skipped），記錄成 baseline number — 2026-04-25 confirmed 698 passed / 19 skipped / 79 warnings / 84.97s
- [x] 1.2 確認既有 `tests/golden/` 28 測全綠 — 已涵蓋在 1.1 全測中

## 2. Spec MODIFIED — A 條（usage-tracking M1 wording 移除）

對應 ADDED-by-MODIFY spec delta `openspec/changes/spec-cleanup-stage-4-batch-1/specs/usage-tracking/spec.md`，覆寫 main spec 的 `LLMCallLogger writes llm_calls.jsonl` Requirement，`Sanitizer-ready field reserved` Scenario 移除「During M1 ... false」一句。

- [x] 2.1 寫 `specs/usage-tracking/spec.md` delta，`## MODIFIED Requirements` 包整條 Requirement 全文（含三 Scenario），其中 `Sanitizer-ready field reserved` Scenario 改為「MUST contain a sanitizer_pass2_applied boolean field whose value reflects whether Sanitizer Pass 2 was applied to the request before dispatch」（無 M1 conditional wording）
- [x] 2.2 `spectra validate --strict` 確認 delta 合法（已驗證一次）

## 3. Spec MODIFIED — B 條（explorer-sse `tokens_used` placeholder 合法化）

對應 spec delta `specs/explorer-sse/spec.md`，`Explorer loop emits agent_thought / agent_action_result / judge_verdict events` Requirement 主文加一句 P0 placeholder 句、加 Scenario `tokens_used MAY be 0 in P0 implementation`。

- [x] 3.1 寫 `specs/explorer-sse/spec.md` delta，`## MODIFIED Requirements` 包整條 Requirement 全文，主文補 P0 placeholder 句
- [x] 3.2 加 Scenario `tokens_used field accepts P0 placeholder zero`
- [x] 3.3 `spectra validate --strict` 確認 delta 合法（已驗證一次）

## 4. Spec MODIFIED — C-1 條（agent-core ReasoningLogger 唯一 stamp）

對應 spec delta `specs/agent-core/spec.md`，`ReasoningLogger appends one JSONL line per Step to workspace path` Requirement 加 Scenario 規定 logger 是 prompt version 唯一寫入路徑（rationale: Decision 1 of design.md）。

- [x] 4.1 寫 `specs/agent-core/spec.md` delta C-1 條 MODIFIED Requirement
- [x] 4.2 新 Scenario `Logger is the single source of truth for prompt version stamping` 寫入 delta

## 5. Spec MODIFIED — C-2 條（agent-core follow_imports placeholder）

對應同 `specs/agent-core/spec.md` delta，第二條 MODIFIED Requirement 是 `ReAct loop executes think-act-observe-judge-log-update each iteration`，加 Scenario 合法化 P0 push tool_name 到 pending_queue 的 placeholder 行為。

- [x] 5.1 在 `specs/agent-core/spec.md` delta 加第二條 `## MODIFIED Requirements`（同檔，`---` 分隔）
- [x] 5.2 新 Scenario `Update step uses tool_name as P0 pending_queue placeholder` 寫入 delta
- [x] 5.3 `spectra validate --strict` 確認 agent-core delta（含 C-1 + C-2 兩條）合法（已驗證一次）

## 6. Spec MODIFIED — D 條（sidecar-runtime error containment 涵蓋 /explore）

對應 spec delta `specs/sidecar-runtime/spec.md`，`Background task error containment` Requirement 主文加 `/explore` 列舉 + 加 Scenario for explore failure。

- [x] 6.1 寫 `specs/sidecar-runtime/spec.md` delta，主文加 `/explore` + 顯式錯誤碼表（`SCAN_FAILED` / `KB_BUILD_FAILED` / `EXPLORE_FAILED`）
- [x] 6.2 加 Scenario `Explore task exception surfaces as safe error event`
- [x] 6.3 `spectra validate --strict` 確認 delta 合法（已驗證一次）

## 7. 直接編輯 main spec — E 條（@trace web/dist cleanup）

不走 delta 路徑（rationale: Decision 3 of design.md），直接動 main spec @trace 區塊。**Apply 階段範圍從 2 spec 擴大到 8 spec**（agent spot-check 漏掃，實際 grep 全 spec 發現 17 處同模式 M1 boilerplate）。

- [x] 7.1 編輯 `openspec/specs/llm-provider/spec.md` 2 處：刪 `code:\n  - web/dist\n` 三行（`@trace` block 內）
- [x] 7.2 編輯 `openspec/specs/qdrant-client/spec.md` 2 處：同模式
- [x] 7.3 編輯 `openspec/specs/app-packaging/spec.md` 3 處：同模式（apply 階段擴大範圍發現）
- [x] 7.4 編輯 `openspec/specs/repo-layout/spec.md` 3 處：同模式
- [x] 7.5 編輯 `openspec/specs/tauri-shell/spec.md` 3 處：同模式
- [x] 7.6 編輯 `openspec/specs/tool-sandbox/spec.md` 2 處：同模式
- [x] 7.7 編輯 `openspec/specs/usage-tracking/spec.md` 2 處：同模式
- [x] 7.8 編輯 `openspec/specs/sidecar-runtime/spec.md` 5 處：同模式
- [x] 7.9 grep 確認 `openspec/specs/` 不再有 `web/dist`（`Grep web/dist`，0 hits 確認）

## 8. Production code refactor — F 條（explorer.py dead write 移除）

對應 design.md Decision 2（spec 與 code 同 change）。

- [x] 8.1 編輯 `sidecar/src/codebus_agent/agent/explorer.py` 移除 dead write 兩行
- [x] 8.2 移除 `EXPLORER_PROMPT_VERSION` / `JUDGE_PROMPT_VERSION` 兩個孤兒 import（`from .prompts.explorer import (...)` 與 `from .prompts.judge import JUDGE_PROMPT_VERSION` 對應行）
- [x] 8.3 跑 `uv run pytest sidecar/tests/agent/` + `tests/golden/` — 151 passed / 2 skipped（Windows symlink）
- [x] 8.4 涵蓋於 8.3

## 9. 文件 / metadata 更新

- [x] 9.1 更新 `docs/reviews/2026-04-25-stage-4.md`：Cat 2 區段全部子項勾 `[x]`、進度狀態總表 Cat 2 改 ✅ 完成
- [x] 9.2 更新 `CLAUDE.md` archive 表加入本 change（待 archive 後 work commit 一起做）

## 10. 完整驗證 + commit gate

- [x] 10.1 `uv run pytest sidecar/tests/` 完整 suite 全綠 — 698 passed / 19 skipped / 79 warnings / 91.37s（與 baseline 數字一致，無 regression）
- [x] 10.2 `pre-commit run --all-files` 全綠 — stage-0 hook 6 條全 Passed
- [x] 10.3 `spectra validate --strict` 整個 change 合法 — `✓ spec-cleanup-stage-4-batch-1 — valid`
