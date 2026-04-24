# explorer-golden Specification

## Purpose

TBD - created by archiving change 'explorer-judge-golden'. Update Purpose after archive.

## Requirements

### Requirement: Judge prompt produces station and follow-imports signals

The sidecar SHALL structure `JUDGE_SYSTEM` (in `sidecar/src/codebus_agent/agent/prompts/judge.py`) as a three-section prompt that guides the LLM to produce usable `should_add_station` and `should_follow_imports` verdicts plus a `relevance` float anchored to a five-point scale. The three required sections, in order, are:

1. **Role bounds** — the Judge evaluates a single iteration's `ToolResult`s, runs as a one-shot call, MUST NOT enter a ReAct sub-loop, MUST NOT invoke Explorer tools, and MUST NOT mutate `ExplorerState`.
2. **Station decision criteria** — guidance on when `should_add_station = true` (a new architectural slice, entrypoint, or protocol boundary that is clearly relevant to the task) versus `false` (pure import chains, noise, already-visited files).
3. **Follow-imports decision criteria and relevance anchoring** — guidance on when `should_follow_imports = true` (the `ToolResult` exposes new unvisited symbols or files) versus `false` (visited or clearly unrelated), plus a five-point anchor for `relevance ∈ [0.0, 1.0]`: `0.0` unrelated, `0.3` tangential, `0.5` relevant, `0.8` core, `1.0` entrypoint.

The sidecar SHALL implement `render_judge_prompt(task, results)` (same module) such that the rendered user prompt carries the task string, a visited-files summary (first 20 entries with `... (N more)` truncation), a stations summary (total count plus the last three stations' `role` and `path`), and for each `ToolResult` the tool name, an argument whitelist (`path` / `query` when present), and `output[:800]` (or `error=<msg>` when the result carries an error).

#### Scenario: JUDGE_SYSTEM carries the three required sections

- **WHEN** `JUDGE_SYSTEM` is read from `sidecar/src/codebus_agent/agent/prompts/judge.py`
- **THEN** the string MUST contain a role-bounds section that explicitly states the Judge does not enter a ReAct sub-loop, does not invoke tools, and does not mutate state
- **AND** the string MUST contain a station-decision section describing at least one positive criterion (e.g. new architectural slice / entrypoint / protocol boundary) and at least one negative criterion (e.g. pure import chain / already visited)
- **AND** the string MUST contain a follow-imports section with a `relevance` five-point anchor covering `0.0 / 0.3 / 0.5 / 0.8 / 1.0`

#### Scenario: render_judge_prompt includes visited and stations context

- **WHEN** `render_judge_prompt(task, results)` runs with a non-empty `state.visited_files` and a non-empty `state.stations`
- **THEN** the rendered user prompt MUST reference both the visited-files summary and the stations summary
- **AND** visited files MUST be truncated at 20 entries with a `... (N more)` marker when more than 20 are present

#### Scenario: ToolResult output is truncated at 800 chars

- **WHEN** `render_judge_prompt(task, results)` runs with a `ToolResult` whose `output` is 10_000 characters long
- **THEN** the rendered prompt MUST include at most 800 characters of that output
- **AND** a failed `ToolResult` (non-null `error`) MUST render as `error=<msg>` rather than `output=<...>`


<!-- @trace
source: explorer-judge-golden
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/judge.py
  - tests/golden/demo-synthetic/workspace/src/b.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - docs/decisions.md
  - docs/agent-core.md
  - tests/golden/demo-synthetic/expected.json
  - CLAUDE.md
  - tests/golden/demo-synthetic/workspace/src/a.py
  - tests/golden/demo-synthetic/workspace/src/c.py
tests:
  - sidecar/tests/agent/test_judge_prompt.py
  - sidecar/tests/golden/__init__.py
  - sidecar/tests/golden/test_explorer_replay.py
-->

---
### Requirement: Golden fixture pins expected stations, stopped_reason, step_count, and prompt versions

The repository SHALL carry a golden sample fixture at `tests/golden/demo-synthetic/` whose baseline file `expected.json` pins the Explorer replay's expected output. The baseline JSON SHALL contain exactly these top-level fields:

- `stations` — an array of `{ "path": "<string>", "role": "<string>" }` objects; the replay harness MUST compare pinned vs produced stations as a set of `(path, role)` pairs (order-insensitive, ignoring `relevance`, `why`, and `depends_on`).
- `stopped_reason` — one of `"budget_exhausted"`, `"queue_empty"`, or `"cancelled"`.
- `step_count` — the exact number of ReAct iterations the replay SHALL complete.
- `judge_prompt_version` — the pinned `JUDGE_PROMPT_VERSION` at baseline time.
- `explorer_prompt_version` — the pinned `EXPLORER_PROMPT_VERSION` at baseline time.

#### Scenario: expected.json carries all five load-bearing fields

- **WHEN** `tests/golden/demo-synthetic/expected.json` is read
- **THEN** the JSON MUST contain top-level keys `stations`, `stopped_reason`, `step_count`, `judge_prompt_version`, and `explorer_prompt_version`
- **AND** `stations` MUST be an array whose entries each have a `path` string and a `role` string
- **AND** `stopped_reason` MUST be one of the three allowed string values

#### Scenario: Station comparison ignores relevance, why, and depends_on

- **WHEN** the replay harness produces a `Station` carrying a `relevance` of `0.8` and the baseline pins the same `(path, role)` with no relevance value
- **THEN** the comparison MUST pass as long as `(path, role)` matches


<!-- @trace
source: explorer-judge-golden
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/judge.py
  - tests/golden/demo-synthetic/workspace/src/b.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - docs/decisions.md
  - docs/agent-core.md
  - tests/golden/demo-synthetic/expected.json
  - CLAUDE.md
  - tests/golden/demo-synthetic/workspace/src/a.py
  - tests/golden/demo-synthetic/workspace/src/c.py
tests:
  - sidecar/tests/agent/test_judge_prompt.py
  - sidecar/tests/golden/__init__.py
  - sidecar/tests/golden/test_explorer_replay.py
-->

---
### Requirement: Golden replay harness runs under pytest and fails on drift

The sidecar SHALL carry a pytest harness at `sidecar/tests/golden/test_explorer_replay.py` that invokes the Explorer loop against the `demo-synthetic` fixture using `MockProvider` with a scripted sequence of `ExplorerAction`s and `JudgeVerdict`s, records the outcome, and compares it to `expected.json`. The harness MUST fail the test run when any of the following drift conditions occur:

- The produced `(path, role)` station set does not equal the pinned set.
- The produced `stopped_reason` does not equal the pinned value.
- The produced `step_count` does not equal the pinned value.
- The pinned `judge_prompt_version` does not equal the current `JUDGE_PROMPT_VERSION` at test time.
- The pinned `explorer_prompt_version` does not equal the current `EXPLORER_PROMPT_VERSION` at test time.
- The produced `reasoning_log.jsonl` line count does not equal the pinned `step_count`.

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

#### Scenario: Reasoning log line count mismatch fails the harness

- **WHEN** the replay writes fewer or more `reasoning_log.jsonl` lines than `expected.json.step_count`
- **THEN** the pytest MUST fail


<!-- @trace
source: explorer-judge-golden
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/judge.py
  - tests/golden/demo-synthetic/workspace/src/b.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - docs/decisions.md
  - docs/agent-core.md
  - tests/golden/demo-synthetic/expected.json
  - CLAUDE.md
  - tests/golden/demo-synthetic/workspace/src/a.py
  - tests/golden/demo-synthetic/workspace/src/c.py
tests:
  - sidecar/tests/agent/test_judge_prompt.py
  - sidecar/tests/golden/__init__.py
  - sidecar/tests/golden/test_explorer_replay.py
-->

---
### Requirement: JUDGE_PROMPT_VERSION uses date-version format and bumps with content changes

`JUDGE_PROMPT_VERSION` (in `sidecar/src/codebus_agent/agent/prompts/__init__.py` or the equivalent prompts module) SHALL follow the date-version format `YYYY-MM-DD-N` where `N` is a monotonically increasing integer starting at `1` for each calendar date. The constant MUST be bumped whenever `JUDGE_SYSTEM` or `render_judge_prompt` changes in a way that could alter LLM behaviour, even when the golden baseline is re-pinned in the same commit. `EXPLORER_PROMPT_VERSION` MUST remain unchanged by any change whose scope is limited to Judge prompt work.

#### Scenario: JUDGE_PROMPT_VERSION matches the required regex

- **WHEN** `JUDGE_PROMPT_VERSION` is read
- **THEN** its value MUST match the regular expression `^\d{4}-\d{2}-\d{2}-\d+$`

#### Scenario: EXPLORER_PROMPT_VERSION stays frozen across Judge-only changes

- **WHEN** a change scoped to Judge prompt work is merged
- **THEN** the diff MUST NOT modify `EXPLORER_PROMPT_VERSION`
- **AND** the golden baseline's `explorer_prompt_version` MUST match the unchanged live constant

<!-- @trace
source: explorer-judge-golden
updated: 2026-04-24
code:
  - sidecar/src/codebus_agent/agent/judge.py
  - tests/golden/demo-synthetic/workspace/src/b.py
  - sidecar/src/codebus_agent/agent/prompts/judge.py
  - docs/decisions.md
  - docs/agent-core.md
  - tests/golden/demo-synthetic/expected.json
  - CLAUDE.md
  - tests/golden/demo-synthetic/workspace/src/a.py
  - tests/golden/demo-synthetic/workspace/src/c.py
tests:
  - sidecar/tests/agent/test_judge_prompt.py
  - sidecar/tests/golden/__init__.py
  - sidecar/tests/golden/test_explorer_replay.py
-->
