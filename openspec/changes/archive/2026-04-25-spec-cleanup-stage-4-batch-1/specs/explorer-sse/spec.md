## MODIFIED Requirements

### Requirement: Explorer loop emits agent_thought / agent_action_result / judge_verdict events

The sidecar SHALL extend `run_explorer` to accept an optional `emitter: SSEEmitter | None = None` parameter. When a non-None emitter is injected, each ReAct iteration MUST emit exactly three events in order, matching `docs/sidecar-api.md §四` wire schemas:

1. **After Think** — `{"type": "agent_thought", "step": N, "thought": "<text>", "action": [{"tool": "<name>", "args": {...}}]}` where `action` enumerates the `ExplorerAction.tool_calls` (empty list when the action carries no calls).
2. **After Act** — one `{"type": "agent_action_result", "step": N, "tool": "<name>", "observation": "<first 500 chars>", "tokens_used": <int>}` per `ToolResult`. `observation` MUST be truncated to ≤ 500 characters to prevent channel-flood; failed tools (`ToolResult.error is not None`) MUST also emit but with the truncated error message surfaced in `observation`. P0 implementation MAY emit `tokens_used: 0` as a placeholder until per-tool token attribution lands (currently `ToolResult` does not carry a `tokens_used` field; once it does, the emitter MUST forward the per-tool count). Consumers MUST treat any non-negative integer (including `0`) as valid; `0` does NOT mean "no tokens used", it signals "attribution not yet wired".
3. **After Judge** — `{"type": "judge_verdict", "step": N, "relevance": <float>, "reason": "<text>"}`.

When `emitter is None` the loop behavior MUST be identical to its prior form — no SSE side effects, no AttributeError, no performance regression beyond the negligible `None` check. This optionality preserves backward compatibility with every existing `run_explorer` caller (in-process tests, golden-sample replay, future Q&A integration) that does not wire SSE.

The loop MUST also emit one `{"type": "progress", "phase": "exploring", "current": step_count, "total": initial_budget_steps}` event per iteration so the frontend progress bar stays in sync with the Agent console stream.

#### Scenario: Three event types fire per iteration in order

- **WHEN** `run_explorer(..., emitter=test_emitter)` runs a single iteration with a non-empty `tool_calls` list
- **THEN** `test_emitter.emit` MUST be called with `type="agent_thought"`, then `type="agent_action_result"` (one per tool call), then `type="judge_verdict"`, in that sequence
- **AND** every event MUST carry the same `step` value equal to `state.step_count` at iteration start

#### Scenario: Missing emitter preserves legacy behavior

- **WHEN** `run_explorer(...)` is called without an `emitter` argument (default `None`)
- **THEN** no SSE emission MUST occur and the loop's return value MUST be identical to the pre-SSE form
- **AND** all existing Explorer loop tests MUST pass unchanged

#### Scenario: Observation truncation bounds channel payload

- **WHEN** a tool returns a 10_000-character output
- **THEN** the emitted `agent_action_result.observation` field MUST be at most 500 characters plus a truncation indicator
- **AND** the full output MUST still land in `reasoning_log.jsonl` verbatim

#### Scenario: tokens_used field accepts P0 placeholder zero

- **WHEN** `agent_action_result` events are inspected from a P0-stage Explorer run (no `tokens_used` field on `ToolResult` yet)
- **THEN** the `tokens_used` field on each emitted event MUST be a non-negative integer
- **AND** the value `0` MUST be treated as a valid P0 placeholder rather than a malformed event by every consumer (frontend Agent console, golden replay harness, integration tests)
