"""Defensive tests for `_think` / `_qa_think` provider-wire-prompt ordering.

Backs SHALL clauses in
openspec/changes/react-message-ordering-fix/specs/agent-core/spec.md
  MODIFIED Requirement: Explorer applies rolling message window before
    each Think call
    Scenario: System message is first element of provider.chat payload
    Scenario: Leading orphan tool messages are stripped from windowed history
    Scenario: Assistant tool_calls and matching tool messages stay paired
    Scenario: Think receives at most window-size messages when state grew larger
openspec/changes/react-message-ordering-fix/specs/qa-agent/spec.md
  MODIFIED Requirement: Q&A loop entry point with two-stage RAG-first flow
    Scenario: `_qa_think` provider wire prompt starts with system message
    Scenario: `_qa_think` strips leading orphan tool messages

Lock the OpenAI Chat Completions wire-format invariant
`[system, *windowed_history(no leading orphan tool), user]` so the
production-blocker `400 invalid_request_error` ("messages with role
'tool' must be a response to a preceding message with 'tool_calls'")
seen during Phase 7 e2e cannot recur silently. Tests pass `SpyProvider`
directly to `_think` / `_qa_think`; no TrackedProvider wrap because
the assertion surface is the wire-format the helper builds, not the
Sanitizer Pass 2 lane (covered elsewhere).
"""
from __future__ import annotations

import pytest

from codebus_agent.agent.explorer import _MESSAGE_ROLLING_WINDOW, _think
from codebus_agent.agent.prompts.explorer import EXPLORER_SYSTEM
from codebus_agent.agent.prompts.qa import QA_SYSTEM
from codebus_agent.agent.qa import _qa_think
from codebus_agent.agent.types import Message

from ._message_ordering_helpers import (
    SpyProvider,
    make_explorer_state,
    make_qa_state,
)


# ---------------------------------------------------------------------------
# Explorer `_think` ordering scenarios
# ---------------------------------------------------------------------------


@pytest.mark.anyio("asyncio")
async def test_explorer_think_system_first_user_last() -> None:
    """Spec scenario `System message is first element of provider.chat payload`.

    Use a non-empty history so the buggy `[history, system, user]`
    layout produces wire[0].role == something-other-than-system. The
    empty-history case passes trivially (no history to mis-order) and
    would not exercise the bug.
    """
    spy = SpyProvider()
    state = make_explorer_state(
        messages_history=[Message(role="user", content="prior turn")]
    )

    await _think(state, spy, tool_specs=[])

    assert spy.last_messages is not None, "_think MUST issue a single chat call"
    wire = spy.last_messages
    assert wire[0].role == "system", (
        f"system MUST be first element; got role={wire[0].role!r}"
    )
    assert wire[0].content == EXPLORER_SYSTEM, (
        "first element's content MUST equal EXPLORER_SYSTEM constant"
    )
    assert wire[-1].role == "user", (
        f"user prompt MUST be last element; got role={wire[-1].role!r}"
    )


@pytest.mark.anyio("asyncio")
async def test_explorer_think_strips_leading_orphan_tool() -> None:
    """Spec scenario `Leading orphan tool messages are stripped from windowed history`.

    Window-slicing may strip the preceding `assistant tool_calls` and
    leave a `tool` role message at the head — this is an orphan that
    OpenAI rejects with 400. `_think` MUST trim it before dispatch.
    """
    spy = SpyProvider()
    history = [
        Message(role="tool", content="orphan", tool_call_id="x", tool_name="probe"),
        Message(role="user", content="next"),
    ]
    state = make_explorer_state(messages_history=history)

    await _think(state, spy, tool_specs=[])

    wire = spy.last_messages
    assert wire is not None
    # Walk the wire and verify every `tool` message is preceded by either
    # an `assistant` or another `tool` chained from one (no orphans).
    for i, m in enumerate(wire):
        if m.role != "tool":
            continue
        # i == 0 is forbidden: a tool message at the head is by
        # definition an orphan because the system message is index 0.
        assert i > 0, "no `tool` message MUST appear at the head of the wire"
        prev = wire[i - 1]
        assert prev.role in {"assistant", "tool"}, (
            f"tool@{i} preceded by role={prev.role!r}; orphan tool messages "
            f"violate the OpenAI Chat Completions ordering contract"
        )


@pytest.mark.anyio("asyncio")
async def test_explorer_think_keeps_paired_assistant_tool() -> None:
    """Spec scenario `Assistant tool_calls and matching tool messages stay paired`.

    When the head of the windowed slice IS an `assistant` (with
    `tool_calls` semantically — agent-layer Message has no tool_calls
    field, but role==assistant is the marker), the trim MUST leave it
    AND its trailing `tool` messages alone.
    """
    spy = SpyProvider()
    history = [
        Message(role="assistant", content=""),
        Message(role="tool", content="result", tool_call_id="x", tool_name="probe"),
        Message(role="user", content="next"),
    ]
    state = make_explorer_state(messages_history=history)

    await _think(state, spy, tool_specs=[])

    wire = spy.last_messages
    assert wire is not None
    roles = [m.role for m in wire]
    # Both assistant and tool MUST appear; assistant MUST come before tool.
    assert "assistant" in roles, "paired assistant MUST NOT be stripped"
    assert "tool" in roles, "paired tool MUST NOT be stripped"
    assert roles.index("assistant") < roles.index("tool"), (
        "assistant MUST precede its tool reply in the wire payload"
    )


@pytest.mark.anyio("asyncio")
async def test_explorer_think_window_size_respected() -> None:
    """Spec scenario `Think receives at most window-size messages when state grew larger`.

    Pre-seed state.messages with 20 entries shaped so the head is NOT
    an orphan tool (a `user` then alternating valid pairs). After
    `_think`, the wire payload's history slice MUST be at most
    `_MESSAGE_ROLLING_WINDOW`; total length is window + 2 (system +
    user appended).
    """
    history: list[Message] = [Message(role="user", content="seed")]
    # Add 19 more valid messages — alternating user / tool sequences
    # are fine here because the test only verifies the slice bound,
    # not orphan-tool stripping (that's covered in 2.2).
    for i in range(19):
        if i % 2 == 0:
            history.append(Message(role="user", content=f"u{i}"))
        else:
            history.append(
                Message(
                    role="assistant",
                    content=f"a{i}",
                )
            )
    assert len(history) == 20

    spy = SpyProvider()
    state = make_explorer_state(messages_history=history)
    await _think(state, spy, tool_specs=[])

    wire = spy.last_messages
    assert wire is not None
    # Wire = system (1) + windowed history (≤ WINDOW) + user (1)
    assert wire[0].role == "system"
    assert wire[-1].role == "user"
    history_slice = wire[1:-1]
    assert len(history_slice) <= _MESSAGE_ROLLING_WINDOW, (
        f"history slice MUST be ≤ _MESSAGE_ROLLING_WINDOW "
        f"({_MESSAGE_ROLLING_WINDOW}); got {len(history_slice)}"
    )


# ---------------------------------------------------------------------------
# Q&A `_qa_think` ordering scenarios
# ---------------------------------------------------------------------------


@pytest.mark.anyio("asyncio")
async def test_qa_think_system_first_user_last() -> None:
    """Spec scenario `_qa_think provider wire prompt starts with system message`.

    Non-empty history (mirrors `_think` ordering test) so the buggy
    layout exposes wire[0].role != "system".
    """
    spy = SpyProvider()
    state = make_qa_state(
        messages_history=[Message(role="user", content="prior question")]
    )

    await _qa_think(state, spy, user_prompt="what does storage do?")

    assert spy.last_messages is not None, "_qa_think MUST issue a single chat call"
    wire = spy.last_messages
    assert wire[0].role == "system", (
        f"system MUST be first element; got role={wire[0].role!r}"
    )
    assert wire[0].content == QA_SYSTEM, (
        "first element's content MUST equal QA_SYSTEM constant"
    )
    assert wire[-1].role == "user", (
        f"user prompt MUST be last element; got role={wire[-1].role!r}"
    )


@pytest.mark.anyio("asyncio")
async def test_qa_think_strips_leading_orphan_tool() -> None:
    """Spec scenario `_qa_think strips leading orphan tool messages`."""
    spy = SpyProvider()
    history = [
        Message(role="tool", content="orphan", tool_call_id="x", tool_name="probe"),
    ]
    state = make_qa_state(messages_history=history)

    await _qa_think(state, spy, user_prompt="follow up question")

    wire = spy.last_messages
    assert wire is not None
    for i, m in enumerate(wire):
        if m.role != "tool":
            continue
        assert i > 0, "no `tool` message MUST appear at the head of the wire"
        prev = wire[i - 1]
        assert prev.role in {"assistant", "tool"}, (
            f"tool@{i} preceded by role={prev.role!r}; orphan tool messages "
            f"violate the OpenAI Chat Completions ordering contract"
        )
