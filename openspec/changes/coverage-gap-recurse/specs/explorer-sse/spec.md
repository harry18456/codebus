## ADDED Requirements

### Requirement: Coverage round emits coverage_gaps SSE event

The sidecar SHALL emit exactly one `coverage_gaps` SSE event per coverage round through the `run_explorer` emitter. The event MUST fire after `coverage.check(state)` returns and before recursion is decided (including the decision not to recurse). When `emitter is None` the event MUST NOT be emitted, preserving the legacy file-only behaviour for in-process tests and golden-sample replay.

The event envelope MUST match this wire schema:

```json
{
  "type": "coverage_gaps",
  "round": <int>,
  "gaps": [
    {"description": "<str>", "suggested_target": "<str|null>"}
  ],
  "will_recurse": <bool>,
  "skip_reason": "<str|null>"
}
```

Field semantics:

- `round` MUST equal the `_depth` value at which `coverage.check` was invoked (0-indexed; the first coverage round after the outermost `run_explorer` is `round=0`).
- `gaps` MUST be a JSON array of `Gap.model_dump()` outputs (each a `{description, suggested_target}` object). An empty array signals no gap found.
- `will_recurse` MUST be `true` if and only if all three recursion preconditions hold: `len(gaps) > 0`, `state.budget_steps_left > 0`, and `_depth < _COVERAGE_MAX_DEPTH`.
- `skip_reason` MUST be `null` when `will_recurse=true`. When `will_recurse=false`, it MUST be exactly one of `"no_gaps"` (gaps empty), `"budget_exhausted"` (budget at or below 0 with gaps present), or `"max_depth_reached"` (`_depth` at or above `_COVERAGE_MAX_DEPTH` with gaps and budget otherwise sufficient). When more than one blocking condition holds simultaneously, `skip_reason` MUST follow this precedence: `no_gaps` > `max_depth_reached` > `budget_exhausted`.

#### Scenario: coverage_gaps event fires after check returns gaps and before recursion

- **WHEN** `run_explorer(..., emitter=test_emitter, _depth=0)` completes its main loop, `coverage.check(state)` returns two `Gap` entries, `state.budget_steps_left == 4`, and `_COVERAGE_MAX_DEPTH == 3`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `round=0`, `gaps` a two-element list, `will_recurse=true`, and `skip_reason=null`
- **AND** the event MUST be emitted before the recursive `run_explorer(..., _depth=1)` call begins

#### Scenario: Empty gaps still emit with skip_reason="no_gaps"

- **WHEN** `run_explorer(..., emitter=test_emitter)` completes its main loop and `coverage.check(state)` returns `[]`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `gaps=[]`, `will_recurse=false`, and `skip_reason="no_gaps"`
- **AND** no recursion MUST occur

#### Scenario: Budget-exhausted round emits skip_reason="budget_exhausted"

- **WHEN** `run_explorer(..., emitter=test_emitter)` completes with `state.budget_steps_left == 0` and `coverage.check(state)` returns one `Gap`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `gaps` a one-element list, `will_recurse=false`, and `skip_reason="budget_exhausted"`

#### Scenario: Max-depth round emits skip_reason="max_depth_reached"

- **WHEN** `run_explorer(..., emitter=test_emitter, _depth=2)` completes with `state.budget_steps_left == 10`, `_COVERAGE_MAX_DEPTH == 3`, and `coverage.check(state)` returns one `Gap`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `gaps` a one-element list, `will_recurse=false`, and `skip_reason="max_depth_reached"`

#### Scenario: Missing emitter preserves legacy behavior

- **WHEN** `run_explorer(...)` runs with `emitter=None` and a coverage round fires
- **THEN** no SSE emission MUST occur for the coverage round
- **AND** the recursive-call decision (recurse / return) MUST be unchanged relative to the emitter-set case
