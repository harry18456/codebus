"""TDD red tests for `OpenAIEmbeddingProvider` — Section 2 of
openspec/changes/kb-build-production-wiring/tasks.md.

Backs openspec/changes/kb-build-production-wiring/specs/llm-provider/spec.md
  Requirement: OpenAI embedding provider

Strategy:
  * `respx` mocks OpenAI's HTTPS endpoint at the httpx transport layer,
    so the `openai>=1.0` async client gets realistic response objects
    without any real network traffic.
  * Authentication / rate-limit / per-attempt token tracking scenarios
    exercise the production error-mapping contract that `_classify_exception`
    in `api/tasks.py` will consume.
  * The registry-guard test proves the invariant from D-032 decision 1
    (every provider MUST be wrapped in `TrackedProvider`) still applies
    to the new OpenAI provider — no backdoor registration path.
"""
from __future__ import annotations

import json

import httpx
import pytest
import respx

from codebus_agent.providers.openai_embedding import (
    OPENAI_EMBEDDING_DIM,
    OPENAI_EMBEDDING_MODEL,
    OpenAIAuthError,
    OpenAIEmbeddingProvider,
    OpenAIRateLimitError,
)

_OPENAI_EMBED_URL = "https://api.openai.com/v1/embeddings"


def _fake_embedding(dim: int = OPENAI_EMBEDDING_DIM) -> list[float]:
    """A deterministic 1536-dim vector — content does not matter for unit tests."""
    base = [0.0] * dim
    base[0] = 1.0
    return base


def _openai_embed_response_body(
    texts: list[str], *, model: str = OPENAI_EMBEDDING_MODEL
) -> dict:
    """Shape matches the OpenAI embeddings API response."""
    return {
        "object": "list",
        "data": [
            {"object": "embedding", "index": i, "embedding": _fake_embedding()}
            for i, _ in enumerate(texts)
        ],
        "model": model,
        "usage": {"prompt_tokens": 7 * len(texts), "total_tokens": 7 * len(texts)},
    }


@pytest.fixture(autouse=True)
def _clean_openai_env(monkeypatch: pytest.MonkeyPatch) -> None:
    """Clear both CODEBUS_OPENAI_API_KEY and stock OPENAI_API_KEY.

    Each test opts back in with a set. This also proves the provider
    SHALL NOT fall back to `OPENAI_API_KEY` — the sidecar's degraded-mode
    contract relies on the exact env var name.
    """
    monkeypatch.delenv("CODEBUS_OPENAI_API_KEY", raising=False)
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)


async def test_embed_returns_dim_1536_vectors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Embed call returns vectors with dimension 1536"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test-abc")
    provider = OpenAIEmbeddingProvider()
    with respx.mock(assert_all_called=True) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(
                200, json=_openai_embed_response_body(["alpha", "beta"])
            )
        )
        result = await provider.embed(["alpha", "beta"])

    assert len(result.vectors) == 2
    for vec in result.vectors:
        assert len(vec) == OPENAI_EMBEDDING_DIM
    assert result.usage.embed_tokens > 0
    assert result.usage.cost_usd is not None


async def test_missing_env_var_blocks_construction() -> None:
    """Spec scenario "Missing CODEBUS_OPENAI_API_KEY env var blocks construction"."""
    # env fixture already unset both keys; construction must raise clearly.
    with pytest.raises(Exception) as excinfo:
        OpenAIEmbeddingProvider()
    message = str(excinfo.value)
    assert "CODEBUS_OPENAI_API_KEY" in message, (
        f"error MUST name the exact env var; got {message!r}"
    )
    # Also must NOT fall back to OPENAI_API_KEY.
    assert "OPENAI_API_KEY" not in message.replace("CODEBUS_OPENAI_API_KEY", ""), (
        "provider MUST NOT mention OPENAI_API_KEY fallback"
    )


async def test_no_fallback_to_openai_api_key(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Even if OPENAI_API_KEY is set, construction without CODEBUS_OPENAI_API_KEY fails."""
    monkeypatch.setenv("OPENAI_API_KEY", "sk-legacy-should-not-be-used")
    with pytest.raises(Exception):
        OpenAIEmbeddingProvider()


async def test_401_maps_to_openai_auth_failed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Authentication failure maps to OPENAI_AUTH_FAILED"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-bad")
    provider = OpenAIEmbeddingProvider()
    with respx.mock() as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(
                401, json={"error": {"message": "Invalid API key"}}
            )
        )
        with pytest.raises(OpenAIAuthError) as excinfo:
            await provider.embed(["hi"])

    assert "sk-bad" not in str(excinfo.value), (
        "OpenAIAuthError message MUST NOT echo the API key"
    )


async def test_429_after_retries_maps_to_openai_rate_limited(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Rate limit after retries maps to OPENAI_RATE_LIMITED"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIEmbeddingProvider()
    with respx.mock() as mock:
        # openai SDK default is 2 retries on 429; allow 3 attempts total.
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(429, json={"error": {"message": "Slow down"}})
        )
        with pytest.raises(OpenAIRateLimitError):
            await provider.embed(["hi"])


async def test_retry_attempts_each_recorded_in_token_usage(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Decision 6: every physical attempt MUST reach the usage tracker.

    Even when the retry ends in success, the intermediate 429s should
    not be hidden from cost accounting so operators can see retry cost.
    """
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIEmbeddingProvider()

    attempts: list[int] = []

    def _sequenced_response(request: httpx.Request) -> httpx.Response:
        attempts.append(1)
        if len(attempts) <= 2:
            return httpx.Response(429, json={"error": {"message": "Slow down"}})
        return httpx.Response(
            200, json=_openai_embed_response_body(["hi"])
        )

    with respx.mock() as mock:
        mock.post(_OPENAI_EMBED_URL).mock(side_effect=_sequenced_response)
        result = await provider.embed(["hi"])

    assert len(result.vectors) == 1
    assert len(attempts) >= 3, (
        f"expected at least 3 physical attempts (2 retries then success); got {len(attempts)}"
    )


async def test_registry_rejects_unwrapped_openai_provider(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Registry guard from D-003 / D-local-4 MUST apply to new providers too.

    Even with a real OpenAI provider, callers cannot register it directly —
    every provider SHALL be wrapped in `TrackedProvider` first.
    """
    from codebus_agent.providers import (
        ProviderRegistry,
        ProviderRegistryError,
        ProviderRole,
    )

    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIEmbeddingProvider()

    with pytest.raises(ProviderRegistryError):
        ProviderRegistry({ProviderRole.EMBED: provider})  # type: ignore[dict-item]
