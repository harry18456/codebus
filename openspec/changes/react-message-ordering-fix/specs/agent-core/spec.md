## MODIFIED Requirements

### Requirement: Explorer applies rolling message window before each Think call

The sidecar SHALL keep a module-level constant `_MESSAGE_ROLLING_WINDOW: int` in `codebus_agent.agent.explorer` (default value 16) that bounds the number of trailing `state.messages` entries forwarded to `TrackedProvider.chat` during the `_think` substep. The `_think` implementation MUST compose the provider wire prompt as `[system_message, *windowed_history, user_prompt]` in that exact order — `system_message` MUST be the first element of the messages array passed to `provider.chat`. Earlier entries beyond the window MUST be dropped from the wire payload only (not from `state.messages`).

`windowed_history` is `state.messages[-_MESSAGE_ROLLING_WINDOW:]` after stripping any leading messages whose `role == "tool"`. Each `tool` role message in OpenAI Chat Completions MUST follow an `assistant` message containing matching `tool_calls` per the OpenAI API ordering contract; window slicing may strip the preceding `assistant tool_calls` and leave orphan `tool` messages at the head of `windowed_history`. These orphan messages MUST be removed before the wire payload is sent — leaving them in place causes a `400 invalid_request_error` ("messages with role 'tool' must be a response to a preceding message with 'tool_calls'").

The rolling window MUST NOT mutate `state.messages`, `state.visited_files`, `state.stations`, `state.pending_queue`, or any other field of `ExplorerState`. Reasoning-log audit (`reasoning_log.jsonl`) MUST continue to capture the full per-iteration Step record and MUST NOT be abbreviated by the window or by orphan-tool stripping.

The window MUST apply uniformly across main-loop iterations and across coverage-gap recursion frames (i.e., the recursive `run_explorer` call on `_depth=_depth+1` receives the same slicing behaviour, including the leading-orphan-tool strip).

Judge and Coverage Checker one-shot calls MUST NOT apply the window: their `render_judge_prompt(state, results)` and `render_coverage_prompt(state)` helpers already bound their own context (visited-files window 20 + `... (N more)` footer, stations tail, ToolResult 800-char truncation). The rolling window is strictly for the cross-iteration Explorer wire path.

#### Scenario: System message is first element of provider.chat payload

- **WHEN** `_think` is invoked with any `state.messages` length (zero or more)
- **THEN** the `messages` argument passed to `provider.chat` MUST have `messages[0].role == "system"`
- **AND** the system message content MUST equal `EXPLORER_SYSTEM`
- **AND** the user prompt MUST appear as the last element (`messages[-1].role == "user"`)

#### Scenario: Leading orphan tool messages are stripped from windowed history

- **WHEN** `state.messages[-_MESSAGE_ROLLING_WINDOW:]` slice begins with one or more entries whose `role == "tool"` (because the corresponding `assistant` with `tool_calls` was sliced off the head of the window)
- **THEN** `_think` MUST strip those leading `tool` entries before they reach `provider.chat`
- **AND** the messages array passed to `provider.chat` MUST NOT contain any `role == "tool"` entry whose immediately preceding entry has neither `role == "assistant"` with non-empty `tool_calls` nor another (non-orphan) `role == "tool"` chained from the same assistant

#### Scenario: Assistant tool_calls and matching tool messages stay paired inside the window

- **WHEN** the slice `state.messages[-_MESSAGE_ROLLING_WINDOW:]` starts with an `assistant` message that contains `tool_calls` followed by one or more `tool` messages responding to those calls (the assistant is the first entry of the window, not orphaned)
- **THEN** `_think` MUST NOT strip the leading `assistant` or any of its trailing `tool` messages
- **AND** all of these messages MUST be forwarded to `provider.chat` in their original order

#### Scenario: Think receives at most window-size messages when state grew larger

- **WHEN** `run_explorer` completes an iteration that leaves `len(state.messages) > _MESSAGE_ROLLING_WINDOW`
- **THEN** the next iteration's `_think` call MUST pass at most `_MESSAGE_ROLLING_WINDOW` history messages (after orphan-tool stripping) plus the prepended `system` message and the appended `user` message into `provider.chat`
- **AND** the dropped messages (`state.messages[:-_MESSAGE_ROLLING_WINDOW]`) MUST remain on `state.messages` unchanged

#### Scenario: Think preserves all state when message count is below window

- **WHEN** `run_explorer` invokes `_think` with `len(state.messages) <= _MESSAGE_ROLLING_WINDOW` and the head of `state.messages` is not an orphan `tool`
- **THEN** `provider.chat` MUST receive every entry of `state.messages` plus the prepended `system` message and the appended `user` message
- **AND** no slicing or orphan-tool stripping MUST be observable at the provider boundary

#### Scenario: Reasoning log records full iteration history despite windowing

- **WHEN** `run_explorer` writes the Step for an iteration whose wire prompt was windowed or had orphan tool messages stripped
- **THEN** the Step's `tool_results` field MUST contain every `ToolResult` emitted in that iteration in full
- **AND** no Step field MUST reflect the windowed wire prompt or the stripping (the log is faithful to the iteration, not to the prompt)

#### Scenario: Coverage-gap recursion frame respects the same window and stripping

- **WHEN** `run_explorer` recurses into a coverage-gap frame (`_depth=_depth+1`) and that frame's first `_think` call is invoked
- **THEN** the windowing AND the leading-orphan-tool stripping MUST apply identically — `provider.chat` MUST receive a payload where `messages[0].role == "system"`, the windowed history (after stripping) is in the middle, and the user prompt is last
- **AND** the `_enqueue_gap_investigation` user-summary message MUST be visible in the windowed slice (because it is the most recent entry appended to `state.messages` before recursion)
