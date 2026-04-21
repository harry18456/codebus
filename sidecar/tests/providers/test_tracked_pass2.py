"""Tests for TrackedProvider Sanitizer Pass 2 wiring — covers Requirements
"TrackedProvider applies Sanitizer Pass 2 before dispatch" and
"TrackedProvider writes audit entries to sanitize_audit.jsonl"
from openspec/changes/sanitizer-safety-chain/specs/llm-provider/spec.md.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock, patch

import pytest
from pydantic import BaseModel

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import Message, ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import (
    SanitizerAuditLogger,
    SanitizerEngine,
    SanitizerError,
)


class _Plan(BaseModel):
    title: str = ""


def _build_wrapped(
    tmp_path: Path,
    *,
    role: ProviderRole = ProviderRole.CHAT,
    sanitizer: SanitizerEngine | None = None,
) -> tuple[TrackedProvider, Path, Path]:
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger_path = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(logger_path)
    audit_path = tmp_path / "sanitize_audit.jsonl"
    audit = SanitizerAuditLogger(audit_path)
    wrapped = TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=role,
        sanitizer=sanitizer or SanitizerEngine(),
        sanitizer_audit=audit,
        rules_version="2026-04-20-1",
    )
    return wrapped, logger_path, audit_path


def _read_lines(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


@pytest.mark.asyncio
async def test_tracked_provider_chat_sanitizes_before_wrapped_provider(tmp_path):
    """The wrapped MockProvider.chat MUST receive sanitized messages."""
    wrapped, logger_path, _ = _build_wrapped(tmp_path)

    captured: dict[str, Any] = {}
    original_chat = wrapped._inner.chat

    async def spy_chat(messages, *, response_model):
        captured["messages"] = [
            Message(role=m.role, content=m.content, tool_call_id=m.tool_call_id)
            for m in messages
        ]
        return await original_chat(messages, response_model=response_model)

    wrapped._inner.chat = spy_chat  # type: ignore[method-assign]

    await wrapped.chat(
        messages=[Message(role="user", content="alice@example.com")],
        response_model=_Plan,
    )

    assert captured["messages"][0].content == "<REDACTED:email#1>"
    # And the llm_calls.jsonl line records the sanitized content.
    call_lines = _read_lines(logger_path)
    assert len(call_lines) == 1
    request_msgs = call_lines[0]["request"]["messages"]
    assert request_msgs[0]["content"] == "<REDACTED:email#1>"


@pytest.mark.asyncio
async def test_tracked_provider_embed_sanitizes_texts(tmp_path):
    wrapped, _, _ = _build_wrapped(tmp_path, role=ProviderRole.EMBED)

    captured: dict[str, Any] = {}
    original_embed = wrapped._inner.embed

    async def spy_embed(texts):
        captured["texts"] = list(texts)
        return await original_embed(texts)

    wrapped._inner.embed = spy_embed  # type: ignore[method-assign]

    await wrapped.embed(texts=["contact 0912-345-678 today"])

    assert len(captured["texts"]) == 1
    assert "0912-345-678" not in captured["texts"][0]
    assert "<REDACTED:phone#1>" in captured["texts"][0]


@pytest.mark.asyncio
async def test_sanitizer_pass2_applied_field_true_after_call(tmp_path):
    wrapped, logger_path, _ = _build_wrapped(tmp_path)

    await wrapped.chat(
        messages=[Message(role="user", content="hello")],
        response_model=_Plan,
    )

    call_lines = _read_lines(logger_path)
    assert len(call_lines) == 1
    assert call_lines[0]["sanitizer_pass2_applied"] is True
    # Type MUST stay boolean (no breaking change from M1 schema).
    assert isinstance(call_lines[0]["sanitizer_pass2_applied"], bool)


@pytest.mark.asyncio
async def test_tracked_provider_sanitizer_failure_aborts_dispatch(tmp_path):
    """SanitizerError must prevent inner.chat from being called and must
    NOT produce any llm_calls.jsonl line."""

    class _BoomRule:
        rule_id = "boom_v1"
        kind = "email"

        def find(self, text):
            raise RuntimeError("engine exploded")

    boom_engine = SanitizerEngine(rules=[_BoomRule()])
    wrapped, logger_path, audit_path = _build_wrapped(
        tmp_path, sanitizer=boom_engine
    )

    inner_chat_called = {"n": 0}
    original_chat = wrapped._inner.chat

    async def spy_chat(messages, *, response_model):
        inner_chat_called["n"] += 1
        return await original_chat(messages, response_model=response_model)

    wrapped._inner.chat = spy_chat  # type: ignore[method-assign]

    with pytest.raises(SanitizerError):
        await wrapped.chat(
            messages=[Message(role="user", content="hi")],
            response_model=_Plan,
        )

    assert inner_chat_called["n"] == 0
    assert _read_lines(logger_path) == []


@pytest.mark.asyncio
async def test_pass2_audit_entry_written_with_message_prefix(tmp_path):
    wrapped, _, audit_path = _build_wrapped(tmp_path)

    await wrapped.chat(
        messages=[Message(role="user", content="ping alice@example.com")],
        response_model=_Plan,
    )

    audit_lines = _read_lines(audit_path)
    assert len(audit_lines) == 1
    line = audit_lines[0]
    assert line["pass"] == 2
    assert line["source"].startswith("message:")
    assert "chat_req_" in line["source"]
    assert line["kind"] == "email"
    assert line["rules_version"] == "2026-04-20-1"
    assert line["schema_version"] == 1


@pytest.mark.asyncio
async def test_pass2_sanitizes_every_message_in_list(tmp_path):
    wrapped, _, audit_path = _build_wrapped(tmp_path)
    original_chat = wrapped._inner.chat
    captured: dict[str, Any] = {}

    async def spy_chat(messages, *, response_model):
        captured["messages"] = [m.content for m in messages]
        return await original_chat(messages, response_model=response_model)

    wrapped._inner.chat = spy_chat  # type: ignore[method-assign]

    await wrapped.chat(
        messages=[
            Message(role="system", content="be helpful"),
            Message(role="user", content="reach me at alice@example.com"),
            Message(role="assistant", content="Sure, calling 10.0.3.42"),
        ],
        response_model=_Plan,
    )

    for content in captured["messages"]:
        assert "alice@example.com" not in content
        assert "10.0.3.42" not in content

    audit_lines = _read_lines(audit_path)
    kinds = {line["kind"] for line in audit_lines}
    assert "email" in kinds
    assert "ip" in kinds


def test_tracked_provider_requires_sanitizer(tmp_path):
    """Constructing TrackedProvider without a sanitizer SHALL raise
    ValueError — the Pass 2 invariant can't be bypassed from call sites."""
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")

    with pytest.raises(ValueError, match="sanitizer"):
        TrackedProvider(
            MockProvider(),
            tracker=tracker,
            logger=logger,
            role=ProviderRole.CHAT,
            sanitizer=None,  # type: ignore[arg-type]
            sanitizer_audit=SanitizerAuditLogger(tmp_path / "audit.jsonl"),
            rules_version="v1",
        )


def test_tracked_provider_requires_audit(tmp_path):
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")

    with pytest.raises(ValueError, match="sanitizer_audit"):
        TrackedProvider(
            MockProvider(),
            tracker=tracker,
            logger=logger,
            role=ProviderRole.CHAT,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=None,  # type: ignore[arg-type]
            rules_version="v1",
        )
