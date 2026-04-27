## MODIFIED Requirements

### Requirement: Golden replay harness runs under pytest and fails on drift

The sidecar SHALL carry a pytest harness at `sidecar/tests/golden/test_explorer_replay.py` that invokes the Explorer loop against the `demo-synthetic` fixture using `MockProvider` with a scripted sequence of `ExplorerAction`s and `JudgeVerdict`s, records the outcome, and compares it to `expected.json`. The harness MUST fail the test run when any of the following drift conditions occur:

- The produced `(path, role)` station set does not equal the pinned set.
- The produced `stopped_reason` does not equal the pinned value.
- The produced `step_count` does not equal the pinned value.
- The pinned `judge_prompt_version` does not equal the current `JUDGE_PROMPT_VERSION` at test time.
- The pinned `explorer_prompt_version` does not equal the current `EXPLORER_PROMPT_VERSION` at test time.
- The produced `reasoning_log.jsonl` line count is NOT in the closed range `[step_count, step_count + _COVERAGE_MAX_DEPTH]` (inclusive on both ends).

The reasoning-log line-count tolerance is a direct consequence of the `coverage-gap-recurse` Requirement `Coverage round writes one Step line when gaps are non-empty`: each non-empty coverage round (capped at `_COVERAGE_MAX_DEPTH` rounds total per session) appends one extra `Step` to `reasoning_log.jsonl` beyond the main-loop tally. The tolerance upper bound MUST be sourced from `codebus_agent.agent.explorer._COVERAGE_MAX_DEPTH` (the canonical single source of truth) — the harness MUST NOT hard-code the integer `3` even though that is the current value, so future tweaks to `_COVERAGE_MAX_DEPTH` automatically propagate to the drift condition without requiring a spec / harness re-edit.

The harness MUST locate the fixture directory via `Path(__file__)`-based resolution, not via the process working directory, so the test runs correctly regardless of the invoking shell's cwd.

#### Scenario: Baseline match passes the harness

- **WHEN** the harness runs against `demo-synthetic` and the produced output matches every field in `expected.json`
- **AND** both pinned prompt versions match the live `JUDGE_PROMPT_VERSION` / `EXPLORER_PROMPT_VERSION` values
- **THEN** the pytest MUST pass

#### Scenario: Station set drift fails the harness

- **WHEN** the replay produces a station `(path, role)` pair not present in the pinned set, or omits a pinned pair
- **THEN** the pytest MUST fail
- **AND** the failure message MUST name the differing `(path, role)` entries

#### Scenario: Prompt version drift fails the harness with re-baseline hint

- **WHEN** the current `JUDGE_PROMPT_VERSION` differs from the value pinned in `expected.json`
- **THEN** the pytest MUST fail
- **AND** the failure message MUST include a phrase instructing the implementer to re-baseline (e.g. "re-baseline required" or equivalent) so drift is not silently ignored

#### Scenario: Reasoning log line count within main-loop and coverage-recurse range passes

- **WHEN** the replay writes a number of `reasoning_log.jsonl` lines `L` such that `step_count <= L <= step_count + _COVERAGE_MAX_DEPTH`
- **THEN** the pytest MUST pass the line-count drift check (i.e. extra Step lines from non-empty coverage rounds MUST NOT trigger drift)
- **AND** the tolerance upper bound MUST be obtained by importing `_COVERAGE_MAX_DEPTH` from `codebus_agent.agent.explorer`, NOT by hard-coding the integer literal in the harness

#### Scenario: Reasoning log line count outside the tolerance range fails the harness

- **WHEN** the replay writes either fewer than `step_count` `reasoning_log.jsonl` lines OR more than `step_count + _COVERAGE_MAX_DEPTH` lines
- **THEN** the pytest MUST fail
- **AND** the failure message MUST surface both the observed line count and the expected `[step_count, step_count + _COVERAGE_MAX_DEPTH]` range
