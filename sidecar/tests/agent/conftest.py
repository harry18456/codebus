"""Shared fixtures for agent-core tests.

Provides the minimum wiring `test_judge.py` / `test_explorer_loop.py`
need to exercise the ReAct loop end-to-end against an in-memory
MockProvider:

- ``workspace_dir``: throw-away workspace for JSONL writers
- ``mock_reasoning_provider_factory`` / ``mock_judge_provider_factory``:
  workspace-scoped ``TrackedProvider`` factories that mirror the shape
  `app.state.llm_reasoning_provider(ws)` / `llm_judge_provider(ws)`
  produce in `codebus_agent.api.__init__::_make_chat_provider_factory`,
  so tests pin the same contract the Explorer loop will consume in
  production wiring.

Per-role scripts (``mock_script_reasoning`` / ``mock_script_judge``)
stay separate because ``MockProvider`` rejects a pinned response whose
type doesn't match the current ``response_model`` — interleaving
``ExplorerAction`` (reasoning) and ``JudgeVerdict`` (judge) answers in
one queue would fail type validation mid-loop.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

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
    return tmp_path


def _make_factory(
    *, role: ProviderRole, default_module: str, script: MockScript
) -> Callable[[Path], TrackedProvider]:
    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        return TrackedProvider(
            MockProvider(script=script, role=role),
            tracker=UsageTracker(ws / "token_usage.jsonl"),
            logger=LLMCallLogger(ws / "llm_calls.jsonl"),
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(ws / "sanitize_audit.jsonl"),
            rules_version=_RULES_VERSION,
            default_module=default_module,
        )

    return _factory


@pytest.fixture
def mock_script_reasoning() -> MockScript:
    return MockScript()


@pytest.fixture
def mock_script_judge() -> MockScript:
    return MockScript()


@pytest.fixture
def mock_script(mock_script_reasoning: MockScript) -> MockScript:
    """Back-compat alias — older tests pushed to the single shared script.

    ``test_judge.py`` only pushes ``JudgeVerdict`` and ``test_explorer_loop.py``
    uses the per-role scripts explicitly, so either path works without
    crossing types.
    """
    return mock_script_reasoning


@pytest.fixture
def mock_reasoning_provider_factory(
    mock_script_reasoning: MockScript,
) -> Callable[[Path], TrackedProvider]:
    return _make_factory(
        role=ProviderRole.REASONING,
        default_module="reasoning",
        script=mock_script_reasoning,
    )


@pytest.fixture
def mock_judge_provider_factory(
    mock_script_judge: MockScript,
) -> Callable[[Path], TrackedProvider]:
    return _make_factory(
        role=ProviderRole.JUDGE,
        default_module="judge",
        script=mock_script_judge,
    )


@pytest.fixture
def mock_reasoning_provider(
    mock_reasoning_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> TrackedProvider:
    return mock_reasoning_provider_factory(workspace_dir)


@pytest.fixture
def mock_judge_provider(
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> TrackedProvider:
    return mock_judge_provider_factory(workspace_dir)
