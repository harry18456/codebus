## MODIFIED Requirements

### Requirement: ExplorerTools, Judge, and CoverageChecker are structural Protocols

The sidecar SHALL expose three `typing.Protocol` types in `codebus_agent.agent.protocols` that define the boundary between Explorer core and pluggable implementations: `ExplorerTools` (with `primary_search`, `fetch`, `follow_reference` coroutines), `Judge` (with `evaluate`), and `CoverageChecker` (with `check`). These Protocols MUST be `runtime_checkable` so tests can assert duck-typing conformance, but `run_explorer` MUST NOT perform `isinstance` checks in its hot path — type checking is enforced statically and at test boundaries.

The Protocol surface is the day-1 abstraction that unlocks future reuse: Q&A Agent (Module 8) and Topic-mode Explorer (Phase 2) supply their own implementations without touching the core loop. Therefore the P0 shape MUST NOT leak Folder-mode-specific assumptions (e.g. file paths) into the Protocol signatures — use abstract types like `SearchHit`, `Content`, `Target` defined alongside the Protocols.

`ExplorerTools` SHALL additionally declare an OPTIONAL `tool_specs() -> list[dict]` method that returns the tool-spec list consumed by `render_explorer_prompt(state, tool_specs)`. The method is OPTIONAL at the Protocol level (implementors are permitted to omit it) and `run_explorer` MUST provide a fallback empty list when absent. Concrete Folder-mode `FolderTools` (landed by `explorer-tools-p0`) SHALL implement `tool_specs()` to return one dict per exposed tool with keys `name` / `description` / `parameters` so the Explorer Think-step prompt advertises its real tool surface instead of the empty `[]` default supplied in P0.

#### Scenario: MockTools satisfies ExplorerTools structurally

- **WHEN** a test class implements `primary_search` / `fetch` / `follow_reference` with correct coroutine signatures (no `ExplorerTools` inheritance)
- **THEN** `isinstance(mock_tools, ExplorerTools)` MUST return True via `runtime_checkable`, and `run_explorer` MUST accept it as the `tools` argument without type error

#### Scenario: Protocols do not bind Folder-mode types

- **WHEN** `ExplorerTools.primary_search`'s signature is inspected
- **THEN** its parameters and return type MUST be abstract (`query: str` → `list[SearchHit]`) rather than Folder-specific types, so a `TopicTools` implementation (Phase 2) can satisfy the same Protocol without core-loop changes

#### Scenario: tool_specs method is optional on ExplorerTools

- **WHEN** a minimal `_MockTools` class implements only `primary_search` / `fetch` / `follow_reference` and omits `tool_specs`
- **THEN** `isinstance(mock_tools, ExplorerTools)` MUST still return True
- **AND** `run_explorer` MUST fall back to an empty `tool_specs=[]` for prompt rendering without raising `AttributeError`

#### Scenario: FolderTools advertises its tool surface via tool_specs

- **WHEN** a `FolderTools` instance is passed to `run_explorer`
- **AND** `tool_specs()` is invoked on that instance
- **THEN** the return value MUST be a `list[dict]` containing at least one entry for each of `search` / `list_dir` / `read_file` / `mark_station`
- **AND** each entry MUST carry `name` / `description` / `parameters` keys so the prompt render can advertise them to the LLM
