## ADDED Requirements

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
