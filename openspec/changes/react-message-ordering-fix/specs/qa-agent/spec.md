## MODIFIED Requirements

### Requirement: Q&A loop entry point with two-stage RAG-first flow

The sidecar SHALL expose `codebus_agent.agent.qa.run_qa(*, question, state, kb, tools, provider, logger=None, emitter=None, cancel_event=None) -> QAAnswer` as the Q&A Agent entry point, per `docs/decisions.md` D-016 and `docs/qa-agent.md §四`. All parameters are keyword-only — the function signature MUST NOT accept any positional argument. The `provider` parameter is the workspace-scoped Q&A `TrackedProvider` instance (constructed via the `app.state.llm_qa_provider` factory with `default_module="qa_agent"`); `kb` carries its own embedding `TrackedProvider` constructed via `app.state.kb_query_provider` (`default_module="kb_query"`) so the two lanes write to `token_usage.jsonl` with distinct `module` values. Sanitizer / sanitizer-audit / kb-growth-logger plumbing MUST be threaded through `tools` (specifically the `QATools.add_to_kb` callee's bound `ToolContext`), NOT exposed as top-level `run_qa` parameters.

The function SHALL execute exactly three stages in order: (1) **RAG-first probe** — invoke `kb.query(question, top_k=8)` once and pass the hits through `_hits_confident(question, hits)`; (2) **Optional ReAct loop** — entered only when the probe returns `False`, driven by a Q&A-local `_qa_think` helper that mirrors the shape of `codebus_agent.agent.explorer._think` (re-uses `_execute_tools` and `_should_stop` from the explorer module, but the Think substep itself is a Q&A-specific clone of the explorer Think — see ordering rules below); (3) **Synthesize** — `_synthesize_answer(state, provider)` produces a final `QAAnswer` regardless of whether the loop ran.

The Q&A `_qa_think` helper MUST follow the same provider-wire-prompt ordering rules as `_think` defined in the `agent-core` capability spec ("Explorer applies rolling message window before each Think call"). Specifically: the messages array passed to `provider.chat` MUST start with the Q&A system message (`QA_SYSTEM`) at index `0`, followed by the windowed history `state.messages[-_MESSAGE_ROLLING_WINDOW:]` after stripping any leading `tool` role messages whose corresponding `assistant` was sliced off, followed by the rendered user prompt as the final element. Failure to strip orphan `tool` messages causes a `400 invalid_request_error` from OpenAI Chat Completions ("messages with role 'tool' must be a response to a preceding message with 'tool_calls'") — this is the cross-vendor API contract the order MUST satisfy.

`run_qa` MUST NOT instantiate `LLMJudge` or `LLMCoverageChecker`. The Q&A loop's only stop conditions are budget exhaustion (steps / tokens / wall) and explicit cancellation (signalled via the optional `cancel_event` keyword argument); station-coverage style verdicts are out of scope for Q&A. This isolation is the design surface that prevents Folder-mode prompt vocabulary from leaking into Q&A behavior.

The optional `logger: ReasoningLogger | None` parameter receives the workspace-scoped reasoning logger (constructed by the caller, typically `api/qa.py`, against `<ws>/.codebus/reasoning_log.jsonl`); when supplied, every ReAct iteration MUST flush one `Step` line through it. The optional `emitter: SSEEmitter | None` parameter receives the SSE emitter (typically `TaskHandleEmitter`) used for `rag_hits` / `agent_thought` / `agent_action_result` / `kb_growth` / `qa_answer` events.

#### Scenario: Confident hits skip the ReAct loop

- **WHEN** `run_qa` calls `kb.query(question, top_k=8)` and `_hits_confident(question, hits)` returns `True`
- **THEN** `run_qa` MUST return a `QAAnswer` produced by `_answer_from_hits(question, hits, provider)` without entering the ReAct loop
- **AND** the `reasoning_log.jsonl` MUST contain zero ReAct `Step` entries for that call

#### Scenario: Non-confident hits enter the ReAct loop

- **WHEN** `_hits_confident(question, hits)` returns `False` for the initial probe
- **THEN** `run_qa` MUST seed `state.messages` with the rendered Q&A prompt and proceed into the ReAct loop until `_should_stop(state)` returns `True`

#### Scenario: Q&A never instantiates Judge or Coverage

- **WHEN** the `run_qa` module is imported
- **THEN** the module MUST NOT contain any reference to `LLMJudge`, `LLMCoverageChecker`, `Judge` Protocol, or `CoverageChecker` Protocol — verified by an import-graph test

#### Scenario: All run_qa parameters are keyword-only

- **WHEN** `inspect.signature(run_qa).parameters` is read
- **THEN** every parameter's `kind` MUST equal `inspect.Parameter.KEYWORD_ONLY`

#### Scenario: `_qa_think` provider wire prompt starts with system message

- **WHEN** `_qa_think` is invoked during the Q&A ReAct loop with any `state.messages` length
- **THEN** the `messages` argument passed to `provider.chat` MUST have `messages[0].role == "system"`
- **AND** the system message content MUST equal `QA_SYSTEM`
- **AND** the user prompt MUST appear as the last element (`messages[-1].role == "user"`)

#### Scenario: `_qa_think` strips leading orphan tool messages from windowed history

- **WHEN** `state.messages[-_MESSAGE_ROLLING_WINDOW:]` slice begins with one or more entries whose `role == "tool"` (the corresponding `assistant tool_calls` was sliced off the head of the window)
- **THEN** `_qa_think` MUST strip those leading `tool` entries before they reach `provider.chat`
- **AND** the messages array passed to `provider.chat` MUST NOT contain any `role == "tool"` entry whose immediately preceding entry has neither `role == "assistant"` with non-empty `tool_calls` nor another (non-orphan) `role == "tool"` chained from the same assistant
