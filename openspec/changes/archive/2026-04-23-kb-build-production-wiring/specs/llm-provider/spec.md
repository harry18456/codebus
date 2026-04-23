## ADDED Requirements

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
