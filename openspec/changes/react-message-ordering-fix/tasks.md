## 1. 共用測試基礎設施（先做、被後續所有 TDD 紅測依賴）

對應 spec Requirement「Explorer applies rolling message window before each Think call」+「Q&A loop entry point with two-stage RAG-first flow」共用 SpyProvider 抓 messages。

- [ ] 1.1 covers Requirement「Explorer applies rolling message window before each Think call」+「Q&A loop entry point with two-stage RAG-first flow」：寫 helper `sidecar/tests/agent/_message_ordering_helpers.py`，提供 `SpyProvider`（實作 `LLMProvider` Protocol，`chat(messages, response_model)` 把 messages 存到 `self.last_messages` + 回固定 dummy `ExplorerAction` / `QAAction`）+ `make_explorer_state(messages_history=...)` / `make_qa_state(messages_history=...)` 兩個 helper 構造帶指定 message 歷史的 state。對齊現有 `tests/kb/conftest.py::SpyProvider` 的 pattern；註冊到 `TrackedProvider.ALLOWED_INNER_TYPES` 用 `monkeypatch.setattr` 避免 registry guard 拒絕

## 2. Explorer applies rolling message window before each Think call (RED tests)

對應 `agent-core` MODIFIED Requirement「Explorer applies rolling message window before each Think call」全部 7 個 scenario，先紅後綠。

- [ ] 2.1 [P] 寫 RED 測 `sidecar/tests/agent/test_message_ordering.py::test_explorer_think_system_first_user_last`：用 SpyProvider，給 state.messages 為空 → 呼 `_think` → assert `spy.last_messages[0].role == "system"` + content == EXPLORER_SYSTEM + `last_messages[-1].role == "user"`。**現況會 fail**（現在 system 在尾巴、user 在 system 之後）
- [ ] 2.2 [P] 寫 RED 測 `test_explorer_think_strips_leading_orphan_tool`：給 state.messages = `[Message(role="tool", content="orphan", tool_call_id="x"), Message(role="user", content="next")]` → 呼 `_think` → assert spy.last_messages 中找不到任何 orphan tool（即不存在 `messages[i].role == "tool"` 而 `messages[i-1].role` 不是 `"assistant"` with tool_calls 或同一 assistant 對應的 `"tool"`）。**現況會 fail**（現在 tool 直接被送出去）
- [ ] 2.3 [P] 寫 RED 測 `test_explorer_think_keeps_paired_assistant_tool`：給 state.messages = `[Message(role="assistant", content="", tool_call_id=None) with tool_calls, Message(role="tool", content="result", tool_call_id="x"), Message(role="user", content="next")]` → 呼 `_think` → assert assistant 跟 tool 都在 spy.last_messages 中、相對順序維持（assistant 在 tool 之前）。確保 trim 不誤刪有 pair 的 tool
- [ ] 2.4 [P] 寫 RED 測 `test_explorer_think_window_size_respected`：給 state.messages 包 20 個合法 message（head 是 `[user, assistant_tool_calls, tool, ...]`，沒 orphan tool）→ 呼 `_think` → assert spy.last_messages 中除 system + user 外，最多 16 個 history entry（_MESSAGE_ROLLING_WINDOW = 16）

## 3. Explorer fix（GREEN；對應 spec Requirement「Explorer applies rolling message window before each Think call」MODIFIED 條目）

- [ ] 3.1 改 `sidecar/src/codebus_agent/agent/explorer.py:166-171` 的 `_think` 函式 messages 構造：先 trim leading 'tool' role from `windowed`（while loop），再組成 `[system, *windowed, user]`（system 改放最前）。code 旁加註解 cite 本 change 跟 OpenAI ordering rule
- [ ] 3.2 確認 step 2 全部 4 個 RED 測轉綠（`uv run pytest tests/agent/test_message_ordering.py -k explorer -v`）

## 4. Q&A loop entry point with two-stage RAG-first flow (RED tests for `_qa_think` ordering)

對應 `qa-agent` MODIFIED Requirement「Q&A loop entry point with two-stage RAG-first flow」新加的 2 個 `_qa_think` ordering scenario，先紅後綠。

- [ ] 4.1 [P] 寫 RED 測 `test_qa_think_system_first_user_last`：用 SpyProvider，給 QAState.messages 為空 → 呼 `_qa_think` → assert `spy.last_messages[0].role == "system"` + content == QA_SYSTEM + `last_messages[-1].role == "user"`。**現況會 fail**
- [ ] 4.2 [P] 寫 RED 測 `test_qa_think_strips_leading_orphan_tool`：給 QAState.messages = `[Message(role="tool", ...)]` → 呼 `_qa_think` → assert spy.last_messages 不含 orphan tool。**現況會 fail**

## 5. Q&A fix（GREEN；對應 spec Requirement「Q&A loop entry point with two-stage RAG-first flow」MODIFIED）

- [ ] 5.1 改 `sidecar/src/codebus_agent/agent/qa.py:188-192` 的 `_qa_think` 函式 messages 構造：同 explorer 的 fix（trim leading tool + system 放最前）。code 旁加註解 cite agent-core spec 的 ordering rule + 本 change
- [ ] 5.2 確認 step 4 兩個 RED 測轉綠（`uv run pytest tests/agent/test_message_ordering.py -k qa -v`）

## 6. Regression 測

- [ ] 6.1 跑既有 explorer 測 `uv run pytest tests/agent/test_explorer.py -v` 全綠（既有 scenarios 含 `Think receives at most window-size messages` / `Reasoning log records full iteration history` 等不破）
- [ ] 6.2 跑既有 Q&A 測 `uv run pytest tests/agent/test_qa.py -v` 全綠
- [ ] 6.3 跑 golden replay `uv run pytest tests/golden/test_timeline_synthetic_replay.py -v` 全綠（scripted MockProvider 不驗 ordering 但 message-list 結構沒破）
- [ ] 6.4 跑全套 `uv run pytest` 全綠（baseline 0 regression）

## 7. Phase 7 e2e 重跑驗證

- [ ] 7.1 起獨立 sidecar (`uv run python -m codebus_agent.api.main` 配 .env，A9 workaround) + Qdrant；對 `tests/golden/timeline-storage-adapter-synthetic/workspace/` 跑 scan → kb_build → explore（task 描述同 `notes-2026-04-29-phase7-e2e-findings.md` Stage 3.4 那個 storage adapter task）→ assert /explore 跑到 `done` 不再 400
- [ ] 7.2 把 e2e 結果回填 `docs/notes-2026-04-29-phase7-e2e-findings.md`：A10 標記為 `[x] 已修（react-message-ordering-fix）`，並繼續走 Stage 3.5 (Generator) 之後的觀察

## 8. 文件同步

- [ ] 8.1 `docs/decisions.md` D-012「自寫 ReAct loop + Instructor」段落補一條 footnote：「ReAct message wire-format 順序由 `react-message-ordering-fix`（archive 日期 placeholder）鎖死：`[system, *windowed_history(去 orphan tool), user]`，違反會被 OpenAI 400」
- [ ] 8.2 `docs/agent-core.md` 與 `docs/qa-agent.md` 各補一段 `_think` / `_qa_think` 構造說明（與 spec 對齊，避免未來 spec / docs 漂移）

## 9. 整合驗證

- [ ] 9.1 `cd sidecar && uv run pytest tests/agent/test_message_ordering.py tests/agent/test_explorer.py tests/agent/test_qa.py -v` 6+ test_message_ordering scenarios + 既有 explorer / qa 測全綠
- [ ] 9.2 `pre-commit run --all-files` 全綠後 commit

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
