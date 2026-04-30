## MODIFIED Requirements

### Requirement: Explorer applies rolling message window before each Think call

The sidecar SHALL keep a module-level constant `_MESSAGE_ROLLING_WINDOW: int` in `codebus_agent.agent.explorer` (default value 16) that bounds the number of trailing `state.messages` entries forwarded to `TrackedProvider.chat` during the `_think` substep. The `_think` implementation MUST compose the provider wire prompt as `[system_message, *normalized_history, user_prompt]` in that exact order — `system_message` MUST be the first element of the messages array passed to `provider.chat`. Earlier entries beyond the window MUST be dropped from the wire payload only (not from `state.messages`).

`normalized_history` is `state.messages[-_MESSAGE_ROLLING_WINDOW:]` after rewriting orphan `role == "tool"` entries to `role == "user"` notes. Each `tool` role message in OpenAI Chat Completions MUST follow an `assistant` message containing matching `tool_calls` per the OpenAI API ordering contract; window slicing may strip the preceding `assistant tool_calls`, and the current Explorer architecture never emits `assistant tool_calls` into `state.messages` at all (Instructor consumes the assistant response and only the resulting `ToolResult`s are appended via `_append_observations`). Both situations leave orphan `tool` messages whose immediately preceding entry is neither an `assistant` with non-empty `tool_calls` nor another non-orphan `tool` chained from the same assistant. To remain compatible with OpenAI Chat Completions ordering AND preserve cross-iteration observation visibility, `_think` MUST rewrite each orphan `tool` entry as a `role == "user"` message whose content embeds the original tool name and observation text (so the LLM still sees what the previous iteration observed). Paired `tool` messages (preceded by an `assistant` with `tool_calls`, or by another already-paired `tool` chained from one) MUST pass through unchanged. Sending an orphan `role == "tool"` message in the wire payload causes `400 invalid_request_error` ("messages with role 'tool' must be a response to a preceding message with 'tool_calls'") — the rewrite is the mitigation.

The rolling window MUST NOT mutate `state.messages`, `state.visited_files`, `state.stations`, `state.pending_queue`, or any other field of `ExplorerState`. Reasoning-log audit (`reasoning_log.jsonl`) MUST continue to capture the full per-iteration Step record and MUST NOT be abbreviated by the window or by the orphan-tool rewrite.

The window MUST apply uniformly across main-loop iterations and across coverage-gap recursion frames (i.e., the recursive `run_explorer` call on `_depth=_depth+1` receives the same slicing AND the same orphan-tool rewrite).

Judge and Coverage Checker one-shot calls MUST NOT apply the window: their `render_judge_prompt(state, results)` and `render_coverage_prompt(state)` helpers already bound their own context (visited-files window 20 + `... (N more)` footer, stations tail, ToolResult 800-char truncation). The rolling window is strictly for the cross-iteration Explorer wire path.

#### Scenario: System message is first element of provider.chat payload

- **WHEN** `_think` is invoked with any `state.messages` length (zero or more)
- **THEN** the `messages` argument passed to `provider.chat` MUST have `messages[0].role == "system"`
- **AND** the system message content MUST equal `EXPLORER_SYSTEM`
- **AND** the user prompt MUST appear as the last element (`messages[-1].role == "user"`)

#### Scenario: Orphan tool messages are converted to user notes

- **WHEN** `state.messages[-_MESSAGE_ROLLING_WINDOW:]` slice contains one or more entries whose `role == "tool"` and whose immediately preceding entry (in the slice, walking left-to-right) is neither `role == "assistant"` with non-empty `tool_calls` nor another non-orphan `tool`
- **THEN** `_think` MUST rewrite each such orphan entry as a `role == "user"` message in the wire payload
- **AND** the rewritten user-note's content MUST embed the original `tool` message's content (and `tool_name` when available) so the LLM still sees the observation
- **AND** the messages array passed to `provider.chat` MUST NOT contain any `role == "tool"` entry whose immediately preceding entry has neither `role == "assistant"` with non-empty `tool_calls` nor another (non-orphan) `role == "tool"` chained from the same assistant

#### Scenario: Assistant tool_calls and matching tool messages stay paired inside the window

- **WHEN** the slice `state.messages[-_MESSAGE_ROLLING_WINDOW:]` contains an `assistant` message with `tool_calls` followed by one or more `tool` messages responding to those calls (the assistant is not orphaned)
- **THEN** `_think` MUST NOT rewrite the `assistant` or any of its trailing `tool` messages
- **AND** all of these messages MUST be forwarded to `provider.chat` in their original order with their original roles preserved

#### Scenario: Think receives at most window-size messages when state grew larger

- **WHEN** `run_explorer` completes an iteration that leaves `len(state.messages) > _MESSAGE_ROLLING_WINDOW`
- **THEN** the next iteration's `_think` call MUST pass at most `_MESSAGE_ROLLING_WINDOW` history messages (after orphan-tool rewrite — rewrite preserves length, only changes role / content) plus the prepended `system` message and the appended `user` message into `provider.chat`
- **AND** the dropped messages (`state.messages[:-_MESSAGE_ROLLING_WINDOW]`) MUST remain on `state.messages` unchanged

#### Scenario: Think preserves all state when message count is below window

- **WHEN** `run_explorer` invokes `_think` with `len(state.messages) <= _MESSAGE_ROLLING_WINDOW`
- **THEN** `provider.chat` MUST receive a payload of length `len(state.messages) + 2` (every entry of `state.messages` after orphan-tool rewrite, plus the prepended `system` message and the appended `user` message)
- **AND** no slicing MUST be observable at the provider boundary
- **AND** the orphan-tool rewrite MUST still apply when relevant (rewriting only changes role / content; it does not change the wire payload's length)

#### Scenario: Reasoning log records full iteration history despite windowing

- **WHEN** `run_explorer` writes the Step for an iteration whose wire prompt was windowed or had orphan tool messages rewritten
- **THEN** the Step's `tool_results` field MUST contain every `ToolResult` emitted in that iteration in full
- **AND** no Step field MUST reflect the windowed wire prompt or the rewrite (the log is faithful to the iteration, not to the prompt)

#### Scenario: Coverage-gap recursion frame respects the same window and rewrite

- **WHEN** `run_explorer` recurses into a coverage-gap frame (`_depth=_depth+1`) and that frame's first `_think` call is invoked
- **THEN** the windowing AND the orphan-tool rewrite MUST apply identically — `provider.chat` MUST receive a payload where `messages[0].role == "system"`, the windowed history (after rewrite) is in the middle, and the user prompt is last
- **AND** the `_enqueue_gap_investigation` user-summary message MUST be visible in the windowed slice (because it is the most recent entry appended to `state.messages` before recursion)
