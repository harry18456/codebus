"""Tests for engine+config allowlist integration — covers Requirement
"Allowlist hits still audited but not redacted".
"""
from __future__ import annotations

import pytest

from codebus_agent.sanitizer import (
    FileSource,
    PatternAllowlistEntry,
    SanitizerConfig,
    SanitizerEngine,
)


def _cfg(
    *,
    path_allowlist=None,
    filename_allowlist=None,
    pattern_allowlist=None,
) -> SanitizerConfig:
    return SanitizerConfig(
        rules_version="test-v1",
        path_allowlist=path_allowlist or [],
        filename_allowlist=filename_allowlist or [],
        pattern_allowlist=pattern_allowlist or [],
    )


@pytest.mark.asyncio
async def test_pattern_allowlist_hit_leaves_text_and_flags_extra():
    # Allowlist the `noreply@` form — still detected by the email rule,
    # but the engine must keep the original text and flag the audit entry.
    cfg = _cfg(
        pattern_allowlist=[
            PatternAllowlistEntry(pattern="^noreply@", reason="no-reply mailbox"),
        ],
    )
    engine = SanitizerEngine(config=cfg)

    text = "contact: noreply@example.com"
    result = await engine.sanitize(text, source=FileSource(path="src/app.py"))

    assert "noreply@example.com" in result.text
    assert "<REDACTED:" not in result.text
    assert len(result.entries) == 1
    assert result.entries[0].extra.get("allowlisted") is True


@pytest.mark.asyncio
async def test_pattern_allowlist_miss_redacts_normally():
    cfg = _cfg(
        pattern_allowlist=[
            PatternAllowlistEntry(pattern="^noreply@", reason="no-reply mailbox"),
        ],
    )
    engine = SanitizerEngine(config=cfg)

    # Non-matching email is not on the allowlist → redact as usual.
    result = await engine.sanitize(
        "contact: alice@example.com",
        source=FileSource(path="src/app.py"),
    )
    assert "alice@example.com" not in result.text
    assert "<REDACTED:email#1>" in result.text


@pytest.mark.asyncio
async def test_path_allowlist_glob_matches():
    cfg = _cfg(path_allowlist=["tests/fixtures/**"])
    engine = SanitizerEngine(config=cfg)

    text = "hit: alice@example.com"
    result = await engine.sanitize(
        text,
        source=FileSource(path="tests/fixtures/pii_sample.txt"),
    )

    # Path allowlist hit → original email stays, no REDACTED placeholder.
    assert "alice@example.com" in result.text
    assert "<REDACTED:" not in result.text
    assert len(result.entries) == 1
    assert result.entries[0].extra.get("allowlisted") is True


@pytest.mark.asyncio
async def test_path_allowlist_miss_still_redacts():
    cfg = _cfg(path_allowlist=["tests/fixtures/**"])
    engine = SanitizerEngine(config=cfg)

    # Source not under tests/fixtures → no allowlist hit.
    result = await engine.sanitize(
        "hit: alice@example.com",
        source=FileSource(path="src/app.py"),
    )
    assert "alice@example.com" not in result.text
    assert "<REDACTED:email#1>" in result.text
    assert result.entries[0].extra.get("allowlisted") is not True


@pytest.mark.asyncio
async def test_filename_allowlist():
    cfg = _cfg(filename_allowlist=[".env.example"])
    engine = SanitizerEngine(config=cfg)

    result = await engine.sanitize(
        "example_key=AKIAIOSFODNN7EXAMPLE",
        source=FileSource(path="some/dir/.env.example"),
    )
    # Filename allowlist hit → keep original, audit it with allowlisted flag.
    assert "AKIAIOSFODNN7EXAMPLE" in result.text
    assert "<REDACTED:" not in result.text
    assert all(e.extra.get("allowlisted") is True for e in result.entries)


@pytest.mark.asyncio
async def test_filename_allowlist_miss():
    cfg = _cfg(filename_allowlist=[".env.example"])
    engine = SanitizerEngine(config=cfg)

    result = await engine.sanitize(
        "key=AKIAIOSFODNN7EXAMPLE",
        source=FileSource(path="src/.env"),  # .env is not .env.example
    )
    assert "AKIAIOSFODNN7EXAMPLE" not in result.text
    assert "<REDACTED:secret#1>" in result.text
