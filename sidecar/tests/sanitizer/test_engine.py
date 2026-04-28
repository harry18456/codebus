"""Tests for `SanitizerEngine.sanitize` — covers Requirements
"SanitizerEngine exposes pure `sanitize` interface",
"Placeholder format is `<REDACTED:kind#index>`",
and "Placeholder index scope is single sanitize call".

Post-D-033: ``SanitizerEngine.sanitize`` is async and consumes a
``PIIProvider`` injected at construction. ``SanitizerEngine()`` defaults
to a built-in ``RuleBasedPIIProvider`` so existing call sites that
relied on the implicit M1 default rule table keep working.
"""
from __future__ import annotations

import inspect
import re

import pytest

from codebus_agent.providers.pii import PIISpan
from codebus_agent.sanitizer import (
    FileSource,
    MessageSource,
    SanitizerEngine,
    SanitizerError,
)

_PLACEHOLDER_RE = re.compile(
    r"<REDACTED:(email|phone|id|secret|ip|internal-domain|jwt|private-key|credential|suspect)#\d+>"
)


@pytest.mark.asyncio
async def test_engine_pass1_replaces_email_and_returns_audit_entries():
    engine = SanitizerEngine()
    result = await engine.sanitize(
        "contact: alice@example.com",
        source=FileSource(path="src/app.py"),
    )

    assert "alice@example.com" not in result.text
    assert "<REDACTED:email#1>" in result.text
    assert len(result.entries) == 1
    entry = result.entries[0]
    assert entry.kind == "email"
    assert entry.placeholder_index == 1
    assert entry.rule_id == "pii_email_v1"
    assert entry.source == "file:src/app.py"


@pytest.mark.asyncio
async def test_placeholder_format_matches_redacted_kind_index():
    engine = SanitizerEngine()
    result = await engine.sanitize(
        "phone: 0912-345-678; email: bob@example.com; ip: 10.0.0.1",
        source=FileSource(path="x.txt"),
    )
    placeholders = _PLACEHOLDER_RE.findall(result.text)
    assert set(placeholders) == {"phone", "email", "ip"}


@pytest.mark.asyncio
async def test_same_value_same_placeholder_within_call():
    engine = SanitizerEngine()
    result = await engine.sanitize(
        "a: alice@example.com, b: alice@example.com",
        source=FileSource(path="src/a.py"),
    )
    # Both occurrences MUST map to the same placeholder string.
    assert result.text.count("<REDACTED:email#1>") == 2
    # And the audit list MUST contain exactly one entry for that value.
    emails = [e for e in result.entries if e.kind == "email"]
    assert len(emails) == 1


@pytest.mark.asyncio
async def test_placeholder_index_resets_across_calls():
    engine = SanitizerEngine()
    r1 = await engine.sanitize(
        "a: alice@example.com",
        source=FileSource(path="src/a.py"),
    )
    r2 = await engine.sanitize(
        "b: bob@example.com",
        source=FileSource(path="src/b.py"),
    )
    assert "<REDACTED:email#1>" in r1.text
    assert "<REDACTED:email#1>" in r2.text
    # Each call's first placeholder MUST be #1.
    assert r1.entries[0].placeholder_index == 1
    assert r2.entries[0].placeholder_index == 1


@pytest.mark.asyncio
async def test_engine_source_string_message_prefix():
    engine = SanitizerEngine()
    result = await engine.sanitize(
        "user said alice@example.com",
        source=MessageSource(message_id="chat_req_abc"),
    )
    assert result.entries[0].source == "message:chat_req_abc"


@pytest.mark.asyncio
async def test_engine_no_reverse_mapping_exposed():
    """Ensure the engine exposes no method that returns pre-sanitize values."""
    engine = SanitizerEngine()
    # Trigger a sanitize to populate any hypothetical internal state.
    await engine.sanitize("alice@example.com", source=FileSource(path="x.txt"))

    forbidden_method_names = {
        "reverse",
        "unsanitize",
        "resolve",
        "lookup",
        "get_original",
        "get_value",
        "unredact",
    }
    exposed = {
        name
        for name, _ in inspect.getmembers(engine)
        if not name.startswith("_")
    }
    assert exposed.isdisjoint(
        forbidden_method_names
    ), f"SanitizerEngine exposes reverse-lookup API: {exposed & forbidden_method_names}"

    # And no attribute should store the original value post-call.
    for attr_name in dir(engine):
        if attr_name.startswith("__"):
            continue
        val = getattr(engine, attr_name, None)
        if isinstance(val, (str, bytes)):
            assert "alice@example.com" not in (
                val if isinstance(val, str) else val.decode("utf-8", "ignore")
            )


@pytest.mark.asyncio
async def test_engine_fail_closed_raises_sanitizer_error():
    """When the injected PIIProvider raises, the engine MUST raise
    ``SanitizerError`` chained to the original exception (Decision:
    Fail-closed 失敗處理). Post-D-033 this is the PIIProvider-level
    failure path; the legacy ``rules=[ExplodingRule()]`` shape was
    removed when rule ownership moved to ``RuleBasedPIIProvider``.
    """

    class _ExplodingPIIProvider:
        async def detect(self, text: str) -> list[PIISpan]:
            raise RuntimeError("regex engine crashed")

    engine = SanitizerEngine(pii_provider=_ExplodingPIIProvider())

    with pytest.raises(SanitizerError) as exc:
        await engine.sanitize(
            "hello alice@example.com",
            source=FileSource(path="src/app.py"),
        )

    # Error message MUST identify the source.
    assert "file:src/app.py" in str(exc.value)
    # And __cause__ MUST preserve the original crash.
    assert isinstance(exc.value.__cause__, RuntimeError)
    assert "regex engine crashed" in str(exc.value.__cause__)


@pytest.mark.asyncio
async def test_engine_without_matches_returns_text_verbatim():
    engine = SanitizerEngine()
    result = await engine.sanitize(
        "plain prose no secrets", source=FileSource(path="a.md")
    )
    assert result.text == "plain prose no secrets"
    assert result.entries == []
