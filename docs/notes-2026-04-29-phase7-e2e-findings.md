# 2026-04-29 Phase 7 e2e 跑一輪 findings

> 第一次起 sidecar + 真 OpenAI key + Qdrant + Tauri 跑全鏈的觀察紀錄。對齊 `notes-2026-04-29-ai-architecture.md` 的 A1 action item。
>
> **這份是 fill-in-as-you-go 模板** — 你跑到哪寫到哪。跑完後決定哪些觀察值得開 follow-up change。

---

## 一、Pre-flight checklist

- [x] OpenAI key 已設（`CODEBUS_OPENAI_API_KEY` in `.env`）
- [x] Qdrant binary at `~/.codebus/bin/qdrant.exe`
- [x] sidecar venv（`sidecar/.venv`）
- [x] web node_modules
- [x] Rust toolchain (cargo 1.95)
- [x] Qdrant 起來（`bash sidecar/scripts/start-qdrant.sh`）
- [x] cargo tauri dev 起來
- [x] standalone sidecar 起來（為了看 stderr，A9）

---

## 一.5、跑 e2e 過程中發現的真實 bug / finding（**首要產出**）

### A8 — Duplicated TypeScript import "ActionEntry" — ✅ 已修（2026-04-30，`fix-action-entry-import-collision`）

**修法**：兩 type 結構同（`{ tool, observation, tokens_used, isError }`，grep 確認），原 notes 寫「兩 type 的欄位不同」是觀察期誤判。real fix 是 DRY 化抽 `web/app/types/agent-action.ts` 為單一 source，`useExplorerStream.ts` / `useQaSession.ts` 兩支移除自身 `export interface ActionEntry` 後 `import type { ActionEntry } from '~/types/agent-action'`。`openspec/specs/qa-overlay/spec.md` §55 cross-reference 同步改寫成「imported from canonical type module」並加上 scenario「ActionEntry is imported from canonical type module — no duplicate export warning」。`web/tests/types/agent-action.spec.ts` 兩 RED→GREEN 測（source-grep single-source + shape-invariant via `expectTypeOf`）守鎖死。

**驗證**（2026-04-30）：`npm run dev` 啟動 stdout 不再印 `Duplicated imports "ActionEntry"`；`npm run test` 全套 27 files / 139 tests 全綠；root + tutorial route HTTP 200。

**症狀**（修前）：`cargo tauri dev` 啟動時 Nuxt 印 warn：

```
WARN  Duplicated imports "ActionEntry", the one from
"web/app/composables/useExplorerStream.ts" has been ignored and
"web/app/composables/useQaSession.ts" is used
```

**原因**：qa-overlay-p0 在 `useQaSession.ts` 加了 `ActionEntry` type，但 `useExplorerStream.ts`（agent-console-p0 archive）已經先 export 過同名 type。Nuxt auto-import 撞名，後者壓掉前者。儘管兩 type 結構同 → 今天無 runtime bug，但若任一支獨立演進 schema 會 silent collapse 成 production bug。

---

### A9 — Tauri-spawned sidecar 吞掉 stderr，無法診斷

**症狀**：Tauri 啟動的 sidecar 任何 Python traceback / log 都不出現在 `cargo tauri dev` 終端機。kb_build INTERNAL_ERROR 完全沒線索可看（A10 那 bug 一開始抓不到原因就是因為這個）。

**原因**：Tauri sidecar plugin 的 `Command::spawn` 把 sidecar 的 stdout/stderr capture 起來只解析 first-line handshake，後續 stream 不 relay 到父終端。

**workaround**：開獨立 PowerShell `uv run python -m codebus_agent.api.main`，stderr 才看得到。

**fix 方向**：
- 短期：sidecar 加 file logging（寫 `~/.codebus/logs/sidecar-<date>.log`），就算 Tauri 吞 stderr 還是有檔案可看
- 中期：Tauri Rust side 把 sidecar stdout 1+ 行（除第一行 handshake JSON）relay 到 stderr
- 長期：D-033 Change B Onboarding 內含 「Diagnostics」按鈕 → 開 log file location

**規模**：短期 fix（file logging）~半天 + propose；中期 Rust side 修 ~半天

---

### A10 — ReAct message ordering bug（**Production blocker**）— ✅ 已修（2026-04-30，`react-message-ordering-fix`）

**修法**：spec / code / docs 已同步落地。Wire format 鎖死 `[system, *normalized_history, user]`；orphan `role="tool"` 訊息（current state.messages 從不含 `assistant tool_calls`，所以每筆 tool 都是 orphan）由 `_normalize_orphan_tools(windowed)` 改寫成 `role="user"` 的觀察 note，保留 LLM 跨 iteration 觀察視野同時讓 OpenAI Chat Completions 不再 400。`agent.qa._qa_think` 直接 import 重用同 helper。

**E2E rerun 驗證**（2026-04-30）：對 `tests/golden/timeline-storage-adapter-synthetic/workspace/` 跑 scan → kb_build → explore（task: trace storage adapter implementation, budget_steps=6, budget_tokens=50_000），SSE 流跑滿 6 輪 + 1 round coverage gap 後 `terminal type=done`，無任何 400 / `BadRequestError`。多輪 `llm_call` / `agent_thought` / `agent_action_result` / `judge_verdict` 真 OpenAI roundtrip 全綠。

**症狀**（修前）：Explorer + Q&A 第一輪呼 OpenAI 就會在第二步以後拋：

```
openai.BadRequestError: Error code: 400
Invalid parameter: messages with role 'tool' must be a response to a
preceding message with 'tool_calls'.
```

**Root cause**：
- `sidecar/src/codebus_agent/agent/explorer.py:166-171`
- `sidecar/src/codebus_agent/agent/qa.py:188-192`

兩處同 pattern bug：

```python
windowed = state.messages[-_MESSAGE_ROLLING_WINDOW:]
messages = _to_provider_messages(windowed) + [
    ProviderMessage(role="system", content=...),
    ProviderMessage(role="user", content=user_prompt),
]
```

兩問題：
1. **System message 跑到尾巴**（OpenAI 慣例 system 在最前）
2. **滑動窗口可能切掉 `assistant tool_calls`**，留下 orphan `role: tool` 在 windowed 第一位 → OpenAI 拒絕

**為何單元測沒抓到**：MockProvider 不驗 OpenAI message ordering 規則，scripted replay 不 round-trip 到真 API。

**規模 & 決策**：見下面「結論」段。

---

### A11（小）— `tests/golden/demo-synthetic/workspace/` fixture 不完整

該目錄 README 描述要有 TS / Python / YAML / Markdown / .env 多語言植入 PII 的豐富 fixture，**實際只有 3 個 docstring-only Python stub 檔**。Demo / 錄影前要補完。

**規模**：純內容工，1-2h 寫 fixture 檔案 + 規則命中校準。

---

### A12（小）— `npm run typecheck` 3 個 pre-existing error（非 A8 範圍）

跑 `fix-action-entry-import-collision` task 5.1 時順手發現 baseline 已存在的 type error（與 ActionEntry 無關，本 change 沒引入新 error，視為 baseline-neutral 通過）：

1. `web/app/components/qa/QAOverlay.vue:29`（× 2）— `lastTurn.value` possibly `undefined`（TS18048）。
   - 原因：`lastTurn` computed 回 `list[list.length - 1] ?? null`，但 TS `noUncheckedIndexedAccess` 把 `list[i]` 視作 `T | undefined`，整體成 `T | null | undefined`；`sendDisabled` 內 `=== null` 沒收掉 `undefined`。
   - 來源 archive：`qa-overlay-p0`（commit 0cbacac）。
2. `web/app/pages/audit/sanitizer.vue:113` — `v-else-if="showError"` 觸發 TS2774「condition will always return true since this function is always defined」。
   - 原因：template 對 `ComputedRef` 的 auto-unwrap 在 vue-tsc 視角沒展開，把 `showError` 當 function。
   - 來源 archive：`sanitizer-audit-inspector-p0`（commit a26024c）。

**規模**：兩支各加 null-guard / 改寫 template 條件即可，~30min；應另開 change（如 `fix-phase7-typecheck-baseline`）以保持 fix-action-entry-import-collision scoped。

---

---

## 二、Test workspace

選 `tests/golden/demo-synthetic/workspace/`（9 file、含 sanitizer fixture）。

絕對路徑（grant 時填）：`D:/side_project/codebus/tests/golden/demo-synthetic/workspace`

---

## 三、Stage-by-stage 觀察

### 3.1 Grant flow（O-01）

- [ ] grant modal 跳出
- [ ] 顯示 outbound endpoint = `api.openai.com`（單一 provider 現況）
- [ ] sanitizer rules version 顯示
- [ ] `~/.codebus/authorization_audit.jsonl` 多一行 `grant_issued`

**觀察**：

```
（你跑完寫）
```

---

### 3.2 Scan + KB build（Module 1 + 2）

- [ ] scan 進度條跑
- [ ] sanitizer Pass 1 觸發（看 `<ws>/.codebus/sanitize_audit.jsonl` 多行 `pass: 1`）
- [ ] KB build 完成、Qdrant collection 有 chunks

**Token 用量**（看 `<ws>/.codebus/token_usage.jsonl` filter `module: "kb_build"`）：

```
prompt tokens:
completion tokens (n/a for embed):
estimated cost:
```

**觀察**（KB chunk 切得合理嗎？）：

```
（你跑完寫）
```

---

### 3.3 Explorer（Module 4）

- [ ] page `/explorer/<task_id>` 開
- [ ] timeline 即時更新（agent_thought / agent_action_result）
- [ ] CoverageBanner 顯示
- [ ] AuditPanel 切 reasoning tab 看到 step 累積
- [ ] AuditPanel 切 llm tab 看 LlmCallInspector
- [ ] AuditPanel 切 sanitize tab 看 SanitizerAuditInspector

**Step 數 + 終止條件**：

```
總步數:
終止由誰觸發: tool_calls=[] / budget cap / Judge / Coverage
Judge 評分:
Coverage 達標:
```

**Token 用量**（filter `module IN ("reasoning", "judge", "coverage")`）：

```
reasoning total: $
judge total: $
coverage total: $
```

**Reasoning quality 主觀評分**：

| 步 | tool | observation 摘要 | 是否合理 |
|---|---|---|---|
| 1 | | | |
| 2 | | | |
| ... | | | |

**踩到的雷**：

```
（你跑完寫，例如：Judge 給爛答案 9/10 / Explorer 重複 list_dir 同目錄 5 次 / 漏看明顯重要檔）
```

---

### 3.4 Generator（Module 5）

- [ ] 5 站 markdown 產出 at `<ws>/codebus-tutorials/<task_id>/`
- [ ] `tutorial.md` MOC index 完整
- [ ] `route.json` 5 station 結構正確
- [ ] 每站 frontmatter 6 欄齊（station_id / station_index / title / duration_minutes / related_stations / required_checks）
- [ ] `<Checkpoint>` / `<Quiz>` / `<QAEntry>` 三 mdc 元件用法正確（不破 R-01 互動契約）
- [ ] `### Quiz` heading 階層對（不是 `## Quiz`）

**格式錯誤統計**（你的 verifier 提案的 motivation 數字）：

| 規格 | 5 站中違反幾站 |
|---|---|
| frontmatter 漏欄 | / 5 |
| Checkpoint 元件錯 | / 5 |
| Quiz 元件錯 | / 5 |
| QAEntry prompt 為空 | / 5 |
| `### Quiz` 階層錯 | / 5 |
| related_stations 引用不存在站 | / 5 |

**站文品質主觀評分**：

| 站 | 內容深度（1-5）| 流暢度（1-5）| Checkpoint 設問質量（1-5）|
|---|---|---|---|
| s01-* | | | |
| s02-* | | | |
| ... | | | |

**Token 用量**（filter `module: "generate"`）：

```
total: $
average per station: $
```

**踩到的雷**：

```
（你跑完寫，例如：站 3 寫成 README 風格不是 walkthrough / Checkpoint 全是「請理解 X」這種無設問 / 跨站引用斷掉）
```

---

### 3.5 Walk station（R-01 + interactive-tutorial）

- [ ] MOC 首頁顯示 5 站
- [ ] 點 s01 → 站頁渲染 markdown
- [ ] Checkpoint 點打勾 → progress.json 更新 → 解鎖 s02
- [ ] Quiz 答對 → 解鎖
- [ ] QAEntry 點按鈕 → drawer 滑出（qa-overlay-p0）
- [ ] AuditPanel sanitize tab 點 row → SanitizerAuditInspector 開（sanitizer-audit-inspector-p0）
- [ ] AuditPanel llm tab 點 row → LlmCallInspector 開（llm-call-inspector-p0）

**互動踩雷**：

```
（你跑完寫）
```

---

### 3.6 Q&A drawer（Module 8 + qa-overlay-p0）

- [ ] Cmd+K 召喚 drawer
- [ ] 問 3 題（建議：「為什麼有 storage adapter？」「怎麼加新 backend？」「為什麼 atomic write？」）
- [ ] 每題看 4 phase 全到位（user / RAG hits / ReAct steps / answer with citations）
- [ ] kb_growth event 出現 → AuditPanel kb_growth tab live-tail
- [ ] station chip 點擊 → emit navigate-to-station
- [ ] ESC 關 drawer

**add_to_kb 判準**：

| 問題 | LLM 是否 add_to_kb | 你覺得該不該 add | 評語 |
|---|---|---|---|
| Q1 | yes/no | yes/no | |
| Q2 | yes/no | yes/no | |
| Q3 | yes/no | yes/no | |

**Token 用量**（filter `module: "qa_agent"`）：

```
total: $
average per question: $
```

**踩到的雷**：

```
（你跑完寫）
```

---

## 四、整體成本

| 階段 | Tokens (in/out) | Cost |
|---|---|---|
| KB build | / | $ |
| Explorer | / | $ |
| Generator | / | $ |
| Q&A | / | $ |
| **Total** | / | **$** |

**OpenAI dashboard 對得起來嗎？**（CodeBus 算的 cost vs OpenAI 後台）：

```
CodeBus 算: $
OpenAI 後台: $
差距:
```

如果差距 > 20%，可能是 token_usage.jsonl 沒記 cache 折扣（A3 action item 的 motivation）。

---

## 五、整體結論（跑完才寫）

### 5.1 mini 撐得住嗎

```
撐得住 / 撐不住，原因：
```

### 5.2 哪些護欄發揮作用

```
Instructor / Pydantic 鎖 schema：
Budget cap：
Judge：
Coverage：
Sanitizer：
Sandbox：
```

### 5.3 哪些 action items 因此升級 / 降級

| Action | 原規模 | e2e 後 |
|---|---|---|
| A2 Generator verifier | 1d propose + 1d apply | |
| A3 token_usage cache 欄 | 0.5d | |
| A4 D-033 Change B per-role | 1d propose | |
| A5 Judge 升 gpt-4o | 1 行 | |
| A6 AnthropicChatProvider | 200 LoC + qual | |
| A7 step 29 三介入點 | 0.5d propose | |

### 5.4 e2e 自身發現的新 issue（沒在 7 個 action items 裡的）

```
1.
2.
3.
```

### 5.5 下一步建議

```
（基於上面的觀察，下一個 propose 該開哪個 / 哪個先擱置）
```
