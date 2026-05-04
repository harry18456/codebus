## ADDED Requirements

### Requirement: Run query flow on --query invocation

When invoked with `--repo <path> --query "<text>"`, the system SHALL run a read-only flow that lets the agent read existing wiki pages and produce an answer with citations, without writing any files or modifying the vault.

#### Scenario: Query with non-empty wiki succeeds

- **WHEN** the user runs `codebus --repo X --query "how does checkout work?"` and `.codebus/wiki/pages/` contains at least one `.md` file
- **THEN** the system spawns the LLM agent in query mode and streams the agent's reasoning and answer to the terminal

### Requirement: Reject query when wiki is empty

The system SHALL fail fast with a user-facing error pointing to `--goal` when `.codebus/wiki/pages/` is missing or contains no `.md` files.

#### Scenario: Missing pages directory aborts with hint

- **WHEN** the user runs `codebus --repo X --query "..."` and `.codebus/wiki/pages/` does not exist
- **THEN** the system throws an error whose message instructs the user to run `--goal` first

#### Scenario: Empty pages directory aborts with hint

- **WHEN** `.codebus/wiki/pages/` exists but contains no `.md` files
- **THEN** the system throws an error whose message instructs the user to run `--goal` first

### Requirement: Spawn agent in query mode with Write/Edit hard-disabled

The system SHALL spawn the LLM provider with cwd = `.codebus/` (same isolation as ingest) and SHALL extend the disallowedTools list to include `Write` and `Edit` so the agent cannot write files even within the vault.

#### Scenario: Required argv flags are present in query mode

- **WHEN** the system builds argv for query mode
- **THEN** argv contains `-p`, `--output-format stream-json`, `--input-format stream-json`, `--verbose`, `--permission-mode acceptEdits`, and `--disallowedTools` whose value lists `Bash,WebFetch,WebSearch,Write,Edit`

#### Scenario: Provider spawn cwd matches vault root

- **WHEN** the system invokes the LLM provider for query mode against repo X
- **THEN** the spawn cwd equals `<X>/.codebus/`

### Requirement: Compose system prompt from schema and wiki index

The system SHALL build the agent's system prompt by concatenating the built-in schema, the current `wiki/index.md` content (or `(empty)` placeholder), and a query-mode instruction directing the agent to cite via `[[wikilink]]` and not write any files.

#### Scenario: System prompt includes schema and index

- **WHEN** query runs and `.codebus/CLAUDE.md` and `.codebus/wiki/index.md` both exist
- **THEN** the agent's system prompt contains the schema content followed by the index content followed by the query-mode instruction

#### Scenario: Missing index falls back to placeholder

- **WHEN** `.codebus/wiki/index.md` does not exist
- **THEN** the system uses `(empty)` as the index portion of the prompt

### Requirement: Query flow does not mutate the vault

The system SHALL NOT sync raw, append to `goals.jsonl`, run stale-detect, or commit to the nested git repo during query execution.

#### Scenario: Query leaves goals.jsonl unchanged

- **WHEN** query runs
- **THEN** `.codebus/goals.jsonl` content and modification time are unchanged

#### Scenario: Query leaves nested git unchanged

- **WHEN** query runs
- **THEN** `.codebus/.git` HEAD commit is unchanged
