## MODIFIED Requirements

### Requirement: LLMProvider protocol

The sidecar SHALL define an `LLMProvider` Protocol exposing exactly one method, `chat(messages, response_model)`, per `docs/decisions.md` D-012, D-033, and `docs/llm-provider.md`. The Protocol MUST NOT declare any `embed` method — the embedding call shape lives in the separate `EmbeddingProvider` Protocol introduced by this change. This narrowing is BREAKING with respect to the M1-era union Protocol; callers that previously typed parameters as `LLMProvider` to accept either chat-shaped or embed-shaped providers MUST migrate to either `LLMProvider` (chat-only) or `EmbeddingProvider` (embed-only) per their actual usage.

#### Scenario: Protocol declares only chat

- **WHEN** `LLMProvider` is imported from `codebus_agent.providers`
- **THEN** it MUST declare exactly one method `chat(messages, response_model)` returning a validated `BaseModel` instance
- **AND** it MUST NOT declare a method named `embed`

#### Scenario: Protocol is runtime checkable over narrowed interface

- **WHEN** a concrete class implementing only `chat(...)` is checked with `isinstance(instance, LLMProvider)`
- **THEN** the check MUST return `True` even if the class lacks an `embed` method (structural subtyping over the narrowed interface)

#### Scenario: Existing chat-only implementations satisfy narrowed protocol

- **WHEN** `OpenAIChatProvider` (which has only `chat`) is checked against the narrowed `LLMProvider` Protocol
- **THEN** static type analysis MUST accept it as an `LLMProvider` subtype
- **AND** the existing `OpenAIChatProvider` implementation MUST require no signature change to satisfy this Requirement

---

### Requirement: Outbound LLM traffic gated by TrackedProvider whitelist

The sidecar SHALL allow outbound network requests to external LLM providers ONLY through providers that are explicitly listed in `TrackedProvider.ALLOWED_INNER_TYPES` and registered through a `TrackedProvider` wrapper. Any direct construction or use of a non-whitelisted live provider class MUST be rejected at construction time. `ALLOWED_INNER_TYPES` SHALL gate the chat / embed call lane (LLM mode); the PII detection call lane (PII mode) is gated separately by `TrackedProvider.PII_ALLOWED_INNER_TYPES` introduced by the `pii-provider` capability — the two allowlists SHALL be disjoint sets, and any inner class MUST belong to exactly one (or be rejected entirely).

This Requirement REPLACES the M1-era `No outbound LLM traffic during M1` Requirement (now removed), reflecting M2 reality where specific roles have lawful outbound paths.

#### Scenario: ALLOWED_INNER_TYPES enforces explicit allowlist

- **WHEN** code attempts `TrackedProvider(SomeUnknownProvider(), ...)` where `SomeUnknownProvider` is in neither `ALLOWED_INNER_TYPES` nor `PII_ALLOWED_INNER_TYPES`
- **THEN** construction MUST raise `TypeError` naming the disallowed inner class
- **AND** the error message MUST distinguish "not an LLM/Embedding inner" from "not a PII inner" so developers know which allowlist to extend

#### Scenario: Allowed LLM/Embedding inner types are explicitly enumerated

- **WHEN** `TrackedProvider.ALLOWED_INNER_TYPES` is inspected
- **THEN** it MUST be exactly `{MockProvider, OpenAIEmbeddingProvider, OpenAIChatProvider}` after this change lands (unchanged from the chat-provider-wiring change)
- **AND** future live LLM / Embedding providers (e.g., Ollama, Anthropic) MUST be added by an explicit change that updates this spec

#### Scenario: ALLOWED_INNER_TYPES and PII_ALLOWED_INNER_TYPES are disjoint

- **WHEN** the intersection `TrackedProvider.ALLOWED_INNER_TYPES & TrackedProvider.PII_ALLOWED_INNER_TYPES` is computed
- **THEN** the intersection MUST be empty
- **AND** an integration test MUST assert this disjointness so future changes cannot accidentally cross-register a class into both lanes

#### Scenario: Non-whitelisted outbound paths rejected by registry

- **WHEN** code attempts `ProviderRegistry({role: raw_openai_chat_instance})` without TrackedProvider wrapping
- **THEN** the registry MUST raise `ProviderRegistryError` requiring TrackedProvider wrapping (existing `Registry enforces TrackedProvider wrapping per role` Requirement)

---

### Requirement: TrackedProvider applies Sanitizer Pass 2 before dispatch

When operating in LLM mode (inner instance whose concrete type is in `ALLOWED_INNER_TYPES`), the `TrackedProvider` SHALL invoke `SanitizerEngine.sanitize` on every outbound `chat` and `embed` payload before delegating to the wrapped provider. The sanitized payload SHALL be what the wrapped provider receives and SHALL be what `LLMCallLogger` records to `llm_calls.jsonl`, per `docs/decisions.md` D-015 and D-022.

When operating in PII mode (inner instance whose concrete type is in `PII_ALLOWED_INNER_TYPES`), Pass 2 SHALL NOT be applied — see the `pii-provider` capability's `TrackedProvider auto-bypasses Pass 2 for PII inner` Requirement for the bypass contract. This is the sole legitimate exception to D-015 "every Provider input passes through Sanitizer Pass 2"; the exception is determined by inner-type membership, not by any external flag.

#### Scenario: Chat payload sanitized before wrapped provider sees it (LLM mode)

- **WHEN** `TrackedProvider(OpenAIChatProvider(...), role=ProviderRole.CHAT, sanitizer=engine, ...)` is constructed
- **AND** `chat(messages=[{"role": "user", "content": "alice@example.com"}], response_model=...)` is called
- **THEN** the wrapped provider's `chat` MUST receive a messages list whose user message `content` equals `"<REDACTED:email#1>"`
- **AND** the `llm_calls.jsonl` line for this call MUST record the same sanitized content, not the original email

#### Scenario: Embed texts sanitized before wrapped provider sees them (LLM mode)

- **WHEN** `TrackedProvider(OpenAIEmbeddingProvider(...), role=ProviderRole.EMBED, sanitizer=engine, ...).embed(texts=["contact 0912-345-678"])` is called
- **THEN** the wrapped provider's `embed` MUST be invoked with texts where `0912-345-678` has been replaced by `<REDACTED:phone#<N>>`

#### Scenario: sanitizer_pass2_applied field reflects mode

- **WHEN** any LLM-mode `TrackedProvider` call completes (successful or raising)
- **THEN** the corresponding `llm_calls.jsonl` line MUST contain `"sanitizer_pass2_applied": true`
- **AND** when a PII-mode TrackedProvider's inner producer writes an `llm_calls.jsonl` line (only future LLM-based PII providers do this; this change ships only `RuleBasedPIIProvider` / `MockPIIProvider` which do not), the line MUST contain `"sanitizer_pass2_applied": false` per the `usage-tracking` capability's `AuditRole enumerates legal role values` Requirement
- **AND** the field type MUST remain boolean (no breaking change from M1 schema)

#### Scenario: Sanitizer failure aborts dispatch (LLM mode)

- **WHEN** the injected `SanitizerEngine.sanitize` raises `SanitizerError` during an LLM-mode `chat` call
- **THEN** the wrapped provider's `chat` MUST NOT be invoked
- **AND** no entry MUST be written to `llm_calls.jsonl` for this call
- **AND** the `TrackedProvider.chat` call MUST propagate the `SanitizerError` to its caller

## ADDED Requirements

### Requirement: EmbeddingProvider protocol

The sidecar SHALL define an `EmbeddingProvider` Protocol exposing exactly one method, `async def embed(texts: list[str]) -> EmbedResponse`, per `docs/decisions.md` D-033 and `docs/llm-provider.md`. This Protocol replaces the M1-era union `LLMProvider.embed` shape. The Protocol MUST NOT declare any `chat` method — chat-shaped calls live in the narrowed `LLMProvider` Protocol.

`EmbedResponse` SHALL retain its existing shape (`vectors: list[list[float]]`, `usage: Usage`) — no fields are added or removed by this change.

The Protocol SHALL be `@runtime_checkable` so structural subtyping checks via `isinstance(x, EmbeddingProvider)` succeed for any class implementing the `embed` method, regardless of class hierarchy.

#### Scenario: Protocol declares only embed

- **WHEN** `EmbeddingProvider` is imported from `codebus_agent.providers`
- **THEN** it MUST declare exactly one method `embed(texts: list[str]) -> EmbedResponse`
- **AND** the method MUST be declared `async`
- **AND** the Protocol MUST NOT declare a method named `chat`

#### Scenario: Protocol is runtime checkable

- **WHEN** a concrete class implementing only `async def embed(...)` is checked with `isinstance(instance, EmbeddingProvider)`
- **THEN** the check MUST return `True`

#### Scenario: Existing embed-only implementation satisfies new protocol

- **WHEN** `OpenAIEmbeddingProvider` is checked against `EmbeddingProvider`
- **THEN** static type analysis MUST accept it as an `EmbeddingProvider` subtype
- **AND** the existing `OpenAIEmbeddingProvider` implementation MUST require no signature change to satisfy this Requirement

#### Scenario: MockProvider satisfies both LLMProvider and EmbeddingProvider

- **WHEN** `MockProvider()` (which implements both `chat` and `embed` per its M1 contract) is checked against both narrowed Protocols
- **THEN** `isinstance(MockProvider(), LLMProvider)` MUST return `True`
- **AND** `isinstance(MockProvider(), EmbeddingProvider)` MUST return `True`
- **AND** the existing `MockProvider` implementation MUST NOT be split into separate Mock classes — Python structural subtyping permits one class to satisfy multiple narrow Protocols simultaneously
