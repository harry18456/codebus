# llm-provider Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: LLMProvider protocol

The sidecar SHALL define an `LLMProvider` Protocol exposing `chat` and `embed` methods, per `docs/decisions.md` D-012 and `docs/llm-provider.md`.

#### Scenario: Protocol methods present

- **WHEN** `LLMProvider` is imported
- **THEN** it MUST declare a `chat(messages, response_model)` method and an `embed(texts)` method

#### Scenario: Protocol is checkable at type level

- **WHEN** a concrete class that implements both methods is checked against `LLMProvider`
- **THEN** static type analysis MUST accept it as an `LLMProvider` subtype


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Mock provider returns Instructor-compatible output

The sidecar SHALL ship a `MockProvider` implementation whose `chat` method produces values parsed by Instructor and Pydantic through the real code path, per design decision D-local-4.

#### Scenario: Mock chat satisfies response_model

- **WHEN** `MockProvider.chat(messages=[...], response_model=SomeBaseModel)` is called and no script is provided
- **THEN** the return value MUST be an instance of `SomeBaseModel` and MUST pass Pydantic validation

#### Scenario: Mock script controls output

- **WHEN** `MockProvider` is constructed with a `MockScript` that pins the next `chat` output to a specific payload
- **THEN** the subsequent `chat` call MUST return the pinned payload and MUST consume one script entry

#### Scenario: Mock embed returns deterministic vector

- **WHEN** `MockProvider.embed(texts=["hello"])` is called twice with the same input
- **THEN** both calls MUST return the same vector, enabling cache-key tests downstream


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: Outbound LLM traffic gated by TrackedProvider whitelist

The sidecar SHALL allow outbound network requests to external LLM providers ONLY through providers that are explicitly listed in `TrackedProvider.ALLOWED_INNER_TYPES` and registered through a `TrackedProvider` wrapper. Any direct construction or use of a non-whitelisted live provider class MUST be rejected at construction time. This Requirement REPLACES the M1-era `No outbound LLM traffic during M1` Requirement (now removed), reflecting M2 reality where specific roles have lawful outbound paths.

#### Scenario: ALLOWED_INNER_TYPES enforces explicit allowlist

- **WHEN** code attempts `TrackedProvider(SomeUnknownProvider(), ...)` where `SomeUnknownProvider` is not in `TrackedProvider.ALLOWED_INNER_TYPES`
- **THEN** construction MUST raise `TypeError` naming the disallowed inner class

#### Scenario: Allowed inner types are explicitly enumerated

- **WHEN** `TrackedProvider.ALLOWED_INNER_TYPES` is inspected
- **THEN** it MUST be exactly `{MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}` after this change lands; future live providers (e.g., Ollama, Anthropic) MUST be added by an explicit change that updates this spec

#### Scenario: Non-whitelisted outbound paths rejected by registry

- **WHEN** code attempts `ProviderRegistry({role: raw_openai_chat_instance})` without TrackedProvider wrapping
- **THEN** the registry MUST raise `ProviderRegistryError` requiring TrackedProvider wrapping (existing `Registry enforces TrackedProvider wrapping per role` Requirement)

<!-- @trace
source: chat-provider-wiring
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/src/codebus_agent/providers/openai_chat.py
  - sidecar/src/codebus_agent/providers/__init__.py
tests:
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/providers/test_openai_chat.py
-->


<!-- @trace
source: chat-provider-wiring
updated: 2026-04-23
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/src/codebus_agent/api/tasks.py
  - docs/llm-provider.md
  - sidecar/scripts/smoke_chat_provider.py
  - sidecar/src/codebus_agent/providers/__init__.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/providers/openai_chat.py
  - CLAUDE.md
tests:
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/test_wire_kb_dependencies.py
  - sidecar/tests/providers/test_openai_chat.py
-->

---
### Requirement: OpenAI chat provider

The sidecar SHALL implement an `OpenAIChatProvider` class that satisfies the `LLMProvider` Protocol's `chat(messages: list[Message], *, response_model: type[BaseModel]) -> BaseModel` method, using `instructor` to parse OpenAI chat completions into validated Pydantic instances. The provider SHALL accept `model: str`, `temperature: float = 0.2`, and `max_tokens: int | None = None` at construction, read its API key only from the `CODEBUS_OPENAI_API_KEY` environment variable (sharing the same env var as `OpenAIEmbeddingProvider`), and SHALL be wrappable in `TrackedProvider`. Authentication, rate-limit, and context-length failures SHALL translate into documented typed exceptions.

#### Scenario: Chat call returns validated Pydantic instance

- **WHEN** `OpenAIChatProvider("gpt-4o-mini").chat([Message(role="user", content="reply with {\"answer\": \"hi\"}")], response_model=AnswerModel)` succeeds against a mocked OpenAI endpoint
- **THEN** the returned object MUST be an instance of `AnswerModel` with the parsed fields populated; no raw JSON string MUST leak to the caller

#### Scenario: Provider must be registered through TrackedProvider

- **WHEN** `ProviderRegistry.__init__({ProviderRole.CHAT: OpenAIChatProvider("gpt-4o-mini")})` is attempted without a `TrackedProvider` wrapper
- **THEN** the registry MUST raise `ProviderRegistryError` identifying the unwrapped provider, consistent with the existing `Registry enforces TrackedProvider wrapping per role` Requirement

#### Scenario: Authentication failure maps to OPENAI_AUTH_FAILED

- **WHEN** the OpenAI API responds `401 Unauthorized` to a chat completion request
- **THEN** `OpenAIChatProvider.chat` MUST raise `OpenAIAuthError` (the same typed exception used by the embedding provider), and `_classify_exception` in `api/tasks.py` MUST map it to wire code `"OPENAI_AUTH_FAILED"`; the API key MUST NOT appear in the exception message, logs, or any wire payload

#### Scenario: Rate limit after retries maps to OPENAI_RATE_LIMITED

- **WHEN** the OpenAI API returns `429` responses beyond the SDK's retry budget for a chat completion
- **THEN** the provider MUST raise `OpenAIRateLimitError` and `_classify_exception` MUST map it to wire code `"OPENAI_RATE_LIMITED"`

#### Scenario: Context-length error maps to OPENAI_CONTEXT_EXCEEDED

- **WHEN** the OpenAI API responds `400 Bad Request` with `error.code == "context_length_exceeded"` (oversized prompt for the chosen model)
- **THEN** the provider MUST raise a new `OpenAIContextLengthError` exception class, and `_classify_exception` MUST map it to a new wire code `"OPENAI_CONTEXT_EXCEEDED"` added to `ERROR_CODES`. The error event MUST NOT echo the prompt content (which is potentially sensitive)

#### Scenario: Missing CODEBUS_OPENAI_API_KEY env var blocks construction

- **WHEN** `OpenAIChatProvider("gpt-4o-mini")` is constructed without `CODEBUS_OPENAI_API_KEY` set in the environment
- **THEN** construction MUST raise a clear error identifying the missing env var name, and MUST NOT fall back to reading `OPENAI_API_KEY` (so the sidecar's degraded-mode contract in `sidecar-runtime` is not accidentally bypassed)

#### Scenario: Temperature and max_tokens passed to OpenAI

- **WHEN** `OpenAIChatProvider("gpt-4o-mini", temperature=0.0, max_tokens=512).chat(...)` is called
- **THEN** the underlying `openai` SDK request MUST include `temperature=0.0` and `max_tokens=512` so per-role tuning takes effect

<!-- @trace
source: chat-provider-wiring
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/providers/openai_chat.py
  - sidecar/src/codebus_agent/providers/__init__.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/src/codebus_agent/api/tasks.py
  - sidecar/src/codebus_agent/api/__init__.py
  - docs/llm-provider.md
tests:
  - sidecar/tests/providers/test_openai_chat.py
  - sidecar/tests/providers/test_tracked_provider.py
-->


<!-- @trace
source: chat-provider-wiring
updated: 2026-04-23
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/src/codebus_agent/api/tasks.py
  - docs/llm-provider.md
  - sidecar/scripts/smoke_chat_provider.py
  - sidecar/src/codebus_agent/providers/__init__.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/providers/openai_chat.py
  - CLAUDE.md
tests:
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/test_wire_kb_dependencies.py
  - sidecar/tests/providers/test_openai_chat.py
-->

---
### Requirement: ProviderRole enumerates call-site categories

The sidecar SHALL define a `ProviderRole` string enum with exactly four members: `REASONING`, `JUDGE`, `CHAT`, `EMBED`, per `docs/decisions.md` D-003 and the llm-role-routing change proposal.

#### Scenario: ProviderRole exposes four members

- **WHEN** `ProviderRole` is imported from `codebus_agent.providers`
- **THEN** it MUST expose exactly four members named `REASONING`, `JUDGE`, `CHAT`, and `EMBED`
- **AND** each member MUST have a lowercase string value matching its name (e.g., `ProviderRole.REASONING.value == "reasoning"`)

#### Scenario: ProviderRole is a StrEnum

- **WHEN** a `ProviderRole` member is compared to its string value
- **THEN** the comparison MUST return `True` (e.g., `ProviderRole.JUDGE == "judge"`)

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/protocol.py
-->

---
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

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/protocol.py
-->

---
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

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/registry.py
-->

---
### Requirement: Registry enforces TrackedProvider wrapping per role

The `ProviderRegistry` SHALL verify at instantiation time that every provider registered for every role is wrapped by `TrackedProvider`, and SHALL raise a `ValueError` naming the offending role if any provider is not wrapped. This extends the M1 invariant in `usage-tracking` spec to the role dimension.

#### Scenario: Unwrapped provider in any role raises

- **WHEN** a registry is constructed with a raw `MockProvider()` assigned to any role
- **THEN** the `__init__` call MUST raise `ValueError` whose message names both the role and the unwrapped provider class

#### Scenario: Wrapped providers in every role succeed

- **WHEN** a registry is constructed where every role's provider is `TrackedProvider(MockProvider(), role=<matching_role>)`
- **THEN** the `__init__` call MUST succeed without raising

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/registry.py
-->

---
### Requirement: TrackedProvider records role in audit log

The `TrackedProvider` SHALL accept a `role: ProviderRole` argument at construction, and SHALL include the role's string value in every record written to `llm_calls.jsonl` by `LLMCallLogger`.

#### Scenario: Audit record contains role field

- **WHEN** `TrackedProvider(MockProvider(), role=ProviderRole.JUDGE)` performs a `chat` call
- **THEN** the resulting entry in `llm_calls.jsonl` MUST contain a `"role": "judge"` field

#### Scenario: Role field is additive to existing audit schema

- **WHEN** an existing consumer parses `llm_calls.jsonl` without awareness of the `role` field
- **THEN** all fields from the M1 audit schema (`timestamp`, `provider_id`, `model`, `sanitizer_pass2_applied`, `prompt_tokens`, `completion_tokens`) MUST still be present and MUST retain their M1 types

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/tracked.py
-->

---
### Requirement: Config schema declares llm.roles map

The sidecar config SHALL accept a `llm.roles` object mapping each `ProviderRole` value (as lowercase string key) to a `RoleConfig` payload. This schema replaces the M1-era flat `llm.chat_provider` / `llm.embed_provider` fields.

#### Scenario: Config roles map parses into RoleConfig instances

- **WHEN** a config dict `{"llm": {"roles": {"judge": {"provider_id": "mock", "model": "mock-judge"}}}}` is loaded
- **THEN** the parsed representation MUST contain a `RoleConfig(provider_id="mock", model="mock-judge", temperature=0.2, max_tokens=None)` entry keyed by `ProviderRole.JUDGE`

#### Scenario: Config rejects unknown role key

- **WHEN** a config dict contains `"llm": {"roles": {"unknown_role": {...}}}`
- **THEN** parsing MUST raise a validation error naming `unknown_role` and listing the four valid role names

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/__init__.py
-->

---
### Requirement: MockProvider records role for audit reachability

The `MockProvider` SHALL accept a `role: ProviderRole | None = None` argument at construction so that tests and audit records can attribute a given mock invocation to a specific role without class proliferation.

#### Scenario: Mock provider exposes role

- **WHEN** `MockProvider(role=ProviderRole.REASONING)` is constructed
- **THEN** the instance's `role` attribute MUST equal `ProviderRole.REASONING`

#### Scenario: Mock without role remains backward compatible

- **WHEN** `MockProvider()` is constructed without passing `role`
- **THEN** the instance's `role` attribute MUST equal `None` and all existing M1 scenarios for `MockProvider` (Mock chat satisfies response_model, Mock script controls output, Mock embed returns deterministic vector) MUST still hold

<!-- @trace
source: llm-role-routing
updated: 2026-04-20
code:
  - sidecar/src/codebus_agent/providers/mock.py
-->

---
### Requirement: TrackedProvider applies Sanitizer Pass 2 before dispatch

The `TrackedProvider` SHALL invoke `SanitizerEngine.sanitize` on every outbound `chat` and `embed` payload before delegating to the wrapped provider. The sanitized payload SHALL be what the wrapped provider receives and SHALL be what `LLMCallLogger` records to `llm_calls.jsonl`, per `docs/decisions.md` D-015 and D-022.

#### Scenario: Chat payload sanitized before wrapped provider sees it

- **WHEN** `TrackedProvider(MockProvider(), role=ProviderRole.CHAT)` is constructed with a sanitizer injected
- **AND** `chat(messages=[{"role": "user", "content": "alice@example.com"}], response_model=...)` is called
- **THEN** the wrapped `MockProvider.chat` MUST receive a messages list whose user message `content` equals `"<REDACTED:email#1>"`
- **AND** the `llm_calls.jsonl` line for this call MUST record the same sanitized content, not the original email

#### Scenario: Embed texts sanitized before wrapped provider sees them

- **WHEN** `TrackedProvider.embed(texts=["contact 0912-345-678"])` is called
- **THEN** the wrapped provider's `embed` MUST be invoked with texts where `0912-345-678` has been replaced by `<REDACTED:phone#<N>>`

#### Scenario: sanitizer_pass2_applied field set to true

- **WHEN** any `TrackedProvider` call completes (successful or raising from the wrapped provider)
- **THEN** the corresponding `llm_calls.jsonl` line MUST contain `"sanitizer_pass2_applied": true`
- **AND** the field type MUST remain boolean (no breaking change from M1 schema)

#### Scenario: Sanitizer failure aborts dispatch

- **WHEN** the injected `SanitizerEngine.sanitize` raises `SanitizerError` during `chat`
- **THEN** the wrapped provider's `chat` MUST NOT be invoked
- **AND** no entry MUST be written to `llm_calls.jsonl` for this call
- **AND** the `TrackedProvider.chat` call MUST propagate the `SanitizerError` to its caller

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/providers/tracked.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: TrackedProvider writes audit entries to sanitize_audit.jsonl

For every Pass 2 sanitize invocation performed by `TrackedProvider`, each resulting `AuditEntry` SHALL be appended to `{workspace}/.codebus/sanitize_audit.jsonl` by the injected `SanitizerAuditLogger`, with `pass` field equal to `2`.

#### Scenario: Pass 2 audit entry written

- **WHEN** `TrackedProvider.chat` sanitizes a message that contains an email
- **THEN** `sanitize_audit.jsonl` MUST have exactly one appended line with `"pass": 2`
- **AND** that line MUST include the same fields required by the `sanitizer` capability spec (`ts`, `schema_version`, `rules_version`, `session_id`, `source`, `rule_id`, `kind`, `placeholder_index`, `extra`)

#### Scenario: Source field identifies message scope

- **WHEN** a Pass 2 audit entry is written
- **THEN** its `source` field MUST start with the prefix `message:` followed by a stable identifier for the in-flight call (for example `message:chat_req_<uuid>`)

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
-->

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: OpenAI embedding provider

The sidecar SHALL implement an `OpenAIEmbeddingProvider` class that satisfies the `LLMProvider` Protocol's `embed(texts: list[str]) -> EmbedResponse` method, targets the `text-embedding-3-small` model, reads its API key only from the `CODEBUS_OPENAI_API_KEY` environment variable, and SHALL be registered into the provider registry under `ProviderRole.embedding` via a `TrackedProvider` wrapper. The provider SHALL translate authentication and rate-limit failures into documented error codes so calling pipelines can surface them without leaking secrets, per `docs/decisions.md` D-003 and D-032.

#### Scenario: Embed call returns vectors with dimension 1536

- **WHEN** `OpenAIEmbeddingProvider.embed(texts=["alpha", "beta"])` succeeds
- **THEN** the returned `EmbedResponse` MUST contain exactly two vectors, each of length 1536, and a `usage` object with non-null `input_tokens` and `cost_usd`

#### Scenario: Provider must be registered through TrackedProvider

- **WHEN** `ProviderRegistry.register_embedding(OpenAIEmbeddingProvider(...))` is attempted without a `TrackedProvider` wrapper
- **THEN** the registry MUST raise an error identifying the unwrapped provider, consistent with the existing registry guard that rejects unwrapped providers at instantiation

#### Scenario: Authentication failure maps to OPENAI_AUTH_FAILED

- **WHEN** the OpenAI API responds `401 Unauthorized` to an embed request
- **THEN** the provider MUST raise an exception subclass that `_classify_exception` in `api/tasks.py` maps to the wire error code `"OPENAI_AUTH_FAILED"`, and MUST NOT include the API key in the exception message, logs, or SSE payload

#### Scenario: Rate limit after retries maps to OPENAI_RATE_LIMITED

- **WHEN** the OpenAI API returns `429` responses beyond the provider's internal retry budget (max 3 retries, exponential backoff)
- **THEN** the provider MUST raise an exception subclass that maps to the wire error code `"OPENAI_RATE_LIMITED"` and MUST record one `token_usage.jsonl` line per actual attempt (not per logical call) so cost accounting reflects retries

#### Scenario: Missing CODEBUS_OPENAI_API_KEY env var blocks construction

- **WHEN** `OpenAIEmbeddingProvider()` is constructed without `CODEBUS_OPENAI_API_KEY` set in the environment
- **THEN** construction MUST raise a clear error identifying the missing env var name, and MUST NOT fall back to reading `OPENAI_API_KEY` or any other key name (so the sidecar's degraded-mode contract in `sidecar-runtime` is not accidentally bypassed)

<!-- @trace
source: kb-build-production-wiring
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/providers/openai_embedding.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/kb/backend.py
  - sidecar/src/codebus_agent/api/tasks.py
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/providers/tracked.py
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/api/main.py
  - docs/llm-provider.md
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/health.py
  - sidecar/uv.lock
  - docs/decisions.md
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/providers/__init__.py
  - docs/implementation-plan.md
  - CLAUDE.md
tests:
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/test_wire_kb_dependencies.py
  - sidecar/tests/kb/conftest.py
  - sidecar/tests/kb/test_dim_mismatch.py
  - sidecar/tests/providers/test_openai_embedding.py
-->

---
### Requirement: TrackedProvider tags usage records with default_module

The `TrackedProvider` SHALL accept an optional `default_module: str | None = None` argument at construction. When set, every `UsageTracker.record(...)` call made by `TrackedProvider.chat` and `TrackedProvider.embed` SHALL include `module=self._default_module`. The parameter is the SOLE mechanism by which subsystem labels (e.g., `"kb_build"`, `"qa_agent"`) reach `token_usage.jsonl` — callers MUST NOT bypass `TrackedProvider` to make their own `tracker.record()` call, ensuring "exactly one line per LLM call" per the `usage-tracking` capability.

#### Scenario: Default module reaches usage record

- **WHEN** a `TrackedProvider` is constructed with `default_module="kb_build"` and a `chat` or `embed` call succeeds
- **THEN** the corresponding `token_usage.jsonl` line MUST contain `"module": "kb_build"`

#### Scenario: Omitting default_module preserves M1 behavior

- **WHEN** a `TrackedProvider` is constructed without `default_module` (the M1 construction signature)
- **THEN** the wrapper MUST NOT raise, and the `token_usage.jsonl` line's `module` field MUST be the empty string `""` (matching M1's behavior before this Requirement landed)

#### Scenario: Failure path still records with default_module

- **WHEN** a `TrackedProvider` constructed with `default_module="kb_build"` wraps a provider whose `embed()` raises
- **THEN** the `llm_calls.jsonl` failure line MUST still be written (per existing `LLMCallLogger writes llm_calls.jsonl` Requirement), and any usage line written for the failed call MUST also carry `module="kb_build"` so accounting reflects retry costs against the right subsystem

<!-- @trace
source: usage-tracker-dedup
updated: 2026-04-23
code:
  - sidecar/src/codebus_agent/api/__init__.py
  - docs/llm-provider.md
  - CLAUDE.md
  - docs/module-2-kb-builder.md
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/providers/tracked.py
tests:
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/kb/test_knowledge_base.py
  - sidecar/tests/providers/test_default_module.py
-->
