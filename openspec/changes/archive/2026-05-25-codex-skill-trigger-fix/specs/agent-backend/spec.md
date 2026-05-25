## MODIFIED Requirements

### Requirement: Agent Backend Trait Contract

The codebus core SHALL define an `AgentBackend` trait that is the sole contract between the provider-agnostic invocation loop and a concrete agent CLI. The trait SHALL declare three required methods (`build_command`, `parse_stream_line`, `extract_session_id`) and MAY declare additional optional methods whose default implementations preserve the existing three-method behavior. The currently-permitted optional method is `stdin_payload(&SpawnSpec) -> Option<String>`, with a default `None` body so backends that do not need it can continue to implement only the three required methods. The trait SHALL NOT expose tool, sandbox, MCP, model, or argv concepts to its caller — those SHALL be encapsulated entirely inside the implementing type. Any optional method SHALL be motivated by a concrete cross-backend variation (not speculative future extension) and SHALL have a safe default that preserves the prior contract.

#### Scenario: Trait exposes the required contract methods

- **WHEN** a type implements `AgentBackend`
- **THEN** it SHALL provide `build_command(&SpawnSpec) -> Command`, `parse_stream_line(&str) -> Vec<StreamEvent>`, and `extract_session_id(&str) -> Option<String>` AND the trait SHALL NOT require any method that takes tool / sandbox / model parameters

#### Scenario: Backend output is the normalized event contract

- **WHEN** `parse_stream_line` is called with a provider stdout line
- **THEN** it SHALL return `Vec<StreamEvent>` (the normalized cross-provider event type) AND SHALL NOT return any provider-specific event shape

#### Scenario: Optional stdin payload method has a safe default

- **WHEN** a backend implements only the three required methods
- **THEN** the trait's default `stdin_payload` implementation SHALL return `None`, AND the invocation loop SHALL close the child's stdin as before (no behavior change for backends that do not opt in)

#### Scenario: Backend opt-in routes a multi-line prompt to stdin

- **WHEN** a backend's `stdin_payload(spec)` returns `Some(payload)`
- **THEN** the invocation loop SHALL open the child's stdin as a pipe, write `payload` to it, and close the pipe before reading stdout — and the backend's own `build_command` SHALL have used `-` (or omitted) as the prompt argv element so the CLI reads from stdin
