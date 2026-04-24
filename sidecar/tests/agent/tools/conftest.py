"""Shared fixtures for FolderTools tests.

Exposes:
- ``temp_workspace``: tmp workspace with a handful of seed files (some
  carrying sanitize-sensitive content) + a ``.codebus/`` subdir so
  ``sanitize_audit.jsonl`` / ``tool_audit.jsonl`` have a landing place.
- ``sanitizer_for_tools``: a ``SanitizerEngine`` + ``SanitizerAuditLogger``
  pair wired to the workspace's ``.codebus/sanitize_audit.jsonl``.
- ``tool_context``: a fully-populated ``ToolContext`` bound to the
  workspace, sanitizer, and (optionally) a mock KB.
- ``mock_kb``: tiny KB stand-in exposing an ``async query(text, *, top_k)``
  coroutine returning scripted ``KBHit`` results. Counters expose call
  shape so tests can assert dispatch.
- ``explorer_state``: a minimal ``ExplorerState`` for mark_station.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import pytest

from datetime import datetime, timezone

from codebus_agent.kb.payload import KBHit, KBPayload
from codebus_agent.sanitizer import (
    SanitizerAuditLogger,
    SanitizerEngine,
)
from codebus_agent.sandbox import ToolContext


_FAKE_AWS_KEY = "AKIAIOSFODNN7EXAMPLE"  # canonical example from AWS docs, matches SECRET rule


@pytest.fixture
def temp_workspace(tmp_path: Path) -> Path:
    """Populate a minimal workspace — 3 py files + 1 md + 1 binary + subdir."""
    ws = tmp_path / "ws"
    ws.mkdir()
    (ws / ".codebus").mkdir()

    (ws / "app.py").write_text(
        'def entry():\n    """Main entry point for search keyword."""\n    pass\n',
        encoding="utf-8",
    )
    (ws / "secret.py").write_text(
        f'AWS_KEY = "{_FAKE_AWS_KEY}"\nentry_email = "alice@example.com"\n',
        encoding="utf-8",
    )
    (ws / "helper.py").write_text(
        "def helper():\n    # helper without the search keyword\n    return 1\n",
        encoding="utf-8",
    )
    (ws / "README.md").write_text("# Project\n\nSee `entry` in app.py.\n", encoding="utf-8")
    (ws / "image.png").write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00")
    sub = ws / "subdir"
    sub.mkdir()
    (sub / "nested.py").write_text("# nested entry helper\n", encoding="utf-8")
    return ws


@pytest.fixture
def sanitizer_for_tools(temp_workspace: Path) -> tuple[SanitizerEngine, SanitizerAuditLogger]:
    engine = SanitizerEngine()
    logger = SanitizerAuditLogger(temp_workspace / ".codebus" / "sanitize_audit.jsonl")
    return engine, logger


@pytest.fixture
def tool_context(
    temp_workspace: Path,
    sanitizer_for_tools: tuple[SanitizerEngine, SanitizerAuditLogger],
) -> ToolContext:
    engine, _audit = sanitizer_for_tools
    return ToolContext(
        workspace_root=temp_workspace,
        workspace_type="folder",
        workspace_id="ws-test",
        session_id="sess-test",
        sanitizer=engine,
    )


@pytest.fixture
def aws_key_literal() -> str:
    """The fake AWS key seeded in ``secret.py``; tests assert it is NEVER in tool output."""
    return _FAKE_AWS_KEY


@dataclass
class _MockKB:
    """Minimal KB stand-in — records query calls + returns scripted hits.

    Satisfies the structural shape `FolderTools.search` expects:
    ``async def query(text, *, top_k=8, ...) -> list[KBHit]``.
    """

    scripted_hits: list[KBHit] = field(default_factory=list)
    query_calls: list[tuple[str, dict[str, Any]]] = field(default_factory=list)

    async def query(
        self,
        text: str,
        *,
        top_k: int = 8,
        filter_path: str | None = None,
        filter_source_kind: list[str] | None = None,
    ) -> list[KBHit]:
        self.query_calls.append(
            (text, {"top_k": top_k, "filter_path": filter_path, "filter_source_kind": filter_source_kind})
        )
        return list(self.scripted_hits)


def _make_kb_hit(path: str, snippet: str, score: float) -> KBHit:
    return KBHit(
        point_id=f"pt-{path}",
        score=score,
        payload=KBPayload(
            source_kind="code",
            file_path=path,
            line_start=0,
            line_end=len(snippet.splitlines()) or 1,
            text=snippet,
            text_hash="0" * 64,
            added_by="scanner",
            chunk_index=0,
            chunk_total=1,
            created_at=datetime.now(timezone.utc),
        ),
    )


@pytest.fixture
def mock_kb_factory():
    """Return a factory so tests can build scripted KB instances."""

    def _factory(hits: list[tuple[str, str, float]]) -> _MockKB:
        return _MockKB(scripted_hits=[_make_kb_hit(p, s, sc) for p, s, sc in hits])

    return _factory


@pytest.fixture
def mock_kb(mock_kb_factory) -> _MockKB:
    """Default mock KB with one hit pointing at ``app.py``."""
    return mock_kb_factory([("app.py", "def entry():\n    pass\n", 0.82)])


@pytest.fixture
def explorer_state():
    from codebus_agent.agent.types import ExplorerState

    return ExplorerState(
        task="explore",
        budget_steps_left=10,
        budget_tokens_left=10_000,
    )
