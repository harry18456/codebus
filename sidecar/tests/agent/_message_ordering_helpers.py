"""Test-only helpers for `_think` / `_qa_think` message-ordering tests.

Backs SHALL clauses in
openspec/changes/react-message-ordering-fix/specs/agent-core/spec.md
  MODIFIED Requirement: Explorer applies rolling message window before
    each Think call (system-first; leading orphan tool stripped)
openspec/changes/react-message-ordering-fix/specs/qa-agent/spec.md
  MODIFIED Requirement: Q&A loop entry point with two-stage RAG-first flow
    (`_qa_think` ordering rules)

`SpyProvider` is a minimal `LLMProvider`-shaped spy that records the
`messages` arg of every `chat` invocation. Tests pass it DIRECTLY into
`_think` / `_qa_think` (both accept any object with a `.chat()` method
— their type annotations are not runtime-checked).

We deliberately skip wrapping with `TrackedProvider` here. The scope
of these tests is `_think` / `_qa_think`'s wire-format construction;
Sanitizer Pass 2 / audit lanes are exercised by existing tests in
`tests/providers/`. Skipping the wrap keeps the assertion surface
narrow and stable. `add_spy_to_allowlist(monkeypatch)` is provided for
future tests that DO want the wrapped path.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from pydantic import BaseModel

from codebus_agent.agent.types import (
    ExplorerAction,
    ExplorerState,
    Message,
    QAAction,
    QAState,
)
from codebus_agent.providers.protocol import Message as ProviderMessage


__all__ = [
    "SpyProvider",
    "add_spy_to_allowlist",
    "make_explorer_state",
    "make_qa_state",
]


@dataclass
class SpyProvider:
    """`LLMProvider`-shaped spy that records every `chat` invocation.

    `last_messages` holds the most recent `messages` list received
    (handy for single-call tests). `calls` keeps the full per-call
    history so multi-iteration tests can walk every wire payload.

    Returns a deterministic empty dummy instance of `response_model`
    so the caller's downstream logic completes — Explorer expects
    `ExplorerAction(thought=..., tool_calls=...)`; Q&A expects
    `QAAction(thought=..., tool_calls=...)`. No FIFO script is needed
    because the message-ordering tests exercise a single
    `_think` / `_qa_think` call per test.
    """

    name: str = "spy"
    last_messages: list[ProviderMessage] | None = None
    calls: list[list[ProviderMessage]] = field(default_factory=list)

    async def chat(
        self,
        messages: list[ProviderMessage],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        captured = list(messages)
        self.last_messages = captured
        self.calls.append(captured)
        if response_model is ExplorerAction:
            return ExplorerAction(thought="dummy", tool_calls=[], stop=False)
        if response_model is QAAction:
            return QAAction(thought="dummy", tool_calls=[])
        return response_model.model_validate({})


def make_explorer_state(
    *,
    messages_history: list[Message] | None = None,
    task: str = "fix the message ordering bug",
    budget_steps_left: int = 5,
    budget_tokens_left: int = 100_000,
) -> ExplorerState:
    """Build an `ExplorerState` pre-seeded with `messages_history`."""
    return ExplorerState(
        task=task,
        messages=list(messages_history or []),
        budget_steps_left=budget_steps_left,
        budget_tokens_left=budget_tokens_left,
    )


def make_qa_state(
    *,
    messages_history: list[Message] | None = None,
    question: str = "how does storage work",
    session_id: str = "qa_sess_test",
    originating_station_id: str | None = "s02-storage",
) -> QAState:
    """Build a `QAState` pre-seeded with `messages_history`."""
    return QAState(
        question=question,
        originating_station_id=originating_station_id,
        session_id=session_id,
        messages=list(messages_history or []),
    )


def add_spy_to_allowlist(monkeypatch: Any) -> None:
    """Register `SpyProvider` in `TrackedProvider.ALLOWED_INNER_TYPES`.

    Only needed for tests that wrap `SpyProvider` with `TrackedProvider`
    (none of the message-ordering tests do; they pass `SpyProvider`
    directly into `_think` / `_qa_think`). Provided as a knob in case
    a future regression test exercises the full Pass 2 pipeline.
    """
    from codebus_agent.providers.tracked import TrackedProvider

    augmented = frozenset(
        TrackedProvider.ALLOWED_INNER_TYPES | {SpyProvider}
    )
    monkeypatch.setattr(TrackedProvider, "ALLOWED_INNER_TYPES", augmented)
