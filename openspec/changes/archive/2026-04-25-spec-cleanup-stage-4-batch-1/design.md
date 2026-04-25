## Context

Cat 2 of `docs/reviews/2026-04-25-stage-4.md` 6 條 spec drift / placeholder / misalignment fix。Backend Stage 1-4 已 archive 完，下個動工目標是 Module 5 Generator P0（步驟 24）。在 Module 5 動工前清掉 review 抓到的 spec 真錯，避免錯誤 spec 滲入新 Module 設計。

本 change 是 ceremony cleanup change，不導入新 capability、不改 LLM 行為、不動 fixture baseline。範圍嚴格鎖在 review tracker Cat 2 列出的 6 條。

## Goals / Non-Goals

**Goals**

- 把 4 個 capability spec 的真錯改掉（usage-tracking / explorer-sse / agent-core / sidecar-runtime），讓 spec 對齊 production 真實行為
- 把 2 個 spec 的 4 條假 `@trace` 連結（`web/dist`）刪掉
- 把 `explorer.py:601-603` dead write 移除，鎖死「ReasoningLogger 是 prompt version stamp 唯一寫入路徑」
- 走完整 Spectra ceremony（`spectra new change` → `in-progress` → 修 + 測 → `validate` → `archive`）讓 audit trail 完整
- Review tracker Cat 2 區段標 ✅

**Non-Goals**（與 proposal Non-Goals 一致）

見 proposal Non-Goals 段：不動 Cat 2.5 / Cat 3、不重 architect、不改 LLM 行為、不動 fixture baseline、不引入新 capability、不動 archive folder、不直接編輯 main spec 不走 ceremony。

## Decisions

### Decision 1：Bundle 6 條 fix 進一個 change，不拆 6 個

選一個 change 包全部，不拆。

理由：
- **同源**：6 條都是同一輪 review 抓到，`docs/reviews/2026-04-25-stage-4.md` Cat 2 是 single source of truth。拆 6 個 archive 目錄會散
- **Atomic**：「review 完一輪修一輪」是有意義的 ceremony 單位 — 像 `usage-tracker-dedup` 那種「dedup 是一件事」的 atomic 改動
- **Ceremony 成本**：每個 Spectra change 要 propose + design + tasks + apply + archive = 約 30 min ceremony overhead。拆 6 個就是 3 hr 純 ceremony，毫無價值
- **Review trail**：archive 目錄裡 `proposal.md` 會明確列「來自哪輪 review、哪 6 條」，未來追溯一目瞭然

**替代方案**：拆「per-spec change」（usage-tracking-cleanup / explorer-sse-cleanup / ...）。棄用 — 過度切碎，archive 目錄爆增。

### Decision 2：F 條 production code refactor 與 C-1 spec 改放同 change

`explorer.py:601-603` dead write 移除（F 條）與 `agent-core` spec 加 「logger 唯一 stamp 路徑」Scenario（C-1 條）是同一件事的兩面：spec 鎖死約束、code 對齊約束。分兩 change 沒意義。

理由：
- **Codebus convention**：所有 archive 都是「spec 與 code 同 change」（見 `golden-sample-baseline` / `coverage-gap-recurse` 等）
- **One commit, one PR**：後人 review 時看 spec 與 code 並排，理解快
- **Test 同步驗證**：移 dead write 後既有 reasoning_log assertion 仍綠 = 證明 spec 規定的「logger 唯一 stamp」確實如此

**替代方案**：spec 先 land，code 後 land。棄用 — 違反 convention，且兩 change 之間 spec 與 code 短暫不一致。

### Decision 3：`@trace` cleanup（E 條）不寫成 delta spec

`@trace` 區塊是 HTML comment metadata，不是 Requirement。Spectra delta spec 機制（`## ADDED / MODIFIED / REMOVED Requirements`）不涵蓋 metadata 區塊。

選擇直接在 main spec 動手（task 階段）刪掉 `web/dist` 行，不走 delta-sync 路徑。

理由：
- **Metadata vs Requirement**：刪假 trace 不改任何 SHALL clause，不應走 Requirement-level ceremony
- **Spectra 工具支援**：`spectra archive` 會自動更新 `@trace` 的 `source` / `updated` 欄位，但不主動清理舊 trace 行 — 我們手動清是對的
- **既有先例**：archive folder 裡多個 spec 也有過時 `@trace` 行，沒有 ceremony 路徑能自動清；要清就直接動

**替代方案**：把 `@trace` 修正包進 `## MODIFIED Requirements`（複製整個 Requirement，連同末尾 trace block 一起改）。棄用 — 會把 trace block 與 Requirement 強綁，未來 trace 自動更新更難。

### Decision 4：A / B 條只加 / 改 Scenario，不動 Requirement 主文

Requirement 主文是 SHALL 合約，動主文要嚴格論證。A 條（usage-tracking）只刪 Scenario 一句 M1 wording、保留欄位約束；B 條（explorer-sse）加新 Scenario 合法化 placeholder，主文 `<int>` 仍是合約。

理由：
- **最小修改**：spec 修正不應變成 spec 大改
- **不動的好處**：既有測試對 SHALL 的 assertion 全綠不破

### Decision 5：移除 `EXPLORER_PROMPT_VERSION` / `JUDGE_PROMPT_VERSION` import 若不再被用

F 條移 dead write 後，`explorer.py` 的兩個 prompt version import 可能變孤兒。檢查後若確實沒其他用途就一併刪 — 保 import 的 dead code 也是 dead。

理由：
- **完整清理**：half-job 的 dead code 移除是技術債
- **Linter 友善**：未來 ruff `F401` 會抓到孤兒 import，現在順手清

## Risks / Trade-offs

- **[`explorer.py` 移 dead write 可能踩到隱藏依賴]** → 風險低；檢查過 `ReasoningLogger.write` 的 `model_copy` 路徑（`reasoning_logger.py:46-50`）會 unconditional 覆寫兩欄，所以 explorer 那兩行 100% 是 dead write。**Mitigation**：跑完整 sidecar test suite（698 passed）確認無 regression。
- **[`@trace` cleanup 可能誤刪有效 trace]** → 風險低；只刪 `web/dist`（M1 placeholder，目錄不存在），其他 trace path 保留。**Mitigation**：cleanup 後 grep 確認 trace 仍指向 sidecar/src/ 與 docs/ 真實路徑。
- **[Spec wording 改動觸發 prompt version drift guard]** → 不會；本 change 不動 `EXPLORER_PROMPT_VERSION` 與 `JUDGE_PROMPT_VERSION` 常數，只動 spec 文字。golden replay 仍綠。
- **[Module 5 / 8 設計被本 change 影響]** → 是的，但是好的影響；C-2 條合法化 follow_imports placeholder 後，Module 8 Q&A 接 ReAct core 時知道「pending_queue placeholder 是預期行為」，不會以為是 bug。

## Migration Plan

- 無 schema 破壞 — 4 個 spec 都是 wording / Scenario 微調
- 無 production behavior 改變 — `explorer.py` 移 dead write 後 reasoning_log 內容字面相同（logger 已經在 stamp）
- 無測試 baseline 變動 — `tests/golden/*/expected.json` / `ideal-route.json` 不動
- 無 HTTP API 改動 — `/explore` / `/scan` / `/kb/build` schema 不動
- 既有 698 passed sidecar 測 + 28 passed golden 測必須全綠（驗證 acceptance criteria）

## Open Questions

無。（review tracker Cat 2 已涵蓋全部範圍 + 修法明確、非設計取捨）
