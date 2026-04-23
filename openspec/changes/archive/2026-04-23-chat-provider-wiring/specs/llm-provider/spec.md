## REMOVED Requirements

### Requirement: No outbound LLM traffic during M1

**Reason**: M1-era temporary invariant. `kb-build-production-wiring` (2026-04-23, D-032) lawfully introduced outbound traffic for the EMBED role with `OpenAIEmbeddingProvider`, and `chat-provider-wiring` extends the allowlist to chat-ish roles. The replacement Requirement `Outbound LLM traffic gated by TrackedProvider whitelist` (added below) preserves the safety property — every outbound call still flows through `TrackedProvider` and a closed `ALLOWED_INNER_TYPES` allowlist — while accurately reflecting M2 reality.

**Migration**: Existing tests asserting "no outbound HTTP" continue to apply for the legacy role+provider combinations they were written against (e.g., MockProvider). New live providers (`OpenAIEmbeddingProvider`, `OpenAIChatProvider`) are explicitly listed in `ALLOWED_INNER_TYPES`; tests for those providers use `respx` to mock the wire.

#### Scenario: Requirement superseded

- **WHEN** any caller references this Requirement after `chat-provider-wiring` lands
- **THEN** they SHALL be redirected to `Outbound LLM traffic gated by TrackedProvider whitelist` which replaces this Requirement's safety property under the new M2 allowlist semantics

## ADDED Requirements

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
