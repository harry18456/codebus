# module-5-generator Specification

## Purpose

TBD - created by archiving change 'module-5-generator-p0'. Update Purpose after archive.

## Requirements

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


<!-- @trace
source: spec-cleanup-stage-5-batch-a
updated: 2026-04-27
code:
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_station_id_constant.py
-->

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


<!-- @trace
source: spec-cleanup-stage-5-batch-a
updated: 2026-04-27
code:
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_station_id_constant.py
-->

---
### Requirement: Stable station id generation produces s{NN}-{slug} with collision handling

The sidecar SHALL implement `generate_station_id(station_index: int, station_title: str, existing_ids: set[str]) -> str` in `codebus_agent.generator.stable_id`. The function MUST return a string of form `s{NN}-{slug}` where `{NN}` is a zero-padded 2-digit representation of `station_index` (1-based) and `{slug}` is a kebab-case slug derived from `station_title`.

Slug generation rules (in order):

1. Lowercase the title
2. Replace any character not in `[a-z0-9]` with a single `-`
3. Collapse consecutive `-` into one
4. Strip leading and trailing `-`
5. Truncate to at most 40 characters at a `-` boundary (or hard truncate if no boundary exists)
6. If the resulting slug is empty (e.g., title was all CJK characters), use the literal string `"station"` as the slug

Collision handling: if the candidate `s{NN}-{slug}` is already in `existing_ids`, append `-2`. If `s{NN}-{slug}-2` also collides, try `-3`, then `-4`, etc.

The stable id MUST be immutable once a station file is written: subsequent re-runs of `run_generator` against the same workspace MUST NOT change a station's id. (D-029 §十六.2 invariant. Implementation may load existing `route.json` to populate `existing_ids` and preserve prior assignments.)

#### Scenario: ASCII title produces clean slug

- **WHEN** `generate_station_id(station_index=2, station_title="Storage Interface Contract", existing_ids=set())` is called
- **THEN** the returned id MUST equal `"s02-storage-interface-contract"`

#### Scenario: CJK-only title falls back to "station"

- **WHEN** `generate_station_id(station_index=1, station_title="儲存介面契約", existing_ids=set())` is called
- **THEN** the returned id MUST equal `"s01-station"`

#### Scenario: Slug longer than 40 chars truncates at boundary

- **WHEN** the slug-generation pipeline produces an intermediate string of length 60 with a `-` at position 35
- **THEN** the final slug MUST be the substring up to (but not including) that `-`, length 35

#### Scenario: Collision appends -2 suffix

- **WHEN** `generate_station_id(station_index=3, station_title="Storage Interface Contract", existing_ids={"s03-storage-interface-contract"})` is called
- **THEN** the returned id MUST equal `"s03-storage-interface-contract-2"`

#### Scenario: Index zero-padded to two digits

- **WHEN** `generate_station_id(station_index=7, station_title="x", existing_ids=set())` is called
- **THEN** the returned id MUST start with the prefix `"s07-"`


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: Frontmatter renderer produces D-029 schema_version 1 YAML

The sidecar SHALL implement `render_frontmatter(meta: Frontmatter) -> str` in `codebus_agent.generator.frontmatter`. The output MUST be a YAML block delimited by `---` lines (one before the YAML content, one after) suitable for prepending to a Markdown file.

The `Frontmatter` Pydantic model MUST declare exactly the following fields (some required, some optional per D-029 §7.3):

Required: `schema_version: int` (always `1` in P0), `station_id: str`, `station_index: int`, `title: str`, `duration_minutes: int`, `workspace_type: Literal["folder", "topic"]`, `repo_name: str`, `task: str`, `generated_at: datetime`, `required_checks: list[str]`, `degraded: bool`.

Optional: `tags: list[str]`, `related_stations: list[str]`, `related_files: list[str]`.

The renderer MUST emit fields in the order listed above. Optional fields MUST be omitted when their value is `None` or an empty list (rather than rendering as `tags: []`). The `generated_at` field MUST be ISO-8601 with timezone (e.g., `2026-04-25T10:30:00+00:00`).

The schema MUST be additive across versions: future changes adding new optional fields MUST NOT bump `schema_version`. Removing fields or changing existing field types MUST bump `schema_version` and document migration in the change's design.md.

#### Scenario: Required fields rendered in order

- **WHEN** `render_frontmatter(Frontmatter(schema_version=1, station_id="s02-storage", station_index=2, title="Storage", duration_minutes=15, workspace_type="folder", repo_name="timeline", task="add gdrive", generated_at=<2026-04-25T00:00:00Z>, required_checks=["station-2-check"], degraded=False))` is called
- **THEN** the returned string MUST start with `---\n` and end with `\n---\n`
- **AND** the YAML content between delimiters MUST contain `schema_version: 1` as the first key
- **AND** the order MUST be: `schema_version`, `station_id`, `station_index`, `title`, `duration_minutes`, `workspace_type`, `repo_name`, `task`, `generated_at`, `required_checks`, `degraded`

#### Scenario: Optional empty lists are omitted

- **WHEN** `render_frontmatter(...)` is called with `tags=[]`, `related_stations=[]`, `related_files=[]`
- **THEN** the returned YAML MUST NOT contain the `tags:`, `related_stations:`, or `related_files:` keys

#### Scenario: Optional populated lists are rendered

- **WHEN** `render_frontmatter(...)` is called with `tags=["architecture", "interfaces"]`
- **THEN** the returned YAML MUST contain a `tags:` key with the two values as a list


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: MOC assembler writes pure-index tutorial.md with standard markdown links

The sidecar SHALL implement `assemble_moc(*, task: str, total_minutes: int, generated_at: datetime, workspace_name: str, station_summaries: list[StationSummary], mode: Literal["interactive", "plain"], output_path: Path) -> None` in `codebus_agent.generator.moc`. The function MUST write a Markdown file to `output_path` (which equals `<workspace_root>/codebus-tutorials/{task_id}/tutorial.md`).

The MOC content MUST contain:

1. An H1 heading with the task name plus the literal suffix ` — CodeBus 學習教材` (the locale-appropriate phrase per `docs/module-5-generator.md` §7.1)
2. A blockquote section listing `目標`, `預估時長`, `產出時間`, `Repo` metadata
3. An H2 heading `🚌 路線總覽` (route overview)
4. A numbered list where each item is `🚏 [{station_title}](./stations/{station_id}.md)（{duration} min）` — using **standard Markdown link syntax**, NOT wikilinks (D-029 §十六.1 invariant)
5. A horizontal rule and an H2 heading `🎯 下車（完成）`
6. In `interactive` mode only: a `<QAEntry prompt="整條路線我最想再追一下的是：">繼續問 Q&A Agent</QAEntry>` block
7. In `plain` mode: replace the `<QAEntry>` block with the literal sentence `本專案有 Q&A 功能可對話式繼續學習。`

The MOC MUST NOT duplicate station body content (purely an index — D-029 §十六.3 invariant). The MOC MUST NOT contain `<Checkpoint>` / `<Quiz>` / `<CodeRef>` / `<Reveal>` elements (those live in station files only).

#### Scenario: Interactive MOC contains numbered station list with standard markdown links

- **WHEN** `assemble_moc(...)` is called with three station summaries `[("s01-overview", "Repo Overview", 10), ("s02-storage", "Storage", 15), ("s03-adapter", "Adapter Pattern", 20)]` in interactive mode
- **THEN** the written file MUST contain a numbered list with three items
- **AND** each item MUST match the regex `^\d+\.\s+🚏\s+\[.*\]\(\.\/stations\/s\d{2}-[a-z0-9-]+\.md\)`
- **AND** the file MUST NOT contain any `[[...]]` wikilink syntax

#### Scenario: Interactive MOC ends with QAEntry element

- **WHEN** `assemble_moc(..., mode="interactive", ...)` is called
- **THEN** the written file MUST contain exactly one `<QAEntry` opening tag
- **AND** the QAEntry MUST appear AFTER the `🎯 下車（完成）` heading

#### Scenario: Plain MOC replaces QAEntry with plain sentence

- **WHEN** `assemble_moc(..., mode="plain", ...)` is called
- **THEN** the written file MUST NOT contain any `<QAEntry` tag
- **AND** the file MUST contain the literal sentence `本專案有 Q&A 功能可對話式繼續學習。`


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: route.json output carries D-029 §八 schema with station_id and file_path

The sidecar SHALL implement `write_route_json(*, task: str, source_type: Literal["folder", "topic"], source_path: str, generated_at: datetime, stations: list[RouteStation], output_path: Path) -> None` in `codebus_agent.generator.route`. The function MUST write a JSON file to `<workspace_root>/codebus-tutorials/{task_id}/route.json`.

Each `RouteStation` entry in `stations` MUST contain:

- `station_id: str` (stable id from the stable-id Requirement)
- `index: int` (1-based station index)
- `title: str`
- `duration: int` (minutes)
- `file_path: str` (relative path from `route.json` to the station file, always `stations/{station_id}.md`)
- `prerequisites: list[str]` (always `[]` in P0; populated by future `depends-on-backfill` change)
- `related_files: list[str]` (paths within `workspace_root` from station frontmatter)
- `related_stations: list[str]` (stable ids from station frontmatter)
- `required_checks: list[str]` (parsed from station markdown's checkpoint/quiz ids)
- `degraded: bool` (true if the corresponding station file's frontmatter has `degraded: true`)

The top-level JSON object MUST contain `title`, `task`, `source_type`, `source_path`, `estimated_minutes` (sum of station durations), `generated_at`, and `stations`. If every station has `degraded: true`, the top-level object MUST also include `degraded: true`.

#### Scenario: Clean run emits route.json with all stations and no top-level degraded

- **WHEN** `write_route_json(...)` is called with three `RouteStation` entries all having `degraded=False`
- **THEN** the written JSON MUST parse and the top-level object MUST contain exactly `title`, `task`, `source_type`, `source_path`, `estimated_minutes`, `generated_at`, `stations` keys (no `degraded` key)
- **AND** `stations` MUST be a list of exactly three entries
- **AND** every entry MUST contain the keys `station_id`, `index`, `title`, `duration`, `file_path`, `prerequisites`, `related_files`, `related_stations`, `required_checks`, `degraded`

#### Scenario: All-degraded run sets top-level degraded flag

- **WHEN** `write_route_json(...)` is called with all `RouteStation` entries having `degraded=True`
- **THEN** the top-level JSON object MUST contain `"degraded": true`

#### Scenario: file_path uses stations/ relative path with stable id

- **WHEN** any station entry is written
- **THEN** its `file_path` field MUST equal `f"stations/{station_id}.md"` (no leading `./`, no other path prefix)


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: Generator output passes Sanitizer Pass 1 before disk write

Before writing any station file or `tutorial.md` to disk, the sidecar SHALL invoke `SanitizerEngine.sanitize(content, source=FileSource(path=<output_path>))` on the rendered content (frontmatter included for station files; full MOC content for `tutorial.md`). The sanitized text returned by the engine MUST be the value written to disk (not the pre-sanitize text).

Each Pass 1 hit MUST append one entry to `<workspace_root>/.codebus/sanitize_audit.jsonl` with `pass_num=1` and `source.path` set to the output file path under `codebus-tutorials/{task_id}/`. The audit writer is the existing `SanitizerAuditLogger`; no new audit log layer is introduced.

This requirement MUST hold even when the LLM input was already Pass 1 + Pass 2 sanitized — defense in depth covers the case where the LLM creatively synthesizes secret-like patterns or echoes content not covered by source-side scanning.

#### Scenario: Station file content with PII pattern triggers Pass 1 hit

- **WHEN** a station's LLM output contains the string `Contact alice@example.com for the API`
- **THEN** the disk-written station file MUST contain `<REDACTED:email#0>` (or the equivalent placeholder per the sanitizer's rules) instead of the literal email
- **AND** `<workspace_root>/.codebus/sanitize_audit.jsonl` MUST receive at least one entry with `pass_num=1` and `source.path` matching the station's output path under `codebus-tutorials/{task_id}/stations/`

#### Scenario: Clean LLM output writes verbatim with no audit entries

- **WHEN** a station's LLM output contains no patterns matched by the sanitizer rules
- **THEN** the disk-written station file MUST contain the LLM output verbatim
- **AND** no new entry MUST be appended to `sanitize_audit.jsonl` for this station file's write


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: Degraded fallback writes per-station stub after retry exhaustion

When `_generate_station(station, idx, context)` exhausts its 3-retry budget (the validator returned issues on every attempt), the sidecar SHALL produce a degraded stub markdown for that station. The stub MUST contain:

1. The station's H1 heading (using the station title)
2. A single paragraph stating the station could not be generated and inviting the user to re-run
3. Exactly one `<Checkpoint id="station-{idx}-check">` element with one item: `本站需要重新生成`
4. NO `<Quiz>`, `<CodeRef>`, `<Reveal>` elements

The frontmatter for a degraded stub MUST set `degraded: true` and MUST still satisfy the Frontmatter renderer's required-field schema. `required_checks` MUST contain the single checkpoint id.

The stub MUST be written to the same `<workspace_root>/codebus-tutorials/{task_id}/stations/s{NN}-{slug}.md` path as a non-degraded station would have been written. Subsequent stations in the iteration MUST NOT be affected (per-station isolation, D-029 §十六.2).

The degraded event MUST be appended to `<workspace_root>/.codebus/generator_log.jsonl` with at least the keys `timestamp`, `station_id`, `station_index`, `attempts` (always 3), `last_issues` (the validator issues from the final attempt).

If the disk write itself fails (e.g., `OSError` for disk full), the sidecar MUST log an error to the standard logger and append a `generator_log.jsonl` entry with `event="write_failed"` and continue to the next station — the failed station's `route.json` entry MUST set `degraded=true` and `error="write_failed"`. The sidecar MUST NOT retry the disk write.

#### Scenario: Three retries with persistent issues produces degraded stub

- **WHEN** `_generate_station(station, idx=2, context)` runs against a scripted MockProvider whose first three responses each fail validation (e.g., missing checkpoint)
- **THEN** the fourth attempt MUST NOT occur (retry budget exhausted)
- **AND** the file `<workspace_root>/codebus-tutorials/{task_id}/stations/s02-{slug}.md` MUST exist
- **AND** the file's frontmatter MUST contain `degraded: true`
- **AND** the file's body MUST contain exactly one `<Checkpoint>` element and zero `<Quiz>` elements

#### Scenario: Per-station degradation does not affect subsequent stations

- **WHEN** the second of three stations enters degraded fallback while stations 1 and 3 succeed
- **THEN** stations 1 and 3 MUST be written as full non-degraded markdown
- **AND** the `route.json` MUST list all three stations with `degraded=false` for stations 1 and 3 and `degraded=true` for station 2

#### Scenario: Disk write failure does not retry indefinitely

- **WHEN** disk write for a station file raises `OSError`
- **THEN** the generator MUST NOT call the write more than once for that station
- **AND** `generator_log.jsonl` MUST contain an entry with `event="write_failed"` for that station
- **AND** the corresponding `route.json` entry MUST contain `degraded=true` and `error="write_failed"`
- **AND** subsequent stations MUST still be processed


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: Plain mode prompt template emits markdown without custom components

When `options.mode == "plain"`, the sidecar SHALL use a separate prompt template (`STATION_SYSTEM_PLAIN`) that instructs the LLM to produce GitHub-renderable Markdown only — no `<Checkpoint>`, `<Quiz>`, `<CodeRef>`, `<Reveal>`, or `<QAEntry>` elements.

In plain mode:

- Where interactive mode would emit `<Checkpoint>`, plain mode MUST emit a Markdown task list (`- [ ] ...`)
- Where interactive mode would emit `<Quiz>`, plain mode MUST emit a `> 思考題：...` blockquote with the answer noted at the section end
- The `###` page-break separator (when station body exceeds 300 chars) MUST be preserved in both modes (it renders fine on GitHub)
- The validator MUST NOT emit `missing_checkpoint`, `too_many_quizzes`, or `quiz_*` issues in plain mode (separate Requirement)

The MOC's `<QAEntry>` element MUST also be replaced with the literal plain-text sentence in plain mode (separate Requirement on MOC assembler).

#### Scenario: Plain mode output contains no custom component tags

- **WHEN** `_generate_station(station, idx, context)` runs with `options.mode="plain"`
- **THEN** the resulting station file MUST NOT contain any of the literal substrings `<Checkpoint`, `<Quiz`, `<CodeRef`, `<Reveal`, or `<QAEntry`
- **AND** the file MUST contain at least one Markdown task list line matching `- [ ] `

#### Scenario: Validator skips component-specific issues in plain mode

- **WHEN** `validate_station_markdown(md=<plain-mode body without any Checkpoint>, station_idx=2, mode="plain", workspace_root=<path>)` is called
- **THEN** the returned `issues` list MUST NOT contain `"missing_checkpoint"`


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: Output root directory is workspace/codebus-tutorials per task

The sidecar SHALL write all Generator output under `<workspace_root>/codebus-tutorials/{task_id}/`. The directory MUST be created (with `mkdir(parents=True, exist_ok=True)`) before the first write. The literal string `codebus-tutorials` MUST be a module-level constant in `codebus_agent.generator.runner` to avoid magic-string drift. The `{task_id}` is the task identifier issued by `POST /generate` (matching the `^generate_[0-9a-f]{8}$` pattern from `sidecar-runtime` capability).

Generator output MUST NOT be written to `<workspace_root>/.codebus/` (which is reserved for audit JSONL chain) or to `<workspace_root>/tutorials/` (too generic, risks colliding with user-existing folders) or to any path outside `workspace_root`. The `generator_log.jsonl` operational log is the exception — it lives at `<workspace_root>/.codebus/generator_log.jsonl` since it is per-Module operational audit, not user-facing product (parallel to `reasoning_log.jsonl`).

#### Scenario: First write creates codebus-tutorials directory tree

- **WHEN** `run_generator(...)` is invoked against a workspace where `<workspace_root>/codebus-tutorials/` does not exist
- **THEN** by the time the first station file is written, the directory `<workspace_root>/codebus-tutorials/{task_id}/stations/` MUST exist
- **AND** all subsequent files (tutorial.md, route.json, station files) MUST be written under `<workspace_root>/codebus-tutorials/{task_id}/`

#### Scenario: Generator does not write to .codebus subdirectory except generator_log.jsonl

- **WHEN** `run_generator(...)` completes a 3-station run
- **THEN** no station file, tutorial.md, or route.json file MUST exist under `<workspace_root>/.codebus/`
- **AND** `<workspace_root>/.codebus/generator_log.jsonl` MUST exist (the operational log exception)


<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->

---
### Requirement: SSE generating events stream per-station progress

When `run_generator(...)` is invoked through the `POST /generate` endpoint with an attached `TaskHandleEmitter`, the sidecar SHALL emit `progress` events with `phase="generating"` for every per-station lifecycle transition.

The event schema MUST include:

- `type: "progress"`
- `phase: "generating"` (or `"assembling_moc"` for the final MOC + route.json write phase)
- `current_station: int` (1-based, 0 during MOC phase)
- `total_stations: int` (snapshot at run start, never changes mid-run)
- `status: Literal["generating", "validating", "retry", "writing_file", "assembling_moc"]`
- `station_id: str` (the stable id; null/empty during `assembling_moc` phase)
- `file_path: str` (relative path under `codebus-tutorials/{task_id}/`; only populated for `status="writing_file"` and `status="assembling_moc"`)

For each station the emit sequence MUST be: `generating` → `validating` → (zero or more `retry` if validation fails) → `writing_file`. The `assembling_moc` phase MUST emit exactly twice (once for `tutorial.md` writing, once for `route.json` writing) at the end of the run, after all stations are written.

When no emitter is wired (the optional default), no SSE events MUST be emitted and the loop behavior MUST be identical (same as the `agent-sse-wiring` legacy-emitter contract).

#### Scenario: Three-station run emits all phases for each station plus assembling_moc twice

- **WHEN** `run_generator(...)` runs with three scripted stations through `POST /generate` (emitter wired) and all stations succeed first attempt
- **THEN** the captured event list MUST contain at least: 3 events with `status="generating"`, 3 events with `status="validating"`, 3 events with `status="writing_file"`, and 2 events with `phase="assembling_moc"` and `status="writing_file"` (one for tutorial.md, one for route.json)
- **AND** the events for each station MUST appear in order `generating → validating → writing_file`
- **AND** every event MUST carry `total_stations=3`

#### Scenario: Retry attempt emits retry status

- **WHEN** the second station's first attempt fails validation and the second attempt succeeds
- **THEN** the captured event list MUST contain a `progress` event with `status="retry"`, `current_station=2`, `attempt=2`

#### Scenario: Missing emitter suppresses all generating progress events

- **WHEN** `run_generator(...)` is called without an emitter (in-process test)
- **THEN** no SSE side effects MUST occur
- **AND** the `GeneratorResult` returned MUST be structurally identical to the emitter-wired case

<!-- @trace
source: module-5-generator-p0
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/generator/stable_id.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/_audit_paths.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/generator/log.py
  - sidecar/src/codebus_agent/generator/station.py
  - sidecar/src/codebus_agent/generator/prompts/__init__.py
  - sidecar/src/codebus_agent/generator/moc.py
  - sidecar/src/codebus_agent/_audit_paths.py
  - sidecar/src/codebus_agent/generator/frontmatter.py
  - docs/reviews/2026-04-25-stage-4.md
  - sidecar/src/codebus_agent/generator/route.py
  - sidecar/src/codebus_agent/generator/validator.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/api/generate.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/generator/__init__.py
  - docs/module-5-generator.md
  - docs/implementation-plan.md
  - sidecar/src/codebus_agent/generator/types.py
  - sidecar/src/codebus_agent/generator/runner.py
tests:
  - sidecar/tests/generator/test_output_dir.py
  - sidecar/tests/generator/test_frontmatter.py
  - sidecar/tests/generator/test_log.py
  - sidecar/tests/generator/prompts/test_prompts.py
  - sidecar/tests/generator/test_station.py
  - sidecar/tests/generator/test_stable_id.py
  - sidecar/tests/generator/test_types.py
  - sidecar/tests/generator/test_validator.py
  - sidecar/tests/generator/__init__.py
  - sidecar/tests/generator/prompts/__init__.py
  - sidecar/tests/api/test_generate_endpoint.py
  - sidecar/tests/generator/test_moc.py
  - sidecar/tests/generator/test_runner.py
  - sidecar/tests/generator/test_route.py
  - sidecar/tests/generator/test_sanitize_output.py
  - sidecar/tests/generator/test_sse_events.py
  - sidecar/tests/generator/conftest.py
-->
