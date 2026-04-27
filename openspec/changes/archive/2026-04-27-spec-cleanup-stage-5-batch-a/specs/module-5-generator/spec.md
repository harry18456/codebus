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
) -> GeneratorResult
```

The four required (no-default) parameters carry:

- `state` — an `ExplorerState` (specifically `state.stations` and `state.task`); the generator iterates `state.stations` in order. The user's free-form tutorial task description lives on `state.task` (NOT a separate top-level parameter), and is consumed by per-station prompts and the title-derivation fallback (`title or state.task`).
- `workspace_root` — the workspace path under which the generator writes `<workspace_root>/codebus-tutorials/{task_id}/...` and the operational log `<workspace_root>/.codebus/generator_log.jsonl`.
- `task_id` — the SSE / file-system run identifier (matching regex `^generate_[0-9a-f]{8}$`); used as the directory name under `codebus-tutorials/`.
- `llm_chat_provider` — a workspace-scoped factory (`Callable[[Path], TrackedProvider]`) returning the chat provider for the supplied workspace path; the returned `TrackedProvider` MUST carry `default_module="generate"` and `role=ProviderRole.CHAT`.

The eleven optional (default-bearing) parameters MUST also be keyword-only and carry their declared defaults; they wire in `kb` (`KnowledgeBase` for KB lookups during per-station context build), `options` (`GeneratorOptions(mode, target_persona)`), Sanitizer Pass 1 dependencies (`sanitizer`, `sanitizer_audit`, `rules_version`), the operational logger (`log`), tutorial metadata (`repo_name`, `workspace_type`, `title`, `duration_minutes_per_station`), and the SSE emitter (`emitter`). Production callers in `codebus_agent.api.generate` populate the optional params from `app.state.*` factories; unit tests construct the call with only the four required params plus whichever optional params the test under question requires.

The function returns a `GeneratorResult(tutorial_path, station_paths, route_path, log_path, degraded_count)`. The function MUST iterate over `state.stations` in order, invoking `_generate_station(station, idx, context)` for each one, then assemble the MOC and write `route.json` after all stations have been written (or marked degraded).

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

`run_generator` MUST NOT short-circuit on per-station failure. A degraded station MUST NOT prevent subsequent stations from being generated. After the loop completes, the MOC assembler runs once and `route.json` is written once.

#### Scenario: Run generator over three scripted stations writes three station files plus MOC plus route

- **WHEN** `run_generator(state=<3 stations with state.task="walk through the storage adapter">, workspace_root=<fresh tmp path>, task_id="generate_abc12345", llm_chat_provider=<factory returning scripted MockProvider>, kb=<empty stub>, options=GeneratorOptions(mode="interactive", target_persona="experienced engineer"))` is called against a fresh workspace
- **THEN** the result `GeneratorResult.station_paths` MUST contain exactly three paths under `<workspace_root>/codebus-tutorials/{task_id}/stations/`
- **AND** `GeneratorResult.tutorial_path` MUST equal `<workspace_root>/codebus-tutorials/{task_id}/tutorial.md`
- **AND** `GeneratorResult.route_path` MUST equal `<workspace_root>/codebus-tutorials/{task_id}/route.json`
- **AND** `GeneratorResult.degraded_count` MUST equal `0` for a clean run
- **AND** every station file MUST exist on disk and parse as a Markdown document with a YAML frontmatter block

#### Scenario: Per-station failure does not abort the run

- **WHEN** the second of three scripted stations causes the validator to return issues on every retry attempt for 3 consecutive retries
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

---
### Requirement: Markdown validator enforces D-029 component rules

The sidecar SHALL implement `validate_station_markdown(md: str, station_idx: int, mode: Literal["interactive", "plain"]) -> ValidationResult` in `codebus_agent.generator.validator`. The validator MUST return a `ValidationResult` with an `issues: list[str]` field (empty list means valid) and a `parsed: dict` field carrying the structured output for downstream consumers (frontmatter `required_checks` field).

The validator MUST enforce, in `interactive` mode:

1. At least one `<Checkpoint id="...">` element with id matching pattern `station-{station_idx}-check` (or that prefix with a suffix when multiple checkpoints exist). Issue: `"missing_checkpoint"`.
2. At most one `<Quiz id="..." correct="..." >` element. Issue when more: `"too_many_quizzes"`.
3. Every `<Quiz>` element MUST have a `correct` attribute whose value is one of `"a"` / `"b"` / `"c"` / `"d"`. Issue: `f"quiz_bad_correct: {value}"`.
4. Every `<Quiz>` element MUST have at least options `a` and `b`. Issue: `"quiz_missing_options"`.
5. Total markdown character count (excluding frontmatter) MUST NOT exceed **800 characters** per `docs/decisions.md` D-029 component-size rules; this 800-char ceiling is the source-of-truth and MUST equal the production constant `_BODY_LIMIT_CHARS` in `codebus_agent.generator.validator`. Issue: `"too_long"`.
6. Every fenced code block MUST contain at most 30 lines. Issue: `"code_block_too_long"`.
7. Every `<CodeRef file="..." lines="...">` element's `file` attribute MUST resolve to a path under `workspace_root` (validator receives `workspace_root` parameter). Issue: `f"coderef_escape: {value}"`.

In `plain` mode, the validator MUST NOT emit `missing_checkpoint` / `too_many_quizzes` / `quiz_bad_correct` / `quiz_missing_options` / `coderef_escape` issues (the corresponding components MUST NOT appear in plain mode output per the plain-mode Requirement). Length and code-block-line rules continue to apply.

#### Scenario: Interactive mode rejects markdown without checkpoint

- **WHEN** the validator receives interactive-mode markdown with no `<Checkpoint>` element
- **THEN** the returned `issues` list MUST contain `"missing_checkpoint"`

#### Scenario: Quiz with bad correct value rejected

- **WHEN** the validator receives markdown with `<Quiz id="s2-q1" correct="e">...</Quiz>`
- **THEN** the returned `issues` list MUST contain `"quiz_bad_correct: e"`

#### Scenario: Length over 800 characters fails validation per D-029

- **WHEN** the validator receives markdown whose body (excluding frontmatter) has 1500 characters (any value strictly greater than the D-029 800-char ceiling)
- **THEN** the returned `issues` list MUST contain `"too_long"`
- **AND** the 800-char ceiling MUST be sourced from the production constant `_BODY_LIMIT_CHARS` rather than a separate hard-coded literal in the test
