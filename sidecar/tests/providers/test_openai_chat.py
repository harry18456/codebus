"""TDD red tests for `OpenAIChatProvider` — Section 2 of
openspec/changes/chat-provider-wiring/tasks.md.

Backs openspec/changes/chat-provider-wiring/specs/llm-provider/spec.md
  Requirement: OpenAI chat provider

Strategy:
  * `respx` mocks OpenAI's HTTPS `/v1/chat/completions` endpoint at the
    httpx transport layer so the `openai>=1.0` async client (wrapped by
    `instructor.from_openai`) gets realistic response objects without
    any real network traffic.
  * Auth / rate-limit / context-length / temperature scenarios exercise
    the production error-mapping contract that `_classify_exception` in
    `api/tasks.py` consumes.
  * The context-length test also pins the sensitivity guarantee: the
    OpenAIContextLengthError message MUST NOT echo prompt content.
  * The registry-guard test proves `OpenAIChatProvider` cannot bypass
    the M2 wrapping invariant (every provider MUST be wrapped in
    `TrackedProvider` before registration).
"""
from __future__ import annotations

import json

import httpx
import pytest
import respx
from pydantic import BaseModel

from codebus_agent.providers.openai_chat import (
    OpenAIChatProvider,
    OpenAIContextLengthError,
)
from codebus_agent.providers.openai_embedding import (
    OpenAIAuthError,
    OpenAIRateLimitError,
)
from codebus_agent.providers.protocol import Message

_OPENAI_CHAT_URL = "https://api.openai.com/v1/chat/completions"


class _Answer(BaseModel):
    answer: str


def _tool_call_response(
    payload: dict, *, tool_name: str = "_Answer", model: str = "gpt-4o-mini"
) -> dict:
    """Shape matches OpenAI chat completions with tool_calls (Instructor TOOLS mode)."""
    return {
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1_700_000_000,
        "model": model,
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_abc",
                            "type": "function",
                            "function": {
                                "name": tool_name,
                                "arguments": json.dumps(payload),
                            },
                        }
                    ],
                },
                "finish_reason": "tool_calls",
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15,
        },
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


async def test_chat_returns_validated_pydantic_instance(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Chat call returns validated Pydantic instance"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test-abc")
    provider = OpenAIChatProvider("gpt-4o-mini")
    with respx.mock(assert_all_called=True) as mock:
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(
                200, json=_tool_call_response({"answer": "hi"})
            )
        )
        result = await provider.chat(
            [Message(role="user", content="ping")],
            response_model=_Answer,
        )

    assert isinstance(result, _Answer), (
        f"chat MUST return a response_model instance, got {type(result).__name__}"
    )
    assert result.answer == "hi", (
        f"parsed fields MUST be populated from tool_call arguments; got {result!r}"
    )


async def test_missing_env_var_blocks_construction() -> None:
    """Spec scenario "Missing CODEBUS_OPENAI_API_KEY env var blocks construction"."""
    # env fixture already unset both keys; construction must raise clearly.
    with pytest.raises(Exception) as excinfo:
        OpenAIChatProvider("gpt-4o-mini")
    message = str(excinfo.value)
    assert "CODEBUS_OPENAI_API_KEY" in message, (
        f"error MUST name the exact env var; got {message!r}"
    )


async def test_no_fallback_to_openai_api_key(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Even with OPENAI_API_KEY set, construction without CODEBUS_OPENAI_API_KEY fails.

    Prevents a stray shell `export OPENAI_API_KEY=...` from bypassing the
    sidecar's degraded-mode contract.
    """
    monkeypatch.setenv("OPENAI_API_KEY", "sk-legacy-should-not-be-used")
    with pytest.raises(Exception):
        OpenAIChatProvider("gpt-4o-mini")


async def test_401_maps_to_openai_auth_failed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Authentication failure maps to OPENAI_AUTH_FAILED"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-bad")
    provider = OpenAIChatProvider("gpt-4o-mini")
    with respx.mock() as mock:
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(
                401, json={"error": {"message": "Invalid API key"}}
            )
        )
        with pytest.raises(OpenAIAuthError) as excinfo:
            await provider.chat(
                [Message(role="user", content="hi")],
                response_model=_Answer,
            )

    assert "sk-bad" not in str(excinfo.value), (
        "OpenAIAuthError message MUST NOT echo the API key"
    )


async def test_429_after_retries_maps_to_openai_rate_limited(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Rate limit after retries maps to OPENAI_RATE_LIMITED"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIChatProvider("gpt-4o-mini")
    with respx.mock() as mock:
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(
                429, json={"error": {"message": "Slow down"}}
            )
        )
        with pytest.raises(OpenAIRateLimitError):
            await provider.chat(
                [Message(role="user", content="hi")],
                response_model=_Answer,
            )


async def test_context_length_exceeded_maps_to_openai_context_exceeded(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Context-length error maps to OPENAI_CONTEXT_EXCEEDED".

    Must also enforce the sensitivity guarantee: the error message MUST
    NOT echo prompt content (the prompt is potentially sensitive user
    code or workspace text).
    """
    from codebus_agent.api.tasks import ERROR_CODES, _classify_exception

    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIChatProvider("gpt-4o-mini")
    secret_prompt = "UNIQUE-SECRET-CODE-BLOCK-XYZ-9191"
    with respx.mock() as mock:
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(
                400,
                json={
                    "error": {
                        "message": "This model's maximum context length is ...",
                        "type": "invalid_request_error",
                        "code": "context_length_exceeded",
                    }
                },
            )
        )
        with pytest.raises(OpenAIContextLengthError) as excinfo:
            await provider.chat(
                [Message(role="user", content=secret_prompt)],
                response_model=_Answer,
            )

    # Prompt content MUST NOT leak into the exception message.
    assert secret_prompt not in str(excinfo.value), (
        "OpenAIContextLengthError message MUST NOT echo the prompt content"
    )

    # _classify_exception MUST map it to the new wire code.
    code = _classify_exception(excinfo.value)
    assert code == "OPENAI_CONTEXT_EXCEEDED", (
        f"expected OPENAI_CONTEXT_EXCEEDED, got {code!r}"
    )
    assert "OPENAI_CONTEXT_EXCEEDED" in ERROR_CODES, (
        "ERROR_CODES allowlist must include the new code so it reaches the wire"
    )


async def test_temperature_and_max_tokens_pass_through(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Temperature and max_tokens passed to OpenAI".

    Per-role temperature tuning (reasoning=0.1, judge=0.0, chat=0.2) is
    only effective if the kwargs actually land in the wire request body;
    this test pins that plumbing contract.
    """
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIChatProvider(
        "gpt-4o-mini", temperature=0.0, max_tokens=512
    )
    with respx.mock(assert_all_called=True) as mock:
        route = mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(
                200, json=_tool_call_response({"answer": "ok"})
            )
        )
        await provider.chat(
            [Message(role="user", content="ping")],
            response_model=_Answer,
        )

    assert route.called, "expected exactly one call to OpenAI chat completions"
    request_body = json.loads(route.calls[0].request.content)
    assert request_body.get("temperature") == 0.0, (
        f"temperature MUST pass through; got {request_body!r}"
    )
    assert request_body.get("max_tokens") == 512, (
        f"max_tokens MUST pass through; got {request_body!r}"
    )


async def test_registry_rejects_unwrapped_openai_chat_provider(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Provider must be registered through TrackedProvider".

    Even with a real OpenAI chat provider, callers cannot register it
    directly — every provider SHALL be wrapped in `TrackedProvider` first.
    """
    from codebus_agent.providers import (
        ProviderRegistry,
        ProviderRegistryError,
        ProviderRole,
    )

    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    provider = OpenAIChatProvider("gpt-4o-mini")

    with pytest.raises(ProviderRegistryError):
        ProviderRegistry({ProviderRole.CHAT: provider})  # type: ignore[dict-item]
