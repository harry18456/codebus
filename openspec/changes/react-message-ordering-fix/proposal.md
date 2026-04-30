## Problem

Module 4 Explorer 與 Module 8 Q&A 的 ReAct loop 第一次跑真實 OpenAI traffic 就觸發 `400 Bad Request`：

```
Invalid parameter: messages with role 'tool' must be a response to a
preceding message with 'tool_calls'.
```

兩 module 的 `_think` / `_qa_think` 在迭代第二輪以後產生的 message array 結構違反 OpenAI Chat Completions 順序契約：(a) `system` role 訊息被放在 array 尾巴而不是首位；(b) `state.messages` 滑動窗口 (`_MESSAGE_ROLLING_WINDOW`) 切片時可能切掉一段 `assistant tool_calls` 但保留對應的 `tool` 訊息，造成 `tool` 訊息無 preceding `tool_calls` 而被 OpenAI 拒絕。

`agent-core` capability spec line 485 文字甚至直接寫死了 buggy 順序（"compose the provider wire prompt from windowed messages plus the EXPLORER_SYSTEM system message and the rendered user prompt"），導致實作照字面寫出 `[history, system, user]` 而非 `[system, history, user]`。`qa-agent` 透過 `_qa_think` clone（spec 寫「reuse explorer._think」但實作其實有自己的 `_qa_think`）複製了同款 bug。

`MockProvider` 單元測 + scripted replay 不驗 OpenAI message ordering 規則，所以 unit test / golden 測都沒抓到。Phase 7 第一次跑真實 LLM e2e 才暴露。

## Root Cause

`sidecar/src/codebus_agent/agent/explorer.py:166-171`：

```python
windowed = state.messages[-_MESSAGE_ROLLING_WINDOW:]
messages = _to_provider_messages(windowed) + [
    ProviderMessage(role="system", content=EXPLORER_SYSTEM),
    ProviderMessage(role="user", content=user_prompt),
]
```

兩個結構錯：

1. **System 順序錯**：OpenAI Chat Completions 文件慣例 system 訊息放 messages array 首位；放尾巴雖不直接破 OpenAI 規則但 spec 文字也跟著漂移。
2. **Orphan tool 漏 trim**：`state.messages[-N:]` slice 可能讓 windowed 第一筆訊息是 `role="tool"`，其對應 `assistant + tool_calls` 已被切掉。OpenAI 規定每個 `tool` 訊息必須緊跟在含 `tool_calls` 的 `assistant` 之後，否則 400。

`sidecar/src/codebus_agent/agent/qa.py:188-192` 是 explorer `_think` 複製貼上版本（function 名字 `_qa_think`），同 pattern 同 bug。

`agent-core/spec.md:485` Requirement「`_think` window-bounded prompt」直接把 buggy 順序寫進 spec，需 MODIFIED。

## Proposed Solution

### 1. Spec 改寫

**`agent-core/spec.md`** — MODIFY 既有 Requirement「`_think` window-bounded prompt」改成：

> `_think` MUST compose the provider wire prompt as `[system_message, *windowed_history, user_prompt]` in that exact order; `system_message` MUST be the first entry. The windowed history is `state.messages[-_MESSAGE_ROLLING_WINDOW:]` after stripping any leading messages whose `role == "tool"` (orphan tool messages whose corresponding `assistant` was sliced off MUST be removed before the wire payload is sent to comply with OpenAI Chat Completions ordering rules).

並新增 scenarios：
- system 在 array 首位
- windowed 第一筆為 tool 時被 strip
- assistant tool_calls + tool 連續 pair 在 window 內時兩者都保留
- `_MESSAGE_ROLLING_WINDOW` < state.messages.length 時的 slice 邊界行為

**`qa-agent/spec.md`** — MODIFY 既有 Requirement 補一條 cross-reference：「`_qa_think` MUST follow the same message ordering rules as `_think` defined in agent-core capability」+ 校正 spec 文字「reusing `codebus_agent.agent.explorer._think`」（實作是 `_qa_think` clone，不是嚴格 reuse）。

### 2. Code 修法

sidecar/src/codebus_agent/agent/explorer.py 與 sidecar/src/codebus_agent/agent/qa.py 兩處 fix（同 pattern）：

```python
windowed = state.messages[-_MESSAGE_ROLLING_WINDOW:]
# Trim leading 'tool' messages — each 'tool' must follow an 'assistant'
# with matching tool_calls; window slicing may strip the assistant and
# leave orphan tool messages that OpenAI rejects with 400.
while windowed and windowed[0].role == "tool":
    windowed = windowed[1:]
messages = [
    ProviderMessage(role="system", content=EXPLORER_SYSTEM),
    *_to_provider_messages(windowed),
    ProviderMessage(role="user", content=user_prompt),
]
```

### 3. Defensive 測試（鎖死規則）

新增 sidecar/tests/agent/test_message_ordering.py 用 spy provider（不真打 OpenAI）抓 provider.chat(messages, ...) 收到的 messages list，驗：
- 第一筆 role 是 `system`
- 不存在 orphan `tool`（每個 `tool` 前面有 `assistant tool_calls`）
- user prompt 是最後一筆
- windowed slice 邊界正確

兩 module（explorer / qa）各跑同一份規則。

## Non-Goals

- **不重構 `_qa_think` 真的 reuse `explorer._think`**：spec 寫 reuse 但實作是 clone，這次只校正 spec 文字、不動 code 結構，避免擴大 scope。重構留 future change。
- **不引入 OpenAI ordering validator class**：fix 是 inline trim + reorder，不需新抽象層。
- **不動 `_MESSAGE_ROLLING_WINDOW` 大小**：window 大小 16 是 design 決策，本 change 保留。
- **不動 Generator (`run_generator`)**：Generator 每站獨立 LLM call、無 multi-turn ReAct，message 構造 pattern 不同；如有問題另起 change。
- **不動 `MockProvider`**：MockProvider 是測試替身、無 OpenAI ordering 驗證需求；defensive 測在 spy provider 層。

## Success Criteria

1. sidecar/tests/agent/test_message_ordering.py 全綠（4+ scenarios，鎖死順序規則）
2. 既有 sidecar/tests/agent/test_explorer.py / sidecar/tests/agent/test_qa.py 全綠（regression 0）
3. 既有 sidecar/tests/golden/test_timeline_synthetic_replay.py 全綠（scripted replay 不破）
4. **Phase 7 e2e 重跑**：對 tests/golden/timeline-storage-adapter-synthetic/workspace/ 跑 POST /explore，可順利 run 至 done（不再 400）
5. pre-commit run --all-files 全綠

## Impact

- Affected specs:
  - openspec/specs/agent-core/spec.md（MODIFIED：`_think` message ordering Requirement）
  - openspec/specs/qa-agent/spec.md（MODIFIED：`_qa_think` cross-reference + spec 文字校正）
- Affected code:
  - Modified:
    - sidecar/src/codebus_agent/agent/explorer.py（`_think` 函式 messages 構造段）
    - sidecar/src/codebus_agent/agent/qa.py（`_qa_think` 函式 messages 構造段）
  - New:
    - sidecar/tests/agent/test_message_ordering.py
  - Removed: 無
