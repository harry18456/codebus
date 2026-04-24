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


# ------------------------- Coverage-gap fixtures ---------------------------


@pytest.fixture
def mock_script_coverage() -> MockScript:
    """FIFO for pinned `CoverageResult` payloads consumed by MockProvider.

    `coverage-gap-recurse` Section 2 tests push `CoverageResult(...)` here
    and then drive `LLMCoverageChecker.check(state)` against the wrapped
    provider so the assertion surface is `provider.chat` was called once
    with `response_model=CoverageResult`.
    """
    return MockScript()


@pytest.fixture
def mock_coverage_provider_factory(
    mock_script_coverage: MockScript,
) -> Callable[[Path], TrackedProvider]:
    """Workspace-scoped `TrackedProvider` factory tagged `module="coverage"`.

    Shape-compatible with `app.state.llm_coverage_provider(ws)` so tests
    pin the same DI contract the `POST /explore` endpoint will wire up
    in Section 8. `role=ProviderRole.JUDGE` — Coverage rides the judge
    role per `agent-core.md §七` (low-temp structured output evaluator),
    distinct from the `module` tag which splits cost in
    `token_usage.jsonl`.
    """
    return _make_factory(
        role=ProviderRole.JUDGE,
        default_module="coverage",
        script=mock_script_coverage,
    )


@pytest.fixture
def captured_coverage_provider(
    mock_coverage_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> TrackedProvider:
    """Pre-materialized TrackedProvider for direct `LLMCoverageChecker` wiring."""
    return mock_coverage_provider_factory(workspace_dir)


@pytest.fixture
def scripted_coverage_checker():
    """Protocol-satisfying `CoverageChecker` spy that returns preset gaps.

    Mirrors P0's `_CountingCoverage` but richer: accepts either a static
    `gaps` list (returned every call) OR a `gap_queue` (list of gap
    lists, one consumed per call; falls back to `gaps` after the queue
    drains). The queue form lets recursion tests stage "first round
    reports gaps, subsequent rounds report none" without building a
    stateful mock inline.

    Each test calls `scripted_coverage_checker(gaps=[...])` or
    `scripted_coverage_checker(gap_queue=[[g1,g2], []])` to instantiate.
    """
    from codebus_agent.agent.types import Gap

    class _ScriptedCoverage:
        def __init__(
            self,
            gaps: list[Gap] | None = None,
            *,
            gap_queue: list[list[Gap]] | None = None,
        ) -> None:
            self.calls = 0
            self._static_gaps = list(gaps or [])
            # Copy each sub-list so tests mutating the returned list
            # don't leak into the scripted queue.
            self._queue: list[list[Gap]] = (
                [list(g) for g in gap_queue] if gap_queue is not None else []
            )

        async def check(self, state) -> list[Gap]:
            self.calls += 1
            if self._queue:
                return list(self._queue.pop(0))
            return list(self._static_gaps)

    return _ScriptedCoverage
