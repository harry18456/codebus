"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: Mock provider returns Instructor-compatible output
    Scenario: Mock chat satisfies response_model
    Scenario: Mock script controls output
    Scenario: Mock embed returns deterministic vector

Design context: D-local-4 — Mock provider walks the real
Instructor / Pydantic parsing path (no dict stubbing).
"""
from __future__ import annotations

import pytest
from pydantic import BaseModel, ValidationError

from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import Message


class _Plan(BaseModel):
    title: str
    steps: int
    tags: list[str]


class _Nested(BaseModel):
    plan: _Plan
    note: str | None = None


@pytest.mark.asyncio
async def test_chat_auto_generates_valid_instance() -> None:
    """Scenario: Mock chat satisfies response_model."""
    provider = MockProvider()
    result = await provider.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Plan,
    )
    assert isinstance(result, _Plan)
    assert isinstance(result.title, str)
    assert isinstance(result.steps, int)
    assert isinstance(result.tags, list)


@pytest.mark.asyncio
async def test_chat_auto_generates_passes_validation_for_nested_model() -> None:
    """Exercises Pydantic's real validation path through nested submodels."""
    provider = MockProvider()
    result = await provider.chat(
        messages=[Message(role="user", content="n")],
        response_model=_Nested,
    )
    assert isinstance(result, _Nested)
    assert isinstance(result.plan, _Plan)


@pytest.mark.asyncio
async def test_chat_script_pins_output_and_consumes_entry() -> None:
    """Scenario: Mock script controls output."""
    pinned = _Plan(title="fixed", steps=7, tags=["a", "b"])
    script = MockScript()
    script.push(pinned)

    provider = MockProvider(script=script)
    first = await provider.chat(
        messages=[Message(role="user", content="x")],
        response_model=_Plan,
    )
    assert first is pinned
    assert script.empty

    second = await provider.chat(
        messages=[Message(role="user", content="x")],
        response_model=_Plan,
    )
    assert second is not pinned
    assert isinstance(second, _Plan)


@pytest.mark.asyncio
async def test_chat_script_entry_type_must_match_response_model() -> None:
    class _Other(BaseModel):
        foo: str

    script = MockScript()
    script.push(_Other(foo="x"))
    provider = MockProvider(script=script)

    with pytest.raises(TypeError):
        await provider.chat(
            messages=[Message(role="user", content="x")],
            response_model=_Plan,
        )


@pytest.mark.asyncio
async def test_embed_is_deterministic_for_same_input() -> None:
    """Scenario: Mock embed returns deterministic vector."""
    provider = MockProvider()
    first = await provider.embed(texts=["hello"])
    second = await provider.embed(texts=["hello"])
    assert first.vectors == second.vectors
    assert len(first.vectors) == 1
    assert len(first.vectors[0]) == provider.embedding_dim


@pytest.mark.asyncio
async def test_embed_returns_usage_with_embed_call_type() -> None:
    provider = MockProvider()
    res = await provider.embed(texts=["hello", "world"])
    assert res.usage.call_type == "embed"
    assert res.vectors[0] != res.vectors[1]
