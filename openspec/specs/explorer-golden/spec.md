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


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
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

---
### Requirement: Golden scoring helpers compute recall, noise, and composite score

The sidecar test suite SHALL expose three pure scoring functions and one Pydantic schema in `sidecar/tests/golden/scoring.py` that any future golden fixture (Explorer, Q&A, Generator) can reuse to evaluate produced station sets against handwritten ideal routes per design decision D-006.

`station_recall(produced_paths: set[str], must_have_paths: set[str]) -> float` MUST return `len(produced_paths & must_have_paths) / len(must_have_paths)` as a float in `[0.0, 1.0]`. When `must_have_paths` is empty the function MUST raise `ValueError` with the message `"must_have_paths cannot be empty"` so callers cannot silently divide by zero.

`station_noise(produced_paths: set[str], must_have: set[str], nice_to_have: set[str]) -> float` MUST compute `extras = produced_paths - must_have` and return `len(extras - nice_to_have) / len(extras)`. When `extras` is empty the function MUST return `0.0` (no extras → no noise; this is a valid clean output, not an error condition).

`composite_score(recall: float, noise: float, depth: float, weights: dict[str, float] | None = None) -> float` MUST apply the D-006 formula `w_recall * recall + w_noise * (1 - noise) + w_depth * depth` with default weights `{"recall": 0.5, "noise": 0.3, "depth": 0.2}`. Callers MAY pass an override dict containing all three keys; passing partial keys MUST raise `KeyError` so silent default substitution does not mask tuning intent.

`IdealRoute` (Pydantic v2 `BaseModel` subclass) MUST declare exactly four fields with these types: `task: str` (free-text task description), `must_have: list[str]` (paths the agent MUST visit), `nice_to_have: list[str]` (paths that earn no penalty if visited), `noise_paths: list[str]` (paths that explicitly should NOT be visited; reserved for future drift-guard use). The class MUST round-trip via `model_dump_json()` / `model_validate_json()` without data loss so `tests/golden/<fixture>/ideal-route.json` files are the canonical machine-readable source of truth.

The scoring module MUST NOT import from `codebus_agent.*` production packages other than what `IdealRoute` requires (`pydantic`); it MUST stay test-time-only and never appear in any production code path.

#### Scenario: station_recall returns 1.0 on perfect hit

- **WHEN** `station_recall({"a.py", "b.py", "c.py"}, {"a.py", "b.py", "c.py"})` is called
- **THEN** the result MUST equal `1.0`

#### Scenario: station_recall returns partial fraction on partial hit

- **WHEN** `station_recall({"a.py", "x.py"}, {"a.py", "b.py", "c.py"})` is called
- **THEN** the result MUST equal `1.0 / 3.0` (one of three pinned must_have entries was hit)

#### Scenario: station_recall raises on empty must_have

- **WHEN** `station_recall(set(), set())` is called
- **THEN** the function MUST raise `ValueError` with a message containing `"must_have_paths cannot be empty"`

#### Scenario: station_noise treats nice_to_have as not-noise

- **WHEN** `station_noise({"a.py", "n.py"}, must_have={"a.py"}, nice_to_have={"n.py"})` is called
- **THEN** `extras` MUST equal `{"n.py"}` and the function MUST return `0.0` (the only extra is in nice_to_have, so noise is zero)

#### Scenario: station_noise returns zero when extras is empty

- **WHEN** `station_noise({"a.py"}, must_have={"a.py"}, nice_to_have=set())` is called
- **THEN** `extras` MUST be empty and the function MUST return `0.0` (no extras → no noise, NOT an error)

#### Scenario: composite_score applies default D-006 weights

- **WHEN** `composite_score(recall=1.0, noise=0.0, depth=1.0)` is called with no `weights` override
- **THEN** the result MUST equal `0.5 * 1.0 + 0.3 * (1 - 0.0) + 0.2 * 1.0` (= `1.0`)

#### Scenario: composite_score requires all three weight keys when overridden

- **WHEN** `composite_score(recall=0.8, noise=0.2, depth=0.5, weights={"recall": 0.6})` is called
- **THEN** the function MUST raise `KeyError` (partial weights are rejected to surface tuning intent)

#### Scenario: IdealRoute round-trips through JSON

- **WHEN** `IdealRoute(task="t", must_have=["a"], nice_to_have=["b"], noise_paths=["c"])` is dumped via `model_dump_json()` and re-loaded via `model_validate_json(...)`
- **THEN** the resulting instance MUST equal the original (all four fields preserved bit-identically)

<!-- @trace
source: golden-sample-baseline
updated: 2026-04-25
-->

---
### Requirement: Timeline-storage-adapter-synthetic fixture pins ideal-route stations

The repository SHALL ship a `tests/golden/timeline-storage-adapter-synthetic/` fixture mirroring the Storage Adapter topology described in `tests/golden/timeline-gdrive-adapter/ideal-route.md`, expressed as a runnable workspace plus a machine-readable ideal route. The fixture serves as the first concrete test bed for `Golden scoring helpers compute recall, noise, and composite score`.

The fixture root MUST contain at least the following files (additional support files are permitted but each MUST appear in exactly one of the `must_have` / `nice_to_have` / `noise_paths` lists in `ideal-route.json` — orphan files outside the schema are forbidden so drift-guards cover the entire workspace):

- `README.md` — describes the fixture, links back to `ideal-route.md`, and warns readers that this is synthetic, not a real repository mirror
- `ideal-route.json` — `IdealRoute` instance (see scoring Requirement) with all four fields populated
- `workspace/app/types/index.ts` — declares `IStorageService` interface stub with at least one method signature
- `workspace/app/services/MockStorageAdapter.ts` — in-memory adapter implementation stub
- `workspace/app/services/LocalFileAdapter.ts` — file-system adapter implementation stub
- `workspace/app/composables/useStorage.ts` — composable that initialises and exposes the active adapter
- `workspace/app/stores/timeline.ts` — Pinia store consuming `useStorage()` (primary consumer)
- `workspace/app/stores/node.ts` — secondary consumer (nice_to_have)
- `workspace/app/stores/settings.ts` — secondary consumer (nice_to_have)
- `workspace/app/components/EventCard.vue` — UI component (noise; should NOT appear in agent's stations)
- `workspace/README.md` — repo readme (noise)

`ideal-route.json` MUST classify exactly the five `workspace/app/types/...` / `workspace/app/services/...` / `workspace/app/composables/...` / `workspace/app/stores/timeline.ts` paths as `must_have` (these are the load-bearing files in `ideal-route.md` section "必達"), the two secondary store paths as `nice_to_have`, and the EventCard.vue + README.md paths as `noise_paths`.

Workspace files MUST stay below 40 lines each and MAY use `// stub` placeholders rather than valid TypeScript — Scanner / Explorer tools operate at the file-system layer and do not compile TypeScript, so syntactic validity is not required. The constraint is that grep-style search must be able to locate identifying strings (`IStorageService`, `MockStorageAdapter`, `LocalFileAdapter`, `useStorage`).

Total fixture size (including README + ideal-route + all workspace files) MUST stay under 500 lines so the fixture can be reviewed in a single PR diff.

#### Scenario: Fixture provides exactly five must_have entries

- **WHEN** `IdealRoute.model_validate_json(...)` loads `tests/golden/timeline-storage-adapter-synthetic/ideal-route.json`
- **THEN** `must_have` MUST contain exactly five paths
- **AND** every `must_have` path MUST be a relative path under `workspace/app/`

#### Scenario: Fixture nice_to_have list captures secondary consumers

- **WHEN** `IdealRoute.model_validate_json(...)` loads the fixture's `ideal-route.json`
- **THEN** `nice_to_have` MUST contain at least two paths
- **AND** every `nice_to_have` path MUST be distinct from every `must_have` path (no overlap)

#### Scenario: Fixture noise_paths list captures off-route files

- **WHEN** `IdealRoute.model_validate_json(...)` loads the fixture's `ideal-route.json`
- **THEN** `noise_paths` MUST contain at least one UI component or documentation path
- **AND** every `noise_paths` entry MUST be distinct from every `must_have` and `nice_to_have` path (no overlap)

#### Scenario: All workspace files appear in the ideal route schema

- **WHEN** the fixture's `workspace/` directory tree is enumerated and intersected with `must_have ∪ nice_to_have ∪ noise_paths`
- **THEN** every `workspace/`-rooted file MUST appear in exactly one of the three lists (no orphans, no duplicates)

<!-- @trace
source: golden-sample-baseline
updated: 2026-04-25
-->

---
### Requirement: Full-stack golden replay wires Coverage, token probe, and SSE emitter

The sidecar SHALL ship a golden replay harness at `sidecar/tests/golden/test_timeline_synthetic_replay.py` that drives `run_explorer` against the `timeline-storage-adapter-synthetic` fixture with the entire Module 4 stack wired in production-shape: scripted reasoning + judge MockProviders, an `LLMCoverageChecker` over a scripted coverage MockProvider, an `AggregatedTokenProbe` aggregating all three TrackedProvider instances, and a spy `SSEEmitter` that captures every emitted event for assertion.

Scripted reasoning actions MUST be designed so the agent visits all five `must_have` paths from `ideal-route.json` exactly once each across five iterations; judge verdicts MUST all carry `should_add_station=True` and `should_follow_imports=True`. The latter keeps `pending_queue` non-empty across iterations — without it, `_MIN_STATIONS_FOR_CONVERGENCE=3` triggers the `queue_empty` stop branch at iter 4 (once three stations have accumulated and the queue is still empty), cutting the run short of all five must_have paths. With `should_follow_imports=True` the queue grows by one stale `echo` entry per iter (matching `_update_state`'s queue-append branch), surviving until `budget_steps=5` exhausts naturally.

The harness MUST assert the following invariants on a successful run:

- `station_recall(produced_paths, must_have)` MUST equal `1.0` (every must_have path was visited)
- `station_noise(produced_paths, must_have, nice_to_have)` MUST equal `0.0` (no off-route stations)
- `composite_score(recall, noise, depth=1.0)` MUST be `>= 0.9` (`depth` is a placeholder pending Module 5 dep-chain landing per design Decision 6 in `context-compression-token-budget`)
- The spy emitter MUST receive at least one event with `type="coverage_gaps"` (proves the coverage round actually fires through `LLMCoverageChecker` rather than the `_NoopCoverage` shortcut)
- The spy emitter MUST receive exactly one `budget_warning` event with `kind="steps"` at the natural `consumed=4` / `step=4` boundary (`current=4` / `budget=5` / `pct=0.8`) — this matches `context-compression-token-budget`'s pinned `>=` threshold semantics and is verified by the existing unit suite at `sidecar/tests/agent/test_budget_warning_event.py::test_first_iteration_crossing_step_threshold_emits_warning`. The spy emitter MUST receive zero events with `kind="tokens"` (the 10_000-token budget keeps `session_total_tokens` well below the 8_000 threshold across five scripted MockProvider iterations). The dual assertion is a forward-looking drift guard against future `_BUDGET_WARNING_PCT` / token estimator / prompt-size changes
- Every captured `usage_delta` event MUST carry a non-negative integer `session_total_tokens` field (proves `context-compression-token-budget`'s additive field threads through under aggregator wiring)

The harness MUST NOT modify the existing `sidecar/tests/golden/test_explorer_replay.py` file or the `tests/golden/demo-synthetic/expected.json` baseline — the new replay is additive; the demo-synthetic five-field rollout pinned by `explorer-judge-golden` stays exactly as-is.

The harness MUST NOT depend on a live LLM (no `OpenAIChatProvider` instances, no network I/O); future live-snapshot replay is explicitly out of scope and will land in a follow-up change per D-006's `[ ] 打磨期` checklist.

#### Scenario: Replay achieves recall 1.0 on the synthetic Timeline fixture

- **WHEN** the harness runs `run_explorer` against `timeline-storage-adapter-synthetic` with scripted MockProviders that visit all five must_have paths
- **THEN** the produced station path set MUST equal the `must_have` set from `ideal-route.json`
- **AND** `station_recall(produced, must_have)` MUST evaluate to exactly `1.0`

#### Scenario: Replay reports zero noise on a clean run

- **WHEN** the harness produces stations matching `must_have` only (no nice_to_have, no noise paths)
- **THEN** `station_noise(produced, must_have, nice_to_have)` MUST evaluate to exactly `0.0`

#### Scenario: Composite score crosses 0.9 threshold under default weights

- **WHEN** the harness computes `composite_score(recall=1.0, noise=0.0, depth=1.0)` with default D-006 weights
- **THEN** the score MUST be `>= 0.9` (in fact equal to `1.0` under the design's placeholder depth)

#### Scenario: Coverage round emits coverage_gaps event under spy emitter

- **WHEN** the harness's spy emitter is wired through `set_emitter` on every TrackedProvider plus the `LLMCoverageChecker` and the run completes
- **THEN** the captured event list MUST contain at least one entry with `type="coverage_gaps"`
- **AND** that entry's `will_recurse` field MUST equal `false` and `skip_reason` MUST equal `"no_gaps"` (scripted coverage returns empty gaps so recursion does not fire)

#### Scenario: Five-step run emits exactly one steps budget_warning at the 80% boundary

- **WHEN** the harness runs with `budget_steps=5`, `budget_tokens_left=10_000`, and a token probe whose `session_total_tokens` total stays well under `8_000` across all five iterations
- **THEN** the captured event list MUST contain exactly one entry with `type="budget_warning"`, `kind="steps"`, `current=4`, `budget=5`, `pct=0.8` (production fires when `consumed/initial >= 0.8`; `4/5 = 0.8` is the natural boundary on a five-step run that drains all five iterations — see `sidecar/tests/agent/test_budget_warning_event.py::test_first_iteration_crossing_step_threshold_emits_warning` for the pinned unit-level behaviour)
- **AND** the captured event list MUST contain zero entries with `type="budget_warning"` and `kind="tokens"` (token consumption stays under the 80% threshold across the run)
- **AND** the run's `result.stopped_reason` MUST equal `"budget_exhausted"` (the loop drains all five iterations)

#### Scenario: usage_delta events carry session_total_tokens additive field

- **WHEN** the harness collects every `usage_delta` event from the spy emitter
- **THEN** every event MUST contain a `session_total_tokens` key
- **AND** every event's `session_total_tokens` value MUST be a non-negative `int`

<!-- @trace
source: golden-sample-baseline
updated: 2026-04-25
-->
