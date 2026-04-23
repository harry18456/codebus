"""OpenAI embedding provider — production implementation for Module 2 KB build.

Backs openspec/changes/kb-build-production-wiring/specs/llm-provider/spec.md
  Requirement: OpenAI embedding provider

Decisions:
- D-032: text-embedding-3-small (dim 1536); no local fallback in MVP.
- D-003: all providers go through TrackedProvider — the registry guard
  enforces this at registration time.

Key contracts:
- API key SHALL be read only from `CODEBUS_OPENAI_API_KEY`; no fallback
  to `OPENAI_API_KEY` so the sidecar's degraded-mode contract
  (sidecar-runtime / KB dependency injection hook) cannot be accidentally
  bypassed by a stray shell export.
- `embed()` returns `EmbedResponse(vectors, usage)`; `usage` has the real
  `embed_tokens` and `cost_usd` populated from the OpenAI response so
  `TrackedProvider` can persist them to `token_usage.jsonl` without
  estimation (D-021).
- Auth / rate-limit failures raise typed exceptions (`OpenAIAuthError` /
  `OpenAIRateLimitError`) that `api/tasks.py::_classify_exception` maps
  to the wire error codes `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED`.
- Retry / backoff is delegated to the underlying `openai` SDK
  (D-032 decision 6) — we do NOT stack additional retries in the KB
  pipeline to keep rate-limit debugging tractable.
"""
from __future__ import annotations

import os

import openai

from .protocol import EmbedResponse, Usage

__all__ = [
    "OpenAIEmbeddingProvider",
    "OpenAIAuthError",
    "OpenAIRateLimitError",
    "OPENAI_EMBEDDING_MODEL",
    "OPENAI_EMBEDDING_DIM",
    "OPENAI_EMBEDDING_COST_PER_1M_TOKENS",
]


# Model + dim are hard-coded per D-032: no env var override. Swapping the
# model requires a new change (with a migration plan for existing
# collections, since dim change triggers KB_DIM_MISMATCH).
OPENAI_EMBEDDING_MODEL = "text-embedding-3-small"
OPENAI_EMBEDDING_DIM = 1536

# D-032 decision 1: text-embedding-3-small is billed at $0.02 / 1M tokens
# (OpenAI pricing table 2024+). Kept as a constant so cost_usd is derived,
# not estimated. If OpenAI repricings happen, bump this constant rather than
# letting tokens/cost drift apart silently in token_usage.jsonl.
OPENAI_EMBEDDING_COST_PER_1M_TOKENS = 0.02


_ENV_VAR = "CODEBUS_OPENAI_API_KEY"


class OpenAIAuthError(Exception):
    """Raised when the OpenAI API returns 401 Unauthorized.

    Mapped to wire code `OPENAI_AUTH_FAILED` by `_classify_exception`.
    Message intentionally never contains the API key.
    """


class OpenAIRateLimitError(Exception):
    """Raised after the OpenAI SDK exhausts its retry budget on 429.

    Mapped to wire code `OPENAI_RATE_LIMITED` by `_classify_exception`.
    """


class OpenAIEmbeddingProvider:
    """Thin async wrapper around `openai.AsyncOpenAI().embeddings.create`.

    Construction fails fast if `CODEBUS_OPENAI_API_KEY` is absent so the
    sidecar's degraded-mode contract is unambiguous — the caller
    (`wire_kb_dependencies`) decides not to construct the provider when
    the env var is missing, rather than constructing a broken one.
    """

    name: str = "openai-embedding"

    def __init__(self) -> None:
        api_key = os.environ.get(_ENV_VAR)
        if not api_key:
            # D-032 decision 5: env var name is the only supported source.
            # No fallback to OPENAI_API_KEY — mentioning the right var here
            # is crucial so operators fix the right thing.
            raise RuntimeError(
                f"{_ENV_VAR} environment variable is required to construct "
                f"OpenAIEmbeddingProvider; set it before starting the sidecar, "
                f"or leave it unset to keep POST /kb/build in graceful 503 mode."
            )
        self._client = openai.AsyncOpenAI(api_key=api_key)

    async def embed(self, texts: list[str]) -> EmbedResponse:
        try:
            response = await self._client.embeddings.create(
                model=OPENAI_EMBEDDING_MODEL,
                input=list(texts),
            )
        except openai.AuthenticationError:
            # `from None` so chained traceback doesn't leak the original
            # headers (which the openai SDK may include in repr).
            raise OpenAIAuthError(
                "OpenAI authentication failed; verify CODEBUS_OPENAI_API_KEY"
            ) from None
        except openai.RateLimitError:
            raise OpenAIRateLimitError(
                "OpenAI rate limit exceeded after SDK retry budget; "
                "reduce concurrency or wait before retrying"
            ) from None

        vectors = [list(item.embedding) for item in response.data]
        total_tokens = int(getattr(response.usage, "total_tokens", 0) or 0)
        cost = total_tokens * OPENAI_EMBEDDING_COST_PER_1M_TOKENS / 1_000_000
        usage = Usage(
            call_type="embed",
            model=response.model,
            embed_tokens=total_tokens,
            cost_usd=cost,
            estimated=False,
        )
        return EmbedResponse(vectors=vectors, usage=usage)
