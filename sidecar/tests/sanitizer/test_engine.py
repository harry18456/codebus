"""Tests for `SanitizerEngine.sanitize` — covers Requirements
"SanitizerEngine exposes pure `sanitize` interface",
"Placeholder format is `<REDACTED:kind#index>`",
and "Placeholder index scope is single sanitize call".
"""
from __future__ import annotations

import inspect
import re
from unittest.mock import patch

import pytest

from codebus_agent.sanitizer import (
    FileSource,
    MessageSource,
    SanitizerEngine,
    SanitizerError,
)
from codebus_agent.sanitizer.rules import RegexRule, Rule


_PLACEHOLDER_RE = re.compile(
    r"<REDACTED:(email|phone|id|secret|ip|internal-domain|jwt|private-key|credential|suspect)#\d+>"
)


def test_engine_pass1_replaces_email_and_returns_audit_entries():
    engine = SanitizerEngine()
    result = engine.sanitize(
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


def test_placeholder_format_matches_redacted_kind_index():
    engine = SanitizerEngine()
    result = engine.sanitize(
        "phone: 0912-345-678; email: bob@example.com; ip: 10.0.0.1",
        source=FileSource(path="x.txt"),
    )
    placeholders = _PLACEHOLDER_RE.findall(result.text)
    assert set(placeholders) == {"phone", "email", "ip"}


def test_same_value_same_placeholder_within_call():
    engine = SanitizerEngine()
    result = engine.sanitize(
        "a: alice@example.com, b: alice@example.com",
        source=FileSource(path="src/a.py"),
    )
    # Both occurrences MUST map to the same placeholder string.
    assert result.text.count("<REDACTED:email#1>") == 2
    # And the audit list MUST contain exactly one entry for that value.
    emails = [e for e in result.entries if e.kind == "email"]
    assert len(emails) == 1


def test_placeholder_index_resets_across_calls():
    engine = SanitizerEngine()
    r1 = engine.sanitize(
        "a: alice@example.com",
        source=FileSource(path="src/a.py"),
    )
    r2 = engine.sanitize(
        "b: bob@example.com",
        source=FileSource(path="src/b.py"),
    )
    assert "<REDACTED:email#1>" in r1.text
    assert "<REDACTED:email#1>" in r2.text
    # Each call's first placeholder MUST be #1.
    assert r1.entries[0].placeholder_index == 1
    assert r2.entries[0].placeholder_index == 1


def test_engine_source_string_message_prefix():
    engine = SanitizerEngine()
    result = engine.sanitize(
        "user said alice@example.com",
        source=MessageSource(message_id="chat_req_abc"),
    )
    assert result.entries[0].source == "message:chat_req_abc"


def test_engine_no_reverse_mapping_exposed():
    """Ensure the engine exposes no method that returns pre-sanitize values."""
    engine = SanitizerEngine()
    # Trigger a sanitize to populate any hypothetical internal state.
    engine.sanitize("alice@example.com", source=FileSource(path="x.txt"))

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


def test_engine_fail_closed_raises_sanitizer_error():
    """When a rule raises, the engine MUST raise SanitizerError chained to
    the original exception (Decision: Fail-closed 失敗處理)."""

    class ExplodingRule:
        rule_id = "boom_v1"
        kind = "email"

        def find(self, text: str):
            raise RuntimeError("regex engine crashed")

    engine = SanitizerEngine(rules=[ExplodingRule()])

    with pytest.raises(SanitizerError) as exc:
        engine.sanitize(
            "hello alice@example.com",
            source=FileSource(path="src/app.py"),
        )

    # Error message MUST identify the source.
    assert "file:src/app.py" in str(exc.value)
    # And __cause__ MUST preserve the original crash.
    assert isinstance(exc.value.__cause__, RuntimeError)
    assert "regex engine crashed" in str(exc.value.__cause__)


def test_engine_without_matches_returns_text_verbatim():
    engine = SanitizerEngine()
    result = engine.sanitize("plain prose no secrets", source=FileSource(path="a.md"))
    assert result.text == "plain prose no secrets"
    assert result.entries == []
