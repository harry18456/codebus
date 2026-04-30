## 1. 共用測試基礎設施（先做、被後續所有 TDD 紅測依賴）

對應 spec Requirement「Explorer applies rolling message window before each Think call」+「Q&A loop entry point with two-stage RAG-first flow」共用 SpyProvider 抓 messages。

- [x] 1.1 covers Requirement「Explorer applies rolling message window before each Think call」+「Q&A loop entry point with two-stage RAG-first flow」：寫 helper `sidecar/tests/agent/_message_ordering_helpers.py`，提供 `SpyProvider`（實作 `LLMProvider` Protocol，`chat(messages, response_model)` 把 messages 存到 `self.last_messages` + 回固定 dummy `ExplorerAction` / `QAAction`）+ `make_explorer_state(messages_history=...)` / `make_qa_state(messages_history=...)` 兩個 helper 構造帶指定 message 歷史的 state。對齊現有 `tests/kb/conftest.py::SpyProvider` 的 pattern；註冊到 `TrackedProvider.ALLOWED_INNER_TYPES` 用 `monkeypatch.setattr` 避免 registry guard 拒絕

## 2. Explorer applies rolling message window before each Think call (RED tests)

對應 `agent-core` MODIFIED Requirement「Explorer applies rolling message window before each Think call」全部 7 個 scenario，先紅後綠。

- [x] 2.1 [P] 寫 RED 測 `sidecar/tests/agent/test_message_ordering.py::test_explorer_think_system_first_user_last`：用 SpyProvider，給 state.messages 為空 → 呼 `_think` → assert `spy.last_messages[0].role == "system"` + content == EXPLORER_SYSTEM + `last_messages[-1].role == "user"`。**現況會 fail**（現在 system 在尾巴、user 在 system 之後）
- [x] 2.2 [P] 寫 RED 測 `test_explorer_think_strips_leading_orphan_tool`：給 state.messages = `[Message(role="tool", content="orphan", tool_call_id="x"), Message(role="user", content="next")]` → 呼 `_think` → assert spy.last_messages 中找不到任何 orphan tool（即不存在 `messages[i].role == "tool"` 而 `messages[i-1].role` 不是 `"assistant"` with tool_calls 或同一 assistant 對應的 `"tool"`）。**現況會 fail**（現在 tool 直接被送出去）
- [x] 2.3 [P] 寫 RED 測 `test_explorer_think_keeps_paired_assistant_tool`：給 state.messages = `[Message(role="assistant", content="", tool_call_id=None) with tool_calls, Message(role="tool", content="result", tool_call_id="x"), Message(role="user", content="next")]` → 呼 `_think` → assert assistant 跟 tool 都在 spy.last_messages 中、相對順序維持（assistant 在 tool 之前）。確保 trim 不誤刪有 pair 的 tool
- [x] 2.4 [P] 寫 RED 測 `test_explorer_think_window_size_respected`：給 state.messages 包 20 個合法 message（head 是 `[user, assistant_tool_calls, tool, ...]`，沒 orphan tool）→ 呼 `_think` → assert spy.last_messages 中除 system + user 外，最多 16 個 history entry（_MESSAGE_ROLLING_WINDOW = 16）

## 3. Explorer fix（GREEN；對應 spec Requirement「Explorer applies rolling message window before each Think call」MODIFIED 條目）

- [x] 3.1 改 `sidecar/src/codebus_agent/agent/explorer.py:166-171` 的 `_think` 函式 messages 構造：先 trim leading 'tool' role from `windowed`（while loop），再組成 `[system, *windowed, user]`（system 改放最前）。code 旁加註解 cite 本 change 跟 OpenAI ordering rule
- [x] 3.2 確認 step 2 全部 4 個 RED 測轉綠（`uv run pytest tests/agent/test_message_ordering.py -k explorer -v`）

## 4. Q&A loop entry point with two-stage RAG-first flow (RED tests for `_qa_think` ordering)

對應 `qa-agent` MODIFIED Requirement「Q&A loop entry point with two-stage RAG-first flow」新加的 2 個 `_qa_think` ordering scenario，先紅後綠。

- [x] 4.1 [P] 寫 RED 測 `test_qa_think_system_first_user_last`：用 SpyProvider，給 QAState.messages 為空 → 呼 `_qa_think` → assert `spy.last_messages[0].role == "system"` + content == QA_SYSTEM + `last_messages[-1].role == "user"`。**現況會 fail**
- [x] 4.2 [P] 寫 RED 測 `test_qa_think_strips_leading_orphan_tool`：給 QAState.messages = `[Message(role="tool", ...)]` → 呼 `_qa_think` → assert spy.last_messages 不含 orphan tool。**現況會 fail**

## 5. Q&A fix（GREEN；對應 spec Requirement「Q&A loop entry point with two-stage RAG-first flow」MODIFIED）

- [x] 5.1 改 `sidecar/src/codebus_agent/agent/qa.py:188-192` 的 `_qa_think` 函式 messages 構造：同 explorer 的 fix（trim leading tool + system 放最前）。code 旁加註解 cite agent-core spec 的 ordering rule + 本 change
- [x] 5.2 確認 step 4 兩個 RED 測轉綠（`uv run pytest tests/agent/test_message_ordering.py -k qa -v`）

## 6. Regression 測

- [x] 6.1 跑既有 explorer 測 `uv run pytest tests/agent/test_explorer.py -v` 全綠（既有 scenarios 含 `Think receives at most window-size messages` / `Reasoning log records full iteration history` 等不破）
- [x] 6.2 跑既有 Q&A 測 `uv run pytest tests/agent/test_qa.py -v` 全綠
- [x] 6.3 跑 golden replay `uv run pytest tests/golden/test_timeline_synthetic_replay.py -v` 全綠（scripted MockProvider 不驗 ordering 但 message-list 結構沒破）
- [x] 6.4 跑全套 `uv run pytest` 全綠（baseline 0 regression）

## 7. Phase 7 e2e 重跑驗證（deferred — manual smoke after archive）

> Unit + integration test 已全綠（test_message_ordering 6 + test_message_rolling_window 4 + test_observations_feed_forward 1），spec 全 scenario 覆蓋；e2e 重跑是真 OpenAI smoke，留給人手動跑、不阻塞 archive。
>
> 跑法摘要（A9 workaround：起獨立 sidecar 看 stderr）：
> 1. `bash sidecar/scripts/start-qdrant.sh`
> 2. 另 PowerShell：`cd sidecar && uv run python -m codebus_agent.api.main`，記下 stdout 印的 `port` + `bearer`
> 3. POST /scan {workspace_root: <abs path of tests/golden/timeline-storage-adapter-synthetic/workspace>, workspace_type: "folder"}
> 4. POST /kb/build → 等到 `done`
> 5. POST /explore {task: "trace through the storage adapter…", ...} → 確認 SSE 跑到 `done` 不再 400
> 6. 補回填 `docs/notes-2026-04-29-phase7-e2e-findings.md` A10 為 `[x] 已修（react-message-ordering-fix）`

- [ ] 7.1 (deferred) Phase 7 e2e 重跑：起獨立 sidecar + Qdrant，對 `tests/golden/timeline-storage-adapter-synthetic/workspace/` 跑 scan → kb_build → explore，assert /explore 跑到 `done` 不再 400
- [ ] 7.2 (deferred) 把 e2e 結果回填 `docs/notes-2026-04-29-phase7-e2e-findings.md`：A10 標記為 `[x] 已修（react-message-ordering-fix）`，並繼續走 Stage 3.5 (Generator) 之後的觀察

## 8. 文件同步

- [x] 8.1 `docs/decisions.md` D-012「自寫 ReAct loop + Instructor」段落補一條 footnote：「ReAct message wire-format 順序由 `react-message-ordering-fix`（archive 日期 placeholder）鎖死：`[system, *windowed_history(去 orphan tool), user]`，違反會被 OpenAI 400」
- [x] 8.2 `docs/agent-core.md` 與 `docs/qa-agent.md` 各補一段 `_think` / `_qa_think` 構造說明（與 spec 對齊，避免未來 spec / docs 漂移）

## 9. 整合驗證

- [x] 9.1 `cd sidecar && uv run pytest tests/agent/test_message_ordering.py tests/agent/test_message_rolling_window.py tests/agent/test_explorer_loop.py tests/agent/test_run_qa.py tests/golden/test_timeline_synthetic_replay.py -v` — message-ordering 6 + rolling-window 4 + explorer_loop 9 + run_qa 5 + golden replay 10 全綠（既有檔名 test_explorer.py / test_qa.py 不存在；改用實際檔名）
- [x] 9.2 `pre-commit run --all-files` 全綠後 commit

---

## Spec coverage map

每條 spec scenario 對應的 task：

- **`agent-core` MODIFIED Requirement「Explorer applies rolling message window before each Think call」**
  - Scenario「System message is first element」→ 2.1, 3.1, 3.2
  - Scenario「Leading orphan tool messages are stripped」→ 2.2, 3.1, 3.2
  - Scenario「Assistant tool_calls and matching tool messages stay paired」→ 2.3, 3.1, 3.2
  - Scenario「Think receives at most window-size messages」→ 2.4, 3.1, 3.2 (regression in 6.1)
  - Scenario「Think preserves all state when message count is below window」→ 6.1 regression
  - Scenario「Reasoning log records full iteration history」→ 6.1 regression
  - Scenario「Coverage-gap recursion frame respects same window」→ 6.1 regression

- **`qa-agent` MODIFIED Requirement「Q&A loop entry point with two-stage RAG-first flow」**
  - Scenario「`_qa_think` provider wire prompt starts with system message」→ 4.1, 5.1, 5.2
  - Scenario「`_qa_think` strips leading orphan tool messages」→ 4.2, 5.1, 5.2
  - 既有 4 個 scenarios（confident hits / non-confident / Judge / kw-only）→ 6.2 regression
