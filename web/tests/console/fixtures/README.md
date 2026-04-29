# Console fixtures

> Bootstrap by change `agent-console-p0`. Loaded by Vitest only — these files MUST NOT be referenced from `web/app/**` (production).

## `explorer-stream.json`

A flat JSON array of `{ type, data }` SSE event envelopes matching `useSseTask`'s `SseEvent` surface (events are already JSON-parsed, NOT raw `event:` / `data:` lines). Mirrors a 3-step Explorer run on `tests/golden/demo-synthetic/workspace/` (`a.py` / `b.py` / `c.py`) that stops on `budget_exhausted` after step 3.

| Aspect | Value |
| --- | --- |
| Source of truth | `tests/golden/demo-synthetic/expected.json` (`stopped_reason: budget_exhausted`, `step_count: 3`) |
| `agent_thought` / `agent_action_result` / `judge_verdict` payloads | Hand-crafted placeholders consistent with `openspec/specs/explorer-sse/spec.md` Requirement 2 schema; the demo-synthetic golden does NOT ship a live `reasoning_log.jsonl`. |
| `tokens_used` | Always `0` per `explorer-sse` Requirement 2 P0 placeholder rule (per-tool attribution not yet wired). |
| `budget_warning` | Fires once on step 3 (3/3 = 100%, first iteration to cross the 80% threshold with `budget_steps=3`). |
| `coverage_gaps` | Fires once at end with `skip_reason="budget_exhausted"` to align with the golden's `stopped_reason`. |

### Regenerate

If `tests/golden/demo-synthetic/expected.json` drifts (different station count or stop reason), update both:
1. The iteration count in `explorer-stream.json` (one full ReAct triplet per station).
2. `coverage_gaps.skip_reason` to match the new `stopped_reason`.
