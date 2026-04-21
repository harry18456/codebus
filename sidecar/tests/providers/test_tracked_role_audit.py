"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: TrackedProvider records role in audit log
    Scenario: Audit record contains role field
    Scenario: Role field is additive to existing audit schema

Design llm-role-routing §6: TrackedProvider auto-emits role into
llm_calls.jsonl; caller signatures (chat / embed) unchanged.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers import (
    LLMCallLogger,
    MockProvider,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)
from codebus_agent.providers.protocol import Message
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


class _Echo(BaseModel):
    content: str = ""


def _read_lines(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _build_tracked(
    tmp_path: Path, *, role: ProviderRole
) -> tuple[TrackedProvider, Path]:
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    log_path = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(log_path)
    wrapped = TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=role,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
        rules_version="test-v1",
    )
    return wrapped, log_path


@pytest.mark.asyncio
async def test_chat_call_records_role_field(tmp_path: Path) -> None:
    """Scenario: Audit record contains role field (chat path)."""
    wrapped, log_path = _build_tracked(tmp_path, role=ProviderRole.JUDGE)

    await wrapped.chat(
        messages=[Message(role="user", content="is this relevant?")],
        response_model=_Echo,
    )

    entry = _read_lines(log_path)[0]
    assert entry["role"] == "judge"


@pytest.mark.asyncio
async def test_embed_call_records_role_field(tmp_path: Path) -> None:
    """Scenario: Audit record contains role field (embed path)."""
    wrapped, log_path = _build_tracked(tmp_path, role=ProviderRole.EMBED)

    await wrapped.embed(texts=["hello"])

    entry = _read_lines(log_path)[0]
    assert entry["role"] == "embed"


@pytest.mark.asyncio
async def test_audit_record_preserves_m1_schema_fields(tmp_path: Path) -> None:
    """Scenario: Role field is additive to existing audit schema.

    Consumers written against M1 must still find all listed fields
    (timestamp / provider_id / model / sanitizer_pass2_applied /
    prompt_tokens / completion_tokens) with their M1 types.
    """
    wrapped, log_path = _build_tracked(tmp_path, role=ProviderRole.REASONING)

    await wrapped.chat(
        messages=[Message(role="user", content="hello")],
        response_model=_Echo,
    )

    entry = _read_lines(log_path)[0]
    assert isinstance(entry["timestamp"], str) and entry["timestamp"]
    assert isinstance(entry["provider_id"], str) and entry["provider_id"]
    assert isinstance(entry["model"], str) and entry["model"]
    assert entry["sanitizer_pass2_applied"] is True
    assert isinstance(entry["prompt_tokens"], int) and entry["prompt_tokens"] >= 0
    assert isinstance(entry["completion_tokens"], int)
    assert entry["completion_tokens"] >= 0


@pytest.mark.asyncio
async def test_failure_path_records_role_field(tmp_path: Path) -> None:
    """Failure path also carries role — audit chain must not break on error."""
    wrapped, log_path = _build_tracked(tmp_path, role=ProviderRole.CHAT)

    async def _boom(*args: object, **kwargs: object) -> None:
        raise RuntimeError("mock failure")

    wrapped._inner.chat = _boom  # type: ignore[assignment]

    with pytest.raises(RuntimeError, match="mock failure"):
        await wrapped.chat(
            messages=[Message(role="user", content="x")],
            response_model=_Echo,
        )

    entry = _read_lines(log_path)[0]
    assert entry["role"] == "chat"
    assert entry["response"] is None
    assert entry["error"]["class"] == "RuntimeError"
