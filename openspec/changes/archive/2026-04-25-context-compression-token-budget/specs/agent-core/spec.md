## MODIFIED Requirements

### Requirement: Explorer loop stops on budget exhaustion, empty queue, or cancel signal

The sidecar SHALL implement a `_should_stop(state, cancel_event)` predicate (internal to `codebus_agent.agent.explorer`) that returns `True` when any of four convergence conditions fires: (a) `cancel_event.is_set()` is True; (b) a caller-supplied `TokenBudgetProbe` (see separate Requirement) reports `total() >= state.budget_tokens_left`; (c) `state.budget_steps_left <= 0`; (d) `state.pending_queue == []` **and** `len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE` (a module-level constant with a sensible P0 default, e.g. 3).

The predicate MUST be evaluated at the top of each loop iteration (before `_think`) so cancel signals abort cleanly without issuing an LLM call. When the predicate returns True, `run_explorer` MUST populate `ExplorerResult.stopped_reason` with exactly one of the documented string values: `"budget_exhausted"`, `"queue_empty"`, `"cancelled"`, or `"budget_tokens_exhausted"`.

The four conditions MUST be evaluated in this precedence order: cancel > token budget > step budget > queue empty. When more than one condition holds simultaneously, the reason string MUST reflect the first one matched, so operators receive the most actionable signal.

Token budget enforcement MUST NOT fire when the caller passes no `TokenBudgetProbe` (the parameter defaults to `None` for backward compatibility with in-process tests and golden-sample replay). Under that path the loop behaves identically to the pre-Requirement revision.

#### Scenario: Budget exhaustion terminates loop

- **WHEN** `run_explorer` is called with `state.budget_steps_left == 0`
- **THEN** the loop body MUST NOT execute even once — no `_think` call MUST fire — and the returned `ExplorerResult.stopped_reason` MUST equal `"budget_exhausted"`

#### Scenario: Cancel event short-circuits mid-run

- **WHEN** a caller sets the `asyncio.Event` passed as `cancel_event` between iterations K and K+1
- **THEN** iteration K+1 MUST NOT invoke `_think`, `_execute_tools`, or `judge.evaluate`; `run_explorer` MUST return `ExplorerResult` with `stopped_reason == "cancelled"` and the stations accumulated through iteration K intact

#### Scenario: Queue empty + enough stations terminates cleanly

- **WHEN** an iteration completes such that `state.pending_queue == []` and `len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE`
- **THEN** the next iteration's `_should_stop` check MUST return True with `stopped_reason == "queue_empty"`

#### Scenario: Token budget exhaustion terminates loop

- **WHEN** `run_explorer` is called with a non-None `token_probe` whose `total()` returns a value `>= state.budget_tokens_left` at the start of iteration K+1
- **THEN** iteration K+1 MUST NOT invoke `_think` and `run_explorer` MUST return `ExplorerResult` with `stopped_reason == "budget_tokens_exhausted"`
- **AND** the stations accumulated through iteration K MUST be preserved intact on the returned result

#### Scenario: Missing token probe leaves token budget unenforced

- **WHEN** `run_explorer` is called with `token_probe=None` (the backward-compatible default)
- **THEN** the `budget_tokens_exhausted` branch MUST NOT fire regardless of `state.budget_tokens_left` value
- **AND** every previously-passing test in `sidecar/tests/agent/` that calls `run_explorer` without a `token_probe` MUST continue to pass with identical terminal behaviour


## ADDED Requirements

### Requirement: Explorer applies rolling message window before each Think call

The sidecar SHALL keep a module-level constant `_MESSAGE_ROLLING_WINDOW: int` in `codebus_agent.agent.explorer` (default value 16) that bounds the number of trailing `state.messages` entries forwarded to `TrackedProvider.chat` during the `_think` substep. The `_think` implementation MUST compose the provider wire prompt from `state.messages[-_MESSAGE_ROLLING_WINDOW:]` plus the `EXPLORER_SYSTEM` system message and the rendered user prompt; earlier entries MUST be dropped from the wire payload only (not from `state.messages`).

The rolling window MUST NOT mutate `state.messages`, `state.visited_files`, `state.stations`, `state.pending_queue`, or any other field of `ExplorerState`. Reasoning-log audit (`reasoning_log.jsonl`) MUST continue to capture the full per-iteration Step record and MUST NOT be abbreviated by the window.

The window MUST apply uniformly across main-loop iterations and across coverage-gap recursion frames (i.e., the recursive `run_explorer` call on `_depth=_depth+1` receives the same slicing behaviour).

Judge and Coverage Checker one-shot calls MUST NOT apply the window: their `render_judge_prompt(state, results)` and `render_coverage_prompt(state)` helpers already bound their own context (visited-files window 20 + `... (N more)` footer, stations tail, ToolResult 800-char truncation). The rolling window is strictly for the cross-iteration Explorer wire path.

#### Scenario: Think receives at most window-size messages when state grew larger

- **WHEN** `run_explorer` completes an iteration that leaves `len(state.messages) > _MESSAGE_ROLLING_WINDOW`
- **THEN** the next iteration's `_think` call MUST pass exactly `_MESSAGE_ROLLING_WINDOW` messages plus the two appended `system` + `user` messages into `provider.chat`
- **AND** the dropped messages (`state.messages[:-_MESSAGE_ROLLING_WINDOW]`) MUST remain on `state.messages` unchanged

#### Scenario: Think preserves all state when message count is below window

- **WHEN** `run_explorer` invokes `_think` with `len(state.messages) <= _MESSAGE_ROLLING_WINDOW`
- **THEN** `provider.chat` MUST receive every entry of `state.messages` plus the two appended `system` + `user` messages
- **AND** no slicing MUST be observable at the provider boundary

#### Scenario: Reasoning log records full iteration history despite windowing

- **WHEN** `run_explorer` writes the Step for an iteration whose wire prompt was windowed
- **THEN** the Step's `tool_results` field MUST contain every `ToolResult` emitted in that iteration in full
- **AND** no Step field MUST reflect the windowed wire prompt (the log is faithful to the iteration, not to the prompt)

#### Scenario: Coverage-gap recursion frame respects the same window

- **WHEN** `run_explorer` recurses into a coverage-gap frame (`_depth=_depth+1`) and that frame's first `_think` call is invoked
- **THEN** the windowing MUST apply identically — `provider.chat` MUST receive `state.messages[-_MESSAGE_ROLLING_WINDOW:]` plus appended `system` + `user` messages
- **AND** the `_enqueue_gap_investigation` user-summary message MUST be visible in the windowed slice (because it is the most recent entry appended to `state.messages` before recursion)
