"""End-to-end integration test for the sanitizer-safety-chain change.

Covers tasks 9.1 and 9.2 from
openspec/changes/sanitizer-safety-chain/tasks.md:
  9.1 — TrackedProvider chat call writes pass=2 audit line to
        sanitize_audit.jsonl AND a llm_calls.jsonl line with
        sanitized payload + sanitizer_pass2_applied: true.
  9.2 — zero outbound HTTP invariant still holds across the Pass 2
        sanitize + dispatch path.
"""
from __future__ import annotations

import json
import socket
from pathlib import Path
from typing import Any

import pytest
from pydantic import BaseModel

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider
from codebus_agent.providers.protocol import Message, ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

_LOOPBACK_HOSTS = {"127.0.0.1", "::1", "localhost", ""}


class _Plan(BaseModel):
    title: str = ""


def _read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


@pytest.mark.asyncio
async def test_end_to_end_chat_call_writes_both_audit_files(tmp_path: Path) -> None:
    """9.1 — one chat call threads through Pass 2 and leaves trails in
    both sanitize_audit.jsonl (pass=2) and llm_calls.jsonl (sanitized)."""
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger_path = tmp_path / "llm_calls.jsonl"
    logger = LLMCallLogger(logger_path)
    audit_path = tmp_path / "sanitize_audit.jsonl"
    audit = SanitizerAuditLogger(audit_path)
    wrapped = TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=ProviderRole.CHAT,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=audit,
        rules_version="2026-04-20-1",
    )

    await wrapped.chat(
        messages=[
            Message(
                role="user",
                content="please ping alice@example.com about 10.0.3.42",
            )
        ],
        response_model=_Plan,
    )

    audit_lines = _read_jsonl(audit_path)
    assert audit_lines, "sanitize_audit.jsonl must have at least one line"
    assert all(line["pass"] == 2 for line in audit_lines)
    kinds = {line["kind"] for line in audit_lines}
    assert "email" in kinds
    assert "ip" in kinds
    assert all(line["source"].startswith("message:") for line in audit_lines)

    call_lines = _read_jsonl(logger_path)
    assert len(call_lines) == 1
    call = call_lines[0]
    assert call["sanitizer_pass2_applied"] is True
    payload = json.dumps(call, ensure_ascii=False)
    assert "alice@example.com" not in payload
    assert "10.0.3.42" not in payload
    assert "<REDACTED:email#" in payload
    assert "<REDACTED:ip#" in payload


@pytest.mark.asyncio
async def test_zero_outbound_invariant_still_holds(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """9.2 — Pass 2 sanitize + dispatch must not produce any non-loopback
    socket connect.  Mirrors the fixture under
    tests/providers/conftest.py::block_outbound_sockets so callers can
    audit socket-level escapes without requiring the `respx` extra.
    """
    blocked: list[Any] = []
    original_connect = socket.socket.connect

    def guarded_connect(self: socket.socket, address: Any) -> None:
        host = address[0] if isinstance(address, tuple) else str(address)
        if host in _LOOPBACK_HOSTS:
            original_connect(self, address)
            return
        blocked.append(address)
        raise RuntimeError(
            f"outbound socket connect to {address!r} blocked during "
            f"Pass 2 dispatch"
        )

    monkeypatch.setattr(socket.socket, "connect", guarded_connect)

    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    audit = SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl")
    wrapped = TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=ProviderRole.CHAT,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=audit,
        rules_version="2026-04-20-1",
    )

    await wrapped.chat(
        messages=[
            Message(role="user", content="ping alice@example.com"),
        ],
        response_model=_Plan,
    )
    await wrapped.embed(texts=["call 0912-345-678"])

    assert blocked == [], (
        f"Pass 2 dispatch unexpectedly attempted outbound traffic: {blocked!r}"
    )
