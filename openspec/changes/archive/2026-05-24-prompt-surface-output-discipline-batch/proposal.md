## Summary

承接 Phase 4 batch sequence（chat-security-batch 之後），透過實機 LLM 驗證收斂 4 個 finding：1 個真實 bug（F93 quiz verify spawn unparseable）+ 3 個 SKILL 契約違反（F78/F38 Mode STOP 邊界、F70 chat no-match scope drift）。

## Motivation

2026-05-24 實機跑 `codebus quiz "JWT issuance and verification"`（啟用 `quiz.content_verify`）出現 `warning: quiz content-verify output unparseable (non-fatal; content_review: flagged)`，cap 跑完仍 `flagged []`（per `codebus-core/src/verb/content_verify.rs` parser 的 unparseable conservative flag）—— `content_review` 等於壞掉，user 永遠拿到 false-positive flag。

實機追因：
- **F93**（real bug）：`codebus-core/src/verb/quiz.rs` 的 verify spawn input 是 `topic={topic_arg}\n\nQUIZ:\n{body}`，沒帶 planned pages。Agent 只能從 quiz body 的 `[[slug]]` citations 反推該讀哪些 wiki page；spike 顯示 verify spawn 漏讀 2/3 planned page。Quiz spec 第「Stage 2 model-based content verification」要求說 verify spawn「reads the planned pages」，但 SKILL 與 verb 沒明示 page list 怎麼來。
- **F78**（contract violation）：quiz Mode C 收到 prompt 後 emit `**Q1 評估** / **Q2 評估** / **Q3 評估** / 驗證: xxx 第 N 行` 大段 prose 後才 `CONTENT_OK`，違反 `skill-bundles` spec Mode C「emit per-flagged-question line OR exactly CONTENT_OK」契約。Parser 目前 silently 略過非 `<id>|<defect-type>|<suggestion>` 格式行，但任何含 `|` 兩次的 prose 都可能被誤判為 defect line。
- **F38**（contract violation）：goal verify mode 同 pattern——agent emit `已完成所有變更頁面與 raw/code/src/db.py 原始碼的比對。` 後才 `CONTENT_OK`；目前 parser 接受但同樣脆弱。
- **F70**（scope drift）：實機問 chat「dark mode and theme switching」（vault 無 match），chat agent 明確說「codebase 沒這個」後接著 emit 5 點 unsolicited 一般 dark-mode 實作建議（frontend state、CSS variables、persistence、context provider、system preference detection）—— 對應 `chat-verb` spec「multi-turn read-only chat against this vault」hard scope 邊界被柔性違反。

砍 / defer 紀錄（避免歷史遺忘）：
- **F44**（query no-match）砍：實機驗 query agent baseline 對「dark mode」query 明確說「超出範圍」、沒 fabricate，spec 層不必補。
- **F34**（goal mode prefix collision）defer：claude backend `/codebus-goal "<text>"` quote 隔絕 user text；codex backend 「$codebus-goal <text>` 不 quote」是潛在 ambiguity，但 codex backend 還沒上線（per `agent-backend` codex-backend 未實作），現在補 SKILL 沒驗證對象，留待 codex backend change 一起處理。
- **F81**（quiz no-prefix fallback）defer：binary spawn 永遠注入 sub_mode prefix（per `codebus-core/src/agent/spawn_spec.rs` SpawnSpec assembly），user 無法觸發 no-prefix 路徑，只是 defense in depth，沒急。

## Proposed Solution

| Finding | 改動位置 | 修法 |
|---|---|---|
| F93 | `codebus-core/src/verb/quiz.rs`（verify_input format） | 把 `topic={topic_arg}\n\nQUIZ:\n{body}` 改成 `topic={topic_arg}\n\nPLANNED PAGES:\n<pages 列表>\n\nQUIZ:\n{body}`，pages 來自 `options.pages`（已在同檔 line 719 用來填 QuizReport.planned_pages） |
| F78 | `codebus-core/src/skill_bundle/mod.rs`（QUIZ_SKILL_CONTENT Mode C） | Mode C 末加「After the last `Q<n> \| <defect-type> \| <suggestion>` line (or `CONTENT_OK`), STOP. Do not emit any further prose, rationale, or summary.」 |
| F38 | `codebus-core/src/skill_bundle/mod.rs`（GOAL_WORKFLOW Verify mode） | Verify mode 末加「After the last `<path> \| <defect-type> \| <suggestion>` line (or `CONTENT_OK`), STOP. Do not emit any further prose, rationale, or summary.」 |
| F70 | `codebus-core/src/skill_bundle/mod.rs`（CHAT_SKILL_CONTENT Workflow 段） | Workflow 段加 no-match handling：「If `wiki/` and `raw/code/` retrieval returns nothing relevant to the user's question, say so explicitly (e.g., `this vault does not currently cover <topic>`); you MUST NOT emit hypothetical general-knowledge implementation suggestions for the missing topic.」 |

Spec 影響：
- `quiz`: Stage 2 content verification 要求補一句「verify spawn input SHALL include the planned page list」+ 新 Scenario「verify spawn receives planned page list」
- `skill-bundles`: Goal Verify mode + Quiz Mode C 兩段各加 STOP boundary 句 + 對應 Scenario
- `chat-verb`: Hard scope 或 Scope Guard 段補 no-match no-fabrication 句 + 對應 Scenario

實機驗證計畫：完工後重跑同樣 `codebus quiz "JWT issuance and verification"` 啟用 content_verify，預期不再出現 `output unparseable` warning、`content_review` 顯示 `ok` 或正常 `flagged [<question numbers>]`（含具體題號）。重跑 chat no-match 測試，預期 agent 不再 emit hypothetical implementation 5 點建議。

## Non-Goals

- 不處理 F44（query no-match）：實機 query agent 自己 handle 沒 fabricate，spec 不必補。
- 不處理 F34（goal mode prefix collision）：claude quote 隔絕、codex backend 未上線，留 codex backend change。
- 不處理 F81（quiz no-prefix fallback）：binary 永遠注入 sub_mode，純 defense in depth，留 polish batch。
- 不重構 verify spawn output format（不改 pipe-separated 為 JSON）—— F28 / F74 已撤回，本 batch 只補契約邊界、不動 wire format。
- 不改 content_verify cap = 3 的設定，cap 後 best-effort flag 機制不動。

## Alternatives Considered

**F93 — alt (b)：SKILL 明示「從 quiz body 的 `[[slug]]` citations 反推 planned pages」**。否決理由：實機證實 spike 反推**漏讀 2/3 planned page** —— `[[slug]]` 引用集中在 1 頁時，agent 不知道其他 planned pages 也在 scope。把 page list 直接放進 verify_input 才能保證完整 coverage，隱式契約轉顯式契約。

**F70 — alt：完全 refuse no-match 主題**（套 F63 scope-guard 等級的 `out of scope: my role` refusal）。否決理由：no-match 不是 off-topic（user 問的是 codebase，只是這個 vault 沒包含該主題），refuse 整輪太強硬。正解是「明示 no match + 禁止 fabricate hypothetical implementation」。

## Impact

- Affected specs: `quiz`、`skill-bundles`、`chat-verb`
- Affected code:
  - Modified: `codebus-core/src/verb/quiz.rs`、`codebus-core/src/skill_bundle/mod.rs`
  - New: (none)
  - Removed: (none)
- 實機驗證：跑 `codebus quiz` 啟用 content_verify 看 unparseable rate / `content_review` 結果；跑 `codebus chat` 餵 no-match 主題看 hypothetical 建議是否消失
- 後續：本 batch 完工後 update `docs/2026-05-23-prompt-surface-inventory.md` 把 F93/F78/F38/F70 標 FIXED + F44 標 INVALID（agent baseline 已 handle）+ F34/F81 標 DEFERRED
