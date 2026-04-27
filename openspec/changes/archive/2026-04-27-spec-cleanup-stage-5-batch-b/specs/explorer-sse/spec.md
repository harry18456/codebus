## MODIFIED Requirements

### Requirement: POST /explore endpoint spawns Explorer under task registry

The sidecar SHALL expose a `POST /explore` endpoint that accepts a JSON body with `workspace_root: str`, `task: str`, and optional `budget_steps: int` / `budget_tokens: int` fields, validates the workspace root (exists, is a directory), creates an `explore`-kind task via the existing `TaskRegistry.create("explore")` single-slot store, spawns the Explorer agent as a background coroutine under the existing `_run_background_task` wrapper, and responds `202 Accepted` with `{"task_id": "explore_<8-hex>"}`. When another task is already in flight, the endpoint MUST respond `409 Conflict` with `{"code": "TASK_IN_FLIGHT"}`.

The background coroutine SHALL construct a fully-wired Explorer — `ToolContext` bound to the validated workspace root (with `sanitizer=SanitizerEngine()` and `kb=app.state.kb`/`usage_tracker=app.state.kb_usage_tracker(ws)` when configured), `FolderTools(ctx, state)`, `LLMJudge` built via `app.state.llm_judge_provider`, `ReasoningLogger(workspace_root / ".codebus" / "reasoning_log.jsonl")`, and a `TaskHandleEmitter(handle)` — then invoke `run_explorer(...)` with the emitter. On a clean return the wrapper emits `done`; on failure the wrapper emits a sanitized `error` event (existing spec `Background task error containment` continues to apply).

The endpoint coroutine MUST `mkdir(parents=True, exist_ok=True)` the `<workspace_root>/.codebus/` subdirectory **before** instantiating `ReasoningLogger`. Per the `agent-core` Requirement `ReasoningLogger appends one JSONL line per Step to workspace path`, `ReasoningLogger` does NOT auto-mkdir its parent directory (unlike `UsageTracker` / `LLMCallLogger` / `KBGrowthLogger` which do); the caller is responsible for ensuring the `.codebus/` subdirectory exists before construction. This invariant aligns with `audit-path-unification` (archive 2026-04-25), which moved all six workspace-level audit JSONLs from `<workspace_root>/` root into `<workspace_root>/.codebus/`.

#### Scenario: Happy path returns 202 with task_id

- **WHEN** `POST /explore` is called with a valid JSON body and a workspace root that exists as a directory
- **THEN** the response status MUST be `202`
- **AND** the response JSON MUST contain a `task_id` matching `^explore_[0-9a-f]{8}$`

#### Scenario: Concurrent task rejected

- **WHEN** `POST /explore` is called while another task (`scan` / `kb` / `explore`) is still `running`
- **THEN** the response status MUST be `409`
- **AND** the response body MUST contain `{"code": "TASK_IN_FLIGHT"}`

#### Scenario: Missing workspace root rejected

- **WHEN** `POST /explore` is called with a `workspace_root` that does not exist or is not a directory
- **THEN** the response status MUST be `400` (or `404`, at implementation discretion, but MUST NOT create a task handle)
- **AND** `TaskRegistry.current_running()` MUST remain unchanged

#### Scenario: Bearer authentication enforced

- **WHEN** `POST /explore` is called without a valid bearer token
- **THEN** the response status MUST be `401`, identical to every other sidecar endpoint's behavior

#### Scenario: ReasoningLogger lands under .codebus subdirectory

- **WHEN** the endpoint coroutine constructs the `ReasoningLogger` for a successfully validated workspace root `<ws>`
- **THEN** the logger's `path` attribute MUST equal `<ws>/.codebus/reasoning_log.jsonl`
- **AND** the endpoint coroutine MUST have called `(<ws> / ".codebus").mkdir(parents=True, exist_ok=True)` before constructing the logger (caller-mkdir invariant — `ReasoningLogger` does NOT auto-mkdir)
- **AND** subsequent `Step` writes from `run_explorer` MUST land in `<ws>/.codebus/reasoning_log.jsonl`, NOT in `<ws>/reasoning_log.jsonl`
