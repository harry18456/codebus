"""OpenAI chat provider — production implementation for Module 4 Explorer Agent.

Backs openspec/changes/chat-provider-wiring/specs/llm-provider/spec.md
  Requirement: OpenAI chat provider

Decisions:
- D-003: all providers go through TrackedProvider — the registry guard
  enforces this at registration time.
- D-012: self-written ReAct loop uses Instructor for Pydantic structured
  output (not native OpenAI tool_calls). `chat(messages, response_model)`
  returns a validated `BaseModel` instance, letting the Agent layer
  dispatch on fields rather than parsing raw JSON.

Key contracts:
- API key SHALL be read only from `CODEBUS_OPENAI_API_KEY` (shared with
  `OpenAIEmbeddingProvider`); no fallback to `OPENAI_API_KEY` so the
  sidecar's degraded-mode contract (sidecar-runtime / KB dependency
  injection hook) cannot be accidentally bypassed.
- `chat()` returns a validated `BaseModel`; Instructor handles the
  tool-mode round-trip with OpenAI chat completions.
- Auth / rate-limit / context-length failures raise typed exceptions
  (`OpenAIAuthError` / `OpenAIRateLimitError` / `OpenAIContextLengthError`)
  that `api/tasks.py::_classify_exception` maps to the wire error codes
  `OPENAI_AUTH_FAILED` / `OPENAI_RATE_LIMITED` / `OPENAI_CONTEXT_EXCEEDED`.
- Retry / backoff is delegated to the underlying `openai` SDK — we do
  NOT stack additional retries in the provider.
- Exception messages MUST NOT echo prompt content (potentially sensitive)
  or the API key.
"""
from __future__ import annotations

import os
from typing import Any

import instructor
import openai
from instructor.core.exceptions import InstructorRetryException
from pydantic import BaseModel

from .openai_embedding import OpenAIAuthError, OpenAIRateLimitError
from .protocol import Message

__all__ = [
    "OpenAIChatProvider",
    "OpenAIContextLengthError",
]


_ENV_VAR = "CODEBUS_OPENAI_API_KEY"
_CONTEXT_LENGTH_CODE = "context_length_exceeded"


class OpenAIContextLengthError(Exception):
    """Raised when OpenAI rejects a chat request for oversized prompt.

    Mapped to wire code `OPENAI_CONTEXT_EXCEEDED` by `_classify_exception`.
    Message intentionally never contains prompt content (potentially
    sensitive) — only a fixed, sanitized summary.
    """


class OpenAIChatProvider:
    """Thin async wrapper around Instructor-wrapped `openai.AsyncOpenAI`.

    Construction fails fast if `CODEBUS_OPENAI_API_KEY` is absent so the
    sidecar's degraded-mode contract is unambiguous — the caller
    (`wire_kb_dependencies`) decides not to construct the provider when
    the env var is missing, rather than constructing a broken one.
    """

    name: str = "openai-chat"

    def __init__(
        self,
        model: str,
        *,
        temperature: float = 0.2,
        max_tokens: int | None = None,
    ) -> None:
        api_key = os.environ.get(_ENV_VAR)
        if not api_key:
            # Mirrors `OpenAIEmbeddingProvider`: the env var name is the
            # only supported source. No fallback to `OPENAI_API_KEY` — a
            # stray shell export MUST NOT silently bypass the sidecar's
            # degraded-mode contract.
            raise RuntimeError(
                f"{_ENV_VAR} environment variable is required to construct "
                f"OpenAIChatProvider; set it before starting the sidecar, "
                f"or leave it unset to keep chat-ish callers in graceful "
                f"503 mode."
            )
        self._model = model
        self._temperature = temperature
        self._max_tokens = max_tokens
        self._client = instructor.from_openai(openai.AsyncOpenAI(api_key=api_key))

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        wire_messages = [
            _to_openai_message(m) for m in messages
        ]
        kwargs: dict[str, Any] = {
            "model": self._model,
            "temperature": self._temperature,
            "response_model": response_model,
            "messages": wire_messages,
        }
        # Skip `max_tokens` when None so we don't send `null` on the wire
        # (OpenAI rejects null; the SDK happily omits missing keys).
        if self._max_tokens is not None:
            kwargs["max_tokens"] = self._max_tokens

        try:
            result, _completion = await self._client.chat.completions.create_with_completion(
                **kwargs
            )
        except InstructorRetryException as exc:
            # Instructor wraps underlying errors (auth / rate-limit /
            # bad-request) in its retry exception after exhausting retries.
            # Unwrap to surface the typed OpenAI exception the caller cares
            # about. `from None` suppresses the wrapped traceback so
            # leak-prone InstructorRetryException repr (which includes the
            # prompt in `create_kwargs`) never chains onto our typed error.
            inner = _unwrap_openai_exception(exc)
            _raise_typed_if_known(inner)
            raise
        except (
            openai.AuthenticationError,
            openai.RateLimitError,
            openai.BadRequestError,
        ) as exc:
            _raise_typed_if_known(exc)
            raise

        return result


def _to_openai_message(m: Message) -> dict[str, Any]:
    """Flatten `Message` into the OpenAI chat completions wire format.

    `tool_call_id` lands only when present; we intentionally do not
    include `None` fields because the OpenAI SDK rejects them.
    """
    payload: dict[str, Any] = {"role": m.role, "content": m.content}
    if m.tool_call_id is not None:
        payload["tool_call_id"] = m.tool_call_id
    return payload


def _unwrap_openai_exception(exc: InstructorRetryException) -> BaseException | None:
    """Pull the underlying OpenAI error out of Instructor's wrapper.

    Instructor's retry path records each failed attempt as a
    ``FailedAttempt(attempt_number, exception, completion)``; the last
    one is the terminal cause. Falling back to ``__cause__`` covers the
    branch where instructor uses ``raise ... from e``.
    """
    failed_attempts = getattr(exc, "failed_attempts", None)
    if failed_attempts:
        last = failed_attempts[-1]
        inner = getattr(last, "exception", None)
        if inner is not None:
            return inner
    return exc.__cause__


def _raise_typed_if_known(exc: BaseException | None) -> None:
    """Translate a known OpenAI error into a sidecar-typed one, or no-op.

    Message strings are fixed constants — never include prompt content
    or the API key. Using ``from None`` elsewhere means the chained
    traceback (which may contain sensitive headers / prompts in repr)
    does not surface through the typed exception.
    """
    if isinstance(exc, openai.AuthenticationError):
        raise OpenAIAuthError(
            "OpenAI authentication failed; verify CODEBUS_OPENAI_API_KEY"
        ) from None
    if isinstance(exc, openai.RateLimitError):
        raise OpenAIRateLimitError(
            "OpenAI rate limit exceeded after SDK retry budget; "
            "reduce concurrency or wait before retrying"
        ) from None
    if isinstance(exc, openai.BadRequestError) and _is_context_length_error(exc):
        raise OpenAIContextLengthError(
            "LLM context window exceeded for the chosen model"
        ) from None


def _is_context_length_error(exc: openai.BadRequestError) -> bool:
    """Peek at the OpenAI error body's ``code`` without echoing content.

    The OpenAI SDK exposes the decoded error payload via ``exc.body``
    (dict) when the response was JSON; older SDKs may expose it via
    ``exc.response.json()``. Both paths are guarded so an unexpected
    shape falls through to the generic re-raise rather than crashing.
    """
    body = getattr(exc, "body", None)
    if isinstance(body, dict):
        error = body.get("error")
        if isinstance(error, dict) and error.get("code") == _CONTEXT_LENGTH_CODE:
            return True
    # Secondary path: exc.code may be set directly on newer SDKs.
    code = getattr(exc, "code", None)
    if code == _CONTEXT_LENGTH_CODE:
        return True
    return False
