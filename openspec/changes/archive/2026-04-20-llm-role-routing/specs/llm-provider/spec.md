## ADDED Requirements

### Requirement: ProviderRole enumerates call-site categories

The sidecar SHALL define a `ProviderRole` string enum with exactly four members: `REASONING`, `JUDGE`, `CHAT`, `EMBED`, per `docs/decisions.md` D-003 and the llm-role-routing change proposal.

#### Scenario: ProviderRole exposes four members

- **WHEN** `ProviderRole` is imported from `codebus_agent.providers`
- **THEN** it MUST expose exactly four members named `REASONING`, `JUDGE`, `CHAT`, and `EMBED`
- **AND** each member MUST have a lowercase string value matching its name (e.g., `ProviderRole.REASONING.value == "reasoning"`)

#### Scenario: ProviderRole is a StrEnum

- **WHEN** a `ProviderRole` member is compared to its string value
- **THEN** the comparison MUST return `True` (e.g., `ProviderRole.JUDGE == "judge"`)

### Requirement: RoleConfig binds provider, model, and default parameters per role

The sidecar SHALL define a `RoleConfig` dataclass that binds a `ProviderRole` to a `provider_id`, `model`, and default call parameters (`temperature`, `max_tokens`).

#### Scenario: RoleConfig exposes required fields

- **WHEN** `RoleConfig(provider_id="mock", model="mock-judge")` is constructed
- **THEN** the resulting instance MUST expose `provider_id: str`, `model: str`, `temperature: float`, and `max_tokens: int | None` attributes
- **AND** `temperature` MUST default to `0.2` when not provided
- **AND** `max_tokens` MUST default to `None` when not provided

#### Scenario: RoleConfig is frozen

- **WHEN** a caller attempts to mutate any field of a constructed `RoleConfig`
- **THEN** the assignment MUST raise `dataclasses.FrozenInstanceError`

### Requirement: Registry dispatches provider by role

The `ProviderRegistry` SHALL accept a `dict[ProviderRole, LLMProvider]` at construction and SHALL expose a `get(role: ProviderRole) -> LLMProvider` method that returns the provider registered for the given role.

#### Scenario: Registry returns role-specific provider

- **WHEN** a registry is constructed with distinct providers for `REASONING` and `JUDGE`
- **AND** `registry.get(ProviderRole.JUDGE)` is called
- **THEN** it MUST return the provider registered under `JUDGE`, not the one under `REASONING`

#### Scenario: Registry raises on missing role

- **WHEN** a registry is constructed without a provider for `ProviderRole.EMBED`
- **AND** `registry.get(ProviderRole.EMBED)` is called
- **THEN** the call MUST raise a `KeyError` or a subclass of it, naming the missing role

### Requirement: Registry enforces TrackedProvider wrapping per role

The `ProviderRegistry` SHALL verify at instantiation time that every provider registered for every role is wrapped by `TrackedProvider`, and SHALL raise a `ValueError` naming the offending role if any provider is not wrapped. This extends the M1 invariant in `usage-tracking` spec to the role dimension.

#### Scenario: Unwrapped provider in any role raises

- **WHEN** a registry is constructed with a raw `MockProvider()` assigned to any role
- **THEN** the `__init__` call MUST raise `ValueError` whose message names both the role and the unwrapped provider class

#### Scenario: Wrapped providers in every role succeed

- **WHEN** a registry is constructed where every role's provider is `TrackedProvider(MockProvider(), role=<matching_role>)`
- **THEN** the `__init__` call MUST succeed without raising

### Requirement: TrackedProvider records role in audit log

The `TrackedProvider` SHALL accept a `role: ProviderRole` argument at construction, and SHALL include the role's string value in every record written to `llm_calls.jsonl` by `LLMCallLogger`.

#### Scenario: Audit record contains role field

- **WHEN** `TrackedProvider(MockProvider(), role=ProviderRole.JUDGE)` performs a `chat` call
- **THEN** the resulting entry in `llm_calls.jsonl` MUST contain a `"role": "judge"` field

#### Scenario: Role field is additive to existing audit schema

- **WHEN** an existing consumer parses `llm_calls.jsonl` without awareness of the `role` field
- **THEN** all fields from the M1 audit schema (`timestamp`, `provider_id`, `model`, `sanitizer_pass2_applied`, `prompt_tokens`, `completion_tokens`) MUST still be present and MUST retain their M1 types

### Requirement: Config schema declares llm.roles map

The sidecar config SHALL accept a `llm.roles` object mapping each `ProviderRole` value (as lowercase string key) to a `RoleConfig` payload. This schema replaces the M1-era flat `llm.chat_provider` / `llm.embed_provider` fields.

#### Scenario: Config roles map parses into RoleConfig instances

- **WHEN** a config dict `{"llm": {"roles": {"judge": {"provider_id": "mock", "model": "mock-judge"}}}}` is loaded
- **THEN** the parsed representation MUST contain a `RoleConfig(provider_id="mock", model="mock-judge", temperature=0.2, max_tokens=None)` entry keyed by `ProviderRole.JUDGE`

#### Scenario: Config rejects unknown role key

- **WHEN** a config dict contains `"llm": {"roles": {"unknown_role": {...}}}`
- **THEN** parsing MUST raise a validation error naming `unknown_role` and listing the four valid role names

### Requirement: MockProvider records role for audit reachability

The `MockProvider` SHALL accept a `role: ProviderRole | None = None` argument at construction so that tests and audit records can attribute a given mock invocation to a specific role without class proliferation.

#### Scenario: Mock provider exposes role

- **WHEN** `MockProvider(role=ProviderRole.REASONING)` is constructed
- **THEN** the instance's `role` attribute MUST equal `ProviderRole.REASONING`

#### Scenario: Mock without role remains backward compatible

- **WHEN** `MockProvider()` is constructed without passing `role`
- **THEN** the instance's `role` attribute MUST equal `None` and all existing M1 scenarios for `MockProvider` (Mock chat satisfies response_model, Mock script controls output, Mock embed returns deterministic vector) MUST still hold

## MODIFIED Requirements

### Requirement: No outbound LLM traffic during M1

While this change is active, the sidecar SHALL NOT send any network request to an external LLM provider. This invariant extends to every `ProviderRole`: any role's provider MUST be `MockProvider` (optionally wrapped by `TrackedProvider`) during M1 and this llm-role-routing change.

#### Scenario: Only MockProvider registered for every role

- **WHEN** the sidecar process starts with any registry constructed
- **THEN** for every `ProviderRole` registered, the underlying provider class MUST be `MockProvider`
- **AND** no registered provider SHALL perform outbound HTTP to OpenAI, Anthropic, Gemini, or Ollama

#### Scenario: Integration test asserts no outbound calls across roles

- **WHEN** the integration test suite runs against the role-aware registry
- **THEN** it MUST assert that no outbound HTTP request leaves the sidecar process during any test, for any role, using a network-interception fixture
