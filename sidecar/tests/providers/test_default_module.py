"""TDD red tests for `TrackedProvider.default_module` — Section 1 of
openspec/changes/usage-tracker-dedup/tasks.md.

Backs openspec/changes/usage-tracker-dedup/specs/llm-provider/spec.md
  Requirement: TrackedProvider tags usage records with default_module

The bug being fixed: `kb-build-production-wiring` smoke test revealed
that every embed call was recorded twice in `<workspace>/token_usage.jsonl`
— once by `TrackedProvider.embed` (no module label) and once by
`KnowledgeBase.build` (with `module="kb_build"`). The fix collapses
both records into one by letting `TrackedProvider` carry the module
label itself, and removing `KnowledgeBase`'s manual `tracker.record()`.

These tests pin TrackedProvider's contract; the KB-side dedup test
lives in Section 3 (`test_kb_build_does_not_double_record_usage`).
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import Message, ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


class _Plan(BaseModel):
    title: str = ""


def _read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _build_tracked(
    tmp_path: Path,
    *,
    inner=None,
    role: ProviderRole = ProviderRole.CHAT,
    default_module: str | None = None,
) -> TrackedProvider:
    """Helper to build a TrackedProvider with the new `default_module` kwarg."""
    return TrackedProvider(
        inner or MockProvider(),
        tracker=UsageTracker(tmp_path / "token_usage.jsonl"),
        logger=LLMCallLogger(tmp_path / "llm_calls.jsonl"),
        role=role,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
        rules_version="2026-04-20-1",
        default_module=default_module,
    )


async def test_tracked_provider_records_default_module_on_embed(
    tmp_path: Path,
) -> None:
    """Spec scenario "Default module reaches usage record" (embed path)."""
    tracked = _build_tracked(
        tmp_path,
        inner=MockProvider(),
        role=ProviderRole.EMBED,
        default_module="kb_build",
    )
    await tracked.embed(["alpha", "beta"])

    lines = _read_jsonl(tmp_path / "token_usage.jsonl")
    embed_lines = [l for l in lines if l.get("operation") == "embed"]
    assert len(embed_lines) == 1, f"expected 1 embed line, got {len(embed_lines)}"
    assert embed_lines[0]["module"] == "kb_build", (
        f"expected module='kb_build', got {embed_lines[0].get('module')!r}"
    )


async def test_tracked_provider_records_default_module_on_chat(
    tmp_path: Path,
) -> None:
    """Spec scenario "Default module reaches usage record" (chat path).

    TrackedProvider tags usage records with default_module — chat path
    SHALL apply the same module tag as embed.
    """
    script = MockScript()
    script.push(_Plan(title="hi"))
    tracked = _build_tracked(
        tmp_path,
        inner=MockProvider(script=script),
        role=ProviderRole.CHAT,
        default_module="qa_agent",
    )
    await tracked.chat(
        [Message(role="user", content="hi")], response_model=_Plan
    )

    lines = _read_jsonl(tmp_path / "token_usage.jsonl")
    chat_lines = [l for l in lines if l.get("operation") == "chat"]
    assert len(chat_lines) == 1
    assert chat_lines[0]["module"] == "qa_agent"


async def test_omitting_default_module_writes_empty_string(
    tmp_path: Path,
) -> None:
    """Spec scenario "Omitting default_module preserves M1 behavior".

    Backward compat: TrackedProvider built with the M1 signature (no
    `default_module` kwarg) MUST NOT raise, and the usage line's
    `module` field MUST be the empty string.
    """
    tracked = _build_tracked(tmp_path, role=ProviderRole.EMBED)  # no default_module
    await tracked.embed(["x"])

    lines = _read_jsonl(tmp_path / "token_usage.jsonl")
    embed_lines = [l for l in lines if l.get("operation") == "embed"]
    assert len(embed_lines) == 1
    assert embed_lines[0]["module"] == "", (
        f"expected module='' for backward compat, got {embed_lines[0].get('module')!r}"
    )


async def test_failure_path_still_records_default_module(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Failure path still records with default_module".

    When the inner provider raises, the LLMCallLogger failure record
    MUST still be written (existing contract). When a usage line is
    written for the failed call (M1 currently writes none on chat
    failure), it MUST carry the `module` tag so retry costs land on
    the right subsystem.
    """
    inner = MockProvider()

    async def _boom(*_args, **_kwargs):
        raise RuntimeError("boom")

    monkeypatch.setattr(inner, "chat", _boom)

    tracked = _build_tracked(
        tmp_path,
        inner=inner,
        role=ProviderRole.CHAT,
        default_module="kb_build",
    )

    with pytest.raises(RuntimeError):
        await tracked.chat(
            [Message(role="user", content="hi")], response_model=_Plan
        )

    # Failure goes to llm_calls.jsonl unconditionally per existing contract.
    call_log = _read_jsonl(tmp_path / "llm_calls.jsonl")
    assert any(line.get("response") is None for line in call_log), (
        "expected at least one failure record (response=null) in llm_calls.jsonl"
    )

    # If a usage line was written for the failed call, it MUST carry the module.
    # M1 currently does NOT record on chat failure; we don't force a count
    # here — only assert that IF any was written, it has the module tag.
    usage_path = tmp_path / "token_usage.jsonl"
    usage_lines = _read_jsonl(usage_path) if usage_path.exists() else []
    for line in usage_lines:
        assert line.get("module") == "kb_build", (
            f"failure-path usage line missing module tag: {line}"
        )
