## MODIFIED Requirements

### Requirement: Generator entrypoint orchestrates per-station markdown pipeline

The sidecar SHALL implement an async `run_generator(...)` function in `codebus_agent.generator.runner` whose entire parameter surface is keyword-only — the function signature MUST start with `*` and MUST NOT accept any positional argument. The signature is:

```
async def run_generator(
    *,
    state: ExplorerState,
    workspace_root: Path,
    task_id: str,
    llm_chat_provider: Callable[[Path], Any],
    kb: Any | None = None,
    options: GeneratorOptions | None = None,
    sanitizer: SanitizerEngine | None = None,
    sanitizer_audit: SanitizerAuditLogger | None = None,
    rules_version: str = _DEFAULT_RULES_VERSION,
    log: GeneratorLogger | None = None,
    repo_name: str | None = None,
    workspace_type: str = "folder",
    duration_minutes_per_station: int = _DEFAULT_DURATION_MINUTES,
    title: str | None = None,
    emitter: Any | None = None,
    target_stations: list[str] | None = None,
) -> GeneratorResult
```

The four required (no-default) parameters carry:

- `state` — an `ExplorerState` (specifically `state.stations` and `state.task`); the generator iterates `state.stations` in order. The user's free-form tutorial task description lives on `state.task` (NOT a separate top-level parameter), and is consumed by per-station prompts and the title-derivation fallback (`title or state.task`).
- `workspace_root` — the workspace path under which the generator writes `<workspace_root>/codebus-tutorials/{task_id}/...` and the operational log `<workspace_root>/.codebus/generator_log.jsonl`.
- `task_id` — the SSE / file-system run identifier (matching regex `^generate_[0-9a-f]{8}$`); used as the directory name under `codebus-tutorials/`.
- `llm_chat_provider` — a workspace-scoped factory (`Callable[[Path], TrackedProvider]`) returning the chat provider for the supplied workspace path; the returned `TrackedProvider` MUST carry `default_module="generate"` and `role=ProviderRole.CHAT`.

The twelve optional (default-bearing) parameters MUST also be keyword-only and carry their declared defaults; they wire in `kb` (`KnowledgeBase` for KB lookups during per-station context build), `options` (`GeneratorOptions(mode, target_persona)`), Sanitizer Pass 1 dependencies (`sanitizer`, `sanitizer_audit`, `rules_version`), the operational logger (`log`), tutorial metadata (`repo_name`, `workspace_type`, `title`, `duration_minutes_per_station`), the SSE emitter (`emitter`), and the partial-regen station selector (`target_stations`). Production callers in `codebus_agent.api.generate` populate the optional params from `app.state.*` factories; unit tests construct the call with only the four required params plus whichever optional params the test under question requires.

The function returns a `GeneratorResult(tutorial_path, station_paths, route_path, log_path, degraded_count)`. When `target_stations is None` (the default), the function MUST iterate over ALL of `state.stations` in order, invoking `_generate_station(station, idx, context)` for each one, then assemble the MOC and write `route.json` after all stations have been written (or marked degraded) — this is the original full-tutorial generation path. When `target_stations` is a non-empty list, the function MUST follow the partial-regen path described in the separate Requirement; the partial path MUST NOT touch the MOC nor `route.json`, and MUST NOT regenerate stations whose id is not in the list.

`_generate_station(station, idx, context)` MUST:

1. Assign a stable station id `s{NN}-{slug}` per the separate Requirement on stable id generation
2. Build per-station context (related_files content + KB hits + previous_stations_summary + stable id)
3. Call `provider.chat(messages, response_model=StationMarkdown)` once per attempt where `provider` is the workspace-scoped `llm_chat_provider(workspace_root)` factory result (TrackedProvider with `default_module="generate"`, `role=ProviderRole.CHAT`)
4. Pass the LLM output through the validator pipeline (separate Requirement)
5. Retry up to 3 times if the validator returns issues (the previous attempt's issues MUST feed into the next attempt's prompt as a correction hint)
6. After 3 failed attempts, produce a degraded stub per the degraded-fallback Requirement and stop retrying
7. Pass the final markdown content through Sanitizer Pass 1 per the separate Requirement
8. Render the frontmatter and prepend it to the markdown
9. Write the file to `<workspace_root>/codebus-tutorials/{task_id}/stations/s{NN}-{slug}.md`

`run_generator` MUST NOT short-circuit on per-station failure. A degraded station MUST NOT prevent subsequent stations from being generated. After the loop completes (full mode), the MOC assembler runs once and `route.json` is written once.

#### Scenario: Run generator over three scripted stations writes three station files plus MOC plus route

- **WHEN** `run_generator(state=<3 stations with state.task="walk through the storage adapter">, workspace_root=<fresh tmp path>, task_id="generate_abc12345", llm_chat_provider=<factory returning scripted MockProvider>, kb=<empty stub>, options=GeneratorOptions(mode="interactive", target_persona="experienced engineer"))` is called against a fresh workspace (with `target_stations` left at its default `None`)
- **THEN** the result `GeneratorResult.station_paths` MUST contain exactly three paths under `<workspace_root>/codebus-tutorials/{task_id}/stations/`
- **AND** `GeneratorResult.tutorial_path` MUST equal `<workspace_root>/codebus-tutorials/{task_id}/tutorial.md`
- **AND** `GeneratorResult.route_path` MUST equal `<workspace_root>/codebus-tutorials/{task_id}/route.json`
- **AND** `GeneratorResult.degraded_count` MUST equal `0` for a clean run
- **AND** every station file MUST exist on disk and parse as a Markdown document with a YAML frontmatter block

#### Scenario: Per-station failure does not abort the run

- **WHEN** the second of three scripted stations causes the validator to return issues on every retry attempt for 3 consecutive retries (full mode, `target_stations is None`)
- **THEN** the second station's file MUST be written with frontmatter `degraded: true` (the degraded-fallback Requirement specifies the stub content)
- **AND** the third station MUST still be generated normally
- **AND** `GeneratorResult.degraded_count` MUST equal `1`
- **AND** the MOC and `route.json` MUST still be written

#### Scenario: Generator uses TrackedProvider through llm_chat_provider factory

- **WHEN** the entrypoint receives `llm_chat_provider` from `app.state.llm_chat_provider` (the production factory)
- **THEN** every per-station LLM call MUST flow through a `TrackedProvider` instance with `default_module="generate"` so `<workspace>/.codebus/token_usage.jsonl` lines for this run carry `"module": "generate"`
- **AND** `<workspace>/.codebus/llm_calls.jsonl` MUST receive one wire-payload entry per LLM call (success or failure)
- **AND** the generator MUST NOT call `tracker.record(...)` directly (deduplication contract from `usage-tracker-dedup` archive)

#### Scenario: All run_generator parameters are keyword-only

- **WHEN** `inspect.signature(run_generator).parameters` is read
- **THEN** every parameter's `kind` MUST equal `inspect.Parameter.KEYWORD_ONLY` (no positional arguments are accepted by the public entrypoint)
- **AND** the four required (no-default) parameter names MUST be exactly the set `{"state", "workspace_root", "task_id", "llm_chat_provider"}`
- **AND** `"task"` MUST NOT appear in the parameter set — the user task description is carried on `state.task`, not as a top-level parameter
- **AND** any additional parameters beyond the four required ones MUST also be keyword-only, but their exact set MAY evolve as the audit / wire surface expands; tests SHALL NOT pin the full set by exact equality (this scenario only pins the required-parameter contract)

## ADDED Requirements

### Requirement: Partial regen via target_stations preserves unrelated stations

When `run_generator(...)` receives a non-empty `target_stations: list[str]`, the function SHALL execute partial-regen mode: only the stations whose stable ids appear in the list are regenerated, and the MOC (`tutorial.md`) and `route.json` files MUST NOT be modified. Stations whose ids are NOT in `target_stations` MUST NOT have their `stations/s{NN}-{slug}.md` files touched (no read, no write, no delete) — the on-disk content for unrelated stations MUST be byte-identical before and after the partial run.

The `POST /generate` endpoint SHALL accept the same `target_stations` field on `GenerateRequest` (defaulting to `None`); when present, the field MUST be a `list[str] | None` matching one of `state.stations[*].station_id` for every member, otherwise the endpoint MUST reject with HTTP 400 `GENERATE_TARGET_STATION_INVALID` carrying the offending id.

For each id in `target_stations`, the runner MUST:

1. Look up the `Station` in `state.stations` whose `station_id` matches (after the `s{NN}-{slug}` stable id has been computed)
2. Invoke `_generate_station(...)` with the same context-building rules as full mode (KB hits + related_files + previous_stations_summary)
3. After the LLM produces new markdown, the runner MUST verify the regenerated content's stable station id MATCHES the requested id; on mismatch the runner MUST reject the regen for that station with `GENERATE_STATION_ID_DRIFT` and leave the on-disk file unchanged
4. Pass the new markdown through Sanitizer Pass 1 (same as full mode)
5. Overwrite `<workspace_root>/codebus-tutorials/{task_id}/stations/{stable_id}.md` with the regenerated content + frontmatter
6. Append a `generator_log.jsonl` entry with `mode="partial"` and the regenerated station id

`route.json.stations[*].station_id` order, the MOC link list in `tutorial.md`, and any unrelated station files MUST be preserved verbatim across a partial run. The runner MUST NOT call the MOC assembler nor the route writer in partial mode.

If any `target_stations` id fails (LLM returns issues exceeding retry budget OR Sanitizer Pass 1 rejects), the runner MUST emit `GENERATE_FAILED` for that id (matching the existing error class) and continue regenerating the remaining ids in the list — partial mode does NOT short-circuit on a single station failure, so the user can recover one bad regen without losing progress on the others.

`GeneratorResult` returned by partial mode MUST have `tutorial_path` and `route_path` pointing to the existing (untouched) on-disk paths, `station_paths` listing only the regenerated station files (in the same order as `target_stations`), `degraded_count` reflecting only failures within the targeted set, and `log_path` pointing to the same `generator_log.jsonl` (which now carries the new `mode="partial"` rows in addition to history).

#### Scenario: Partial regen with single station updates only that station file

- **WHEN** a workspace already contains `tutorial.md`, `route.json`, and three station files `s01-overview.md`, `s02-mqtt-client.md`, `s03-storage.md` from a prior full run AND `run_generator(..., target_stations=["s02-mqtt-client"])` is invoked
- **THEN** `s02-mqtt-client.md` MUST be overwritten with newly-generated content
- **AND** `s01-overview.md` MUST be byte-identical to its pre-run content
- **AND** `s03-storage.md` MUST be byte-identical to its pre-run content
- **AND** `tutorial.md` MUST be byte-identical to its pre-run content
- **AND** `route.json` MUST be byte-identical to its pre-run content
- **AND** `GeneratorResult.station_paths` MUST equal `[<path-to>/s02-mqtt-client.md]` (length 1)

#### Scenario: Partial regen with multiple stations writes each but leaves MOC and route untouched

- **WHEN** `run_generator(..., target_stations=["s01-overview", "s03-storage"])` is invoked against the same three-station workspace
- **THEN** `s01-overview.md` and `s03-storage.md` MUST both be overwritten with newly-generated content
- **AND** `s02-mqtt-client.md` MUST be byte-identical to its pre-run content
- **AND** `tutorial.md` MUST be byte-identical to its pre-run content
- **AND** `route.json` MUST be byte-identical to its pre-run content
- **AND** `GeneratorResult.station_paths` MUST equal `[<path-to>/s01-overview.md, <path-to>/s03-storage.md]` in the order requested

#### Scenario: Partial regen with unknown station id is rejected by endpoint

- **WHEN** `POST /generate` receives a body with `target_stations: ["s99-not-real"]` and the corresponding `state.stations` does not contain a station whose stable id will resolve to `s99-not-real`
- **THEN** the endpoint MUST respond HTTP 400 with `{"detail": {"code": "GENERATE_TARGET_STATION_INVALID", "message": "...", "station_id": "s99-not-real"}}`
- **AND** no background task MUST be spawned
- **AND** no on-disk files MUST be modified

#### Scenario: LLM produces drifting station_id during partial regen

- **WHEN** during partial regen of `s02-mqtt-client`, the LLM returns markdown whose stable station id resolves to `s02-something-else` (the slug drifted)
- **THEN** the runner MUST reject the regen with `GENERATE_STATION_ID_DRIFT` for that id
- **AND** `s02-mqtt-client.md` on disk MUST remain byte-identical to its pre-run content (the drifting content is discarded, not written)
- **AND** the `generator_log.jsonl` entry for this attempt MUST record the drift with both the requested and observed ids
- **AND** the runner MUST continue with the remaining ids in `target_stations` (no short-circuit)

#### Scenario: Partial regen log entry distinguishes mode from full run

- **WHEN** `run_generator(..., target_stations=["s02-mqtt-client"])` completes successfully
- **THEN** `<workspace_root>/.codebus/generator_log.jsonl` MUST receive a new line with `"mode": "partial"` and `"station_id": "s02-mqtt-client"` among its fields
- **AND** lines from the prior full run MUST remain on disk (append-only)
- **AND** a subsequent full-mode run on the same workspace MUST emit a new line with `"mode": "full"` (or whatever the existing full-mode label is), preserving the mode discriminator
