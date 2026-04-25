"""Shared fixtures for Generator (Module 5) tests.

Mirrors the shape of ``sidecar/tests/agent/conftest.py``:

- ``workspace_dir``: fresh ``tmp_path`` for JSONL writers (`<ws>/.codebus/...`)
- ``mock_generate_provider_factory``: workspace-scoped TrackedProvider
  factory tagged ``module="generate"`` / ``role=ProviderRole.CHAT``;
  shape-compatible with what
  ``codebus_agent.api.__init__::_make_chat_provider_factory`` produces in
  production wiring (sans the inner ``OpenAIChatProvider`` — we use
  ``MockProvider`` so tests stay offline).
- ``mock_script_generate``: FIFO script the factory's MockProvider pops
  from on each ``provider.chat`` call.
- ``spy_emitter``: structural ``SSEEmitter`` recording every event for
  per-event assertions; mirrors ``test_explorer_loop_sse.py``.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


_RULES_VERSION = "2026-04-20-1"


@pytest.fixture
def workspace_dir(tmp_path: Path) -> Path:
    """Throw-away workspace root for the test."""
    return tmp_path


@pytest.fixture
def mock_script_generate() -> MockScript:
    """FIFO of pinned ``StationMarkdown`` (or other) responses."""
    return MockScript()


def _make_generate_factory(
    *, script: MockScript
) -> Callable[[Path], TrackedProvider]:
    """Build a TrackedProvider factory tagged ``module="generate"``.

    Path constants intentionally mirror production wiring so factory
    output writes to the same ``<ws>/.codebus/...`` locations as
    ``_make_chat_provider_factory`` does — this lets tests assert on
    real path geometry rather than a test-only convention.
    """

    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        audit_dir = ws / ".codebus"
        return TrackedProvider(
            MockProvider(script=script, role=ProviderRole.CHAT),
            tracker=UsageTracker(audit_dir / "token_usage.jsonl"),
            logger=LLMCallLogger(audit_dir / "llm_calls.jsonl"),
            role=ProviderRole.CHAT,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl"),
            rules_version=_RULES_VERSION,
            default_module="generate",
        )

    return _factory


@pytest.fixture
def mock_generate_provider_factory(
    mock_script_generate: MockScript,
) -> Callable[[Path], TrackedProvider]:
    return _make_generate_factory(script=mock_script_generate)


@pytest.fixture
def mock_generate_provider(
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> TrackedProvider:
    return mock_generate_provider_factory(workspace_dir)


class _SpyEmitter:
    """Records every ``emit`` call in order; satisfies ``SSEEmitter`` Protocol."""

    def __init__(self) -> None:
        self.events: list[dict[str, Any]] = []

    def emit(self, event: dict[str, Any]) -> None:
        self.events.append(dict(event))


@pytest.fixture
def spy_emitter() -> _SpyEmitter:
    return _SpyEmitter()
