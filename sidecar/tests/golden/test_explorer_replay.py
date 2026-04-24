"""Golden replay harness — pins Explorer output against tests/golden/demo-synthetic/expected.json.

Backs SHALL clauses in
openspec/changes/explorer-judge-golden/specs/explorer-golden/spec.md
  Requirement: Golden fixture pins expected stations, stopped_reason, step_count, and prompt versions
  Requirement: Golden replay harness runs under pytest and fails on drift

The replay drives `run_explorer` with scripted `MockProvider`s (both
reasoning and judge) so the outcome is deterministic. Station set
equality ignores `relevance`, `why`, and `depends_on` (only `(path,
role)` pairs are load-bearing). Prompt versions are pinned via
`expected.json`; a live mismatch against `JUDGE_PROMPT_VERSION` or
`EXPLORER_PROMPT_VERSION` forces the implementer to re-baseline.

Fixture path resolution is `Path(__file__)`-based so the harness runs
from any cwd.
"""
from __future__ import annotations

import json
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

from codebus_agent.agent.explorer import run_explorer
from codebus_agent.agent.judge import LLMJudge
from codebus_agent.agent.prompts.explorer import EXPLORER_PROMPT_VERSION
from codebus_agent.agent.prompts.judge import JUDGE_PROMPT_VERSION
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.types import (
    ExplorerAction,
    ExplorerResult,
    ExplorerState,
    JudgeVerdict,
    Station,
    ToolCall,
)


_RULES_VERSION = "2026-04-20-1"
_ALLOWED_STOPPED_REASONS = frozenset(
    {"budget_exhausted", "queue_empty", "cancelled"}
)


# ------------------------- fixture path helpers ----------------------------


def _golden_root() -> Path:
    """Resolve `tests/golden/demo-synthetic/` via `Path(__file__)`, not cwd.

    Layout (see spec design "Fixture path 解析脆弱" mitigation):
        <repo_root>/sidecar/tests/golden/test_explorer_replay.py  ← __file__
                    ^- parents[0] = tests/golden
                    ^- parents[1] = tests
                    ^- parents[2] = sidecar
                    ^- parents[3] = <repo_root>

    So `parents[3] / "tests" / "golden" / "demo-synthetic"` lands on
    the cross-repo fixture root regardless of where pytest was invoked.
    """
    return (
        Path(__file__).resolve().parents[3]
        / "tests"
        / "golden"
        / "demo-synthetic"
    )


def _expected_path() -> Path:
    return _golden_root() / "expected.json"


def _load_expected() -> dict[str, Any]:
    with _expected_path().open("r", encoding="utf-8") as fh:
        return json.load(fh)


# ------------------------- provider + judge wiring --------------------------


def _make_factory(
    *, role: ProviderRole, default_module: str, script: MockScript
) -> Callable[[Path], TrackedProvider]:
    """Mirror of `sidecar/tests/agent/conftest.py::_make_factory`.

    Duplicated inline because conftest.py scope is per-directory and this
    module lives in `tests/golden/`, not `tests/agent/`. Keeping the
    factory local means the golden test has zero cross-directory
    fixture coupling.
    """
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


class _EchoTools:
    """Tool impl where every tool name maps to a no-op echo coroutine.

    The replay's Explorer actions carry `{"path": "src/X.py"}` args so
    `_update_state` can lift the path into the newly appended Station.
    The actual echo output does not matter because Judge verdicts are
    scripted — tools are only called for their side-effect of producing
    non-empty ToolResult.raw (which gates `_update_state`'s follow-imports
    enqueue, but we set should_follow_imports=False so the branch stays
    cold anyway).
    """

    def __init__(self) -> None:
        self.calls: list[tuple[str, dict]] = []

    async def echo(self, **kwargs: Any) -> str:
        self.calls.append(("echo", dict(kwargs)))
        return f"echo:path={kwargs.get('path', '')}"


class _NoopCoverage:
    """CoverageChecker stub — P0 recursion is dormant; we just satisfy the Protocol."""

    async def check(self, state: Any) -> list[Any]:
        return []


async def _run_golden(
    *,
    workspace_dir: Path,
    reasoning_actions: list[ExplorerAction],
    judge_verdicts: list[JudgeVerdict],
    budget_steps: int,
) -> ExplorerResult:
    """Drive `run_explorer` with the pinned MockScripts and return the result."""
    reasoning_script = MockScript()
    for a in reasoning_actions:
        reasoning_script.push(a)
    judge_script = MockScript()
    for v in judge_verdicts:
        judge_script.push(v)

    reasoning_factory = _make_factory(
        role=ProviderRole.REASONING,
        default_module="reasoning",
        script=reasoning_script,
    )
    judge_factory = _make_factory(
        role=ProviderRole.JUDGE,
        default_module="judge",
        script=judge_script,
    )

    reasoning_provider = reasoning_factory(workspace_dir)
    judge = LLMJudge(judge_factory, workspace_dir)
    logger = ReasoningLogger(workspace_dir / "reasoning_log.jsonl")

    state = ExplorerState(
        task="golden replay — pin explorer behaviour under scripted MockProvider",
        budget_steps_left=budget_steps,
        budget_tokens_left=10_000,
    )
    return await run_explorer(
        state=state,
        provider=reasoning_provider,
        tools=_EchoTools(),
        judge=judge,
        coverage=_NoopCoverage(),
        logger=logger,
    )


# ------------------------- MockScript fixture ------------------------------


_GOLDEN_BUDGET_STEPS = 3


def _golden_reasoning_actions() -> list[ExplorerAction]:
    """Three scripted Think outputs, each probing a distinct path.

    Aligns with `_MIN_STATIONS_FOR_CONVERGENCE = 3` and the pinned budget:
    each iteration produces exactly one tool call whose `path` argument
    seeds the Station that `_update_state` appends when the paired
    JudgeVerdict carries `should_add_station=True`.
    """
    return [
        ExplorerAction(
            thought=f"probe {p}",
            tool_calls=[
                ToolCall(id=f"tc_{i}", name="echo", arguments={"path": p})
            ],
            stop=False,
        )
        for i, p in enumerate(("src/a.py", "src/b.py", "src/c.py"))
    ]


def _golden_judge_verdicts() -> list[JudgeVerdict]:
    """Three scripted verdicts — every one seeds a Station, none follows imports.

    follow_imports=False keeps `pending_queue` empty so the loop's stop
    reason is determined purely by budget exhaustion (deterministic).
    """
    return [
        JudgeVerdict(
            relevance=0.8,
            should_follow_imports=False,
            should_add_station=True,
            reason="架構切片 pinned by golden fixture",
        )
        for _ in range(3)
    ]


def _expected_stations_set() -> set[tuple[str, str]]:
    return {
        ("src/a.py", "explorer-p0"),
        ("src/b.py", "explorer-p0"),
        ("src/c.py", "explorer-p0"),
    }


# ------------------------- Section 6 RED -----------------------------------


def test_expected_json_has_five_load_bearing_fields() -> None:
    """`expected.json` MUST carry exactly the five baseline keys.

    Spec scenario `expected.json carries all five load-bearing fields`.
    """
    expected_keys = {
        "stations",
        "stopped_reason",
        "step_count",
        "judge_prompt_version",
        "explorer_prompt_version",
    }
    data = _load_expected()
    assert set(data.keys()) == expected_keys, (
        f"expected.json top-level keys mismatch: got {set(data.keys())!r}, "
        f"want {expected_keys!r}"
    )


def test_expected_json_station_shape() -> None:
    """Every station entry MUST carry a `path` string and a `role` string."""
    data = _load_expected()
    assert isinstance(data["stations"], list), (
        f"expected.json.stations must be a list; got {type(data['stations']).__name__}"
    )
    for i, st in enumerate(data["stations"]):
        assert isinstance(st, dict), f"station[{i}] must be a dict"
        assert "path" in st and isinstance(st["path"], str), (
            f"station[{i}] missing str `path`"
        )
        assert "role" in st and isinstance(st["role"], str), (
            f"station[{i}] missing str `role`"
        )


def test_expected_json_stopped_reason_allowed_value() -> None:
    """`stopped_reason` MUST be one of the three allowed values."""
    data = _load_expected()
    assert data["stopped_reason"] in _ALLOWED_STOPPED_REASONS, (
        f"stopped_reason={data['stopped_reason']!r} must be one of "
        f"{sorted(_ALLOWED_STOPPED_REASONS)!r}"
    )


# ------------------------- Section 7 GREEN — baseline replay ----------------


async def test_golden_replay_matches_baseline(tmp_path: Path) -> None:
    """Scripted replay MUST match every pinned field in `expected.json`.

    Spec Requirement `Golden replay harness runs under pytest and fails on drift`.
    Asserts: (a) station (path, role) set equality, (b) stopped_reason
    equality, (c) step_count equality, (d) reasoning_log line count ==
    step_count, plus (e) prompt_version drift guard (Section 9 scope).
    """
    expected = _load_expected()

    result = await _run_golden(
        workspace_dir=tmp_path,
        reasoning_actions=_golden_reasoning_actions(),
        judge_verdicts=_golden_judge_verdicts(),
        budget_steps=_GOLDEN_BUDGET_STEPS,
    )

    produced_stations = {(s.path, s.role) for s in result.stations}
    pinned_stations = {(s["path"], s["role"]) for s in expected["stations"]}

    missing = pinned_stations - produced_stations
    extra = produced_stations - pinned_stations
    assert not missing and not extra, (
        "station (path, role) set drift:\n"
        f"  missing (pinned but not produced): {sorted(missing)!r}\n"
        f"  extra   (produced but not pinned): {sorted(extra)!r}"
    )

    assert result.stopped_reason == expected["stopped_reason"], (
        f"stopped_reason drift: produced={result.stopped_reason!r}, "
        f"pinned={expected['stopped_reason']!r}"
    )

    log_path = tmp_path / "reasoning_log.jsonl"
    log_lines = log_path.read_text(encoding="utf-8").splitlines()
    assert len(log_lines) == expected["step_count"], (
        f"reasoning_log.jsonl line count {len(log_lines)} != pinned "
        f"step_count {expected['step_count']}"
    )

    # Section 9.1 drift guard
    assert expected["judge_prompt_version"] == JUDGE_PROMPT_VERSION, (
        f"re-baseline required: JUDGE_PROMPT_VERSION drifted from pinned "
        f"baseline (pinned={expected['judge_prompt_version']!r}, "
        f"live={JUDGE_PROMPT_VERSION!r}). Re-run the harness and update "
        f"`tests/golden/demo-synthetic/expected.json` in the SAME commit "
        f"that bumped the version."
    )
    assert expected["explorer_prompt_version"] == EXPLORER_PROMPT_VERSION, (
        f"re-baseline required: EXPLORER_PROMPT_VERSION drifted from "
        f"pinned baseline (pinned={expected['explorer_prompt_version']!r}, "
        f"live={EXPLORER_PROMPT_VERSION!r})."
    )


# ------------------------- Section 8 RED — drift scenarios -----------------


async def test_station_set_drift_fails_with_named_diff(tmp_path: Path) -> None:
    """Producing an extra station MUST fail with the diff surfaced in the error.

    Uses `should_follow_imports=True` so `pending_queue` fills (gating
    the `queue_empty` stop branch) and the loop runs all 4 iterations
    under budget=4. The fourth iteration appends `src/extra.py`, making
    the produced station set drift from the pinned three-path baseline.
    """
    extra_actions = _golden_reasoning_actions() + [
        ExplorerAction(
            thought="probe extra",
            tool_calls=[
                ToolCall(id="tc_extra", name="echo", arguments={"path": "src/extra.py"})
            ],
            stop=False,
        )
    ]
    drift_verdict_kwargs = dict(
        relevance=0.8,
        should_follow_imports=True,  # keep pending_queue non-empty
        should_add_station=True,
        reason="golden drift probe",
    )
    extra_verdicts = [JudgeVerdict(**drift_verdict_kwargs) for _ in range(4)]

    result = await _run_golden(
        workspace_dir=tmp_path,
        reasoning_actions=extra_actions,
        judge_verdicts=extra_verdicts,
        budget_steps=_GOLDEN_BUDGET_STEPS + 1,
    )

    produced_stations = {(s.path, s.role) for s in result.stations}
    pinned_stations = _expected_stations_set()
    missing = pinned_stations - produced_stations
    extra = produced_stations - pinned_stations

    assert ("src/extra.py", "explorer-p0") in extra, (
        "sanity: the drift probe's extra station should appear in the "
        f"produced set, got produced={produced_stations!r}"
    )
    with pytest.raises(AssertionError, match="src/extra.py"):
        assert not missing and not extra, (
            "station (path, role) set drift:\n"
            f"  missing (pinned but not produced): {sorted(missing)!r}\n"
            f"  extra   (produced but not pinned): {sorted(extra)!r}"
        )


def test_prompt_version_drift_fails_with_rebaseline_hint(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Changing live JUDGE_PROMPT_VERSION MUST fail the drift guard with re-baseline hint.

    Spec scenario `Prompt version drift fails the harness with re-baseline hint`.
    We simulate a drift by monkeypatching the module attribute and then
    running the same equality check the harness body uses. Module
    reloading is deliberately avoided — it would reset the patch and
    defeat the simulation; reading `judge_prompts_module.JUDGE_PROMPT_VERSION`
    after `setattr` gives us the patched value directly.
    """
    import codebus_agent.agent.prompts.judge as judge_prompts_module

    monkeypatch.setattr(
        judge_prompts_module, "JUDGE_PROMPT_VERSION", "9999-99-99-99"
    )
    patched_live = judge_prompts_module.JUDGE_PROMPT_VERSION
    assert patched_live == "9999-99-99-99", (
        "sanity: monkeypatch should have replaced the module attribute"
    )

    expected = _load_expected()
    with pytest.raises(AssertionError, match="re-baseline"):
        assert expected["judge_prompt_version"] == patched_live, (
            f"re-baseline required: JUDGE_PROMPT_VERSION drifted from pinned "
            f"baseline (pinned={expected['judge_prompt_version']!r}, "
            f"live={patched_live!r})."
        )


async def test_reasoning_log_line_count_mismatch_fails(tmp_path: Path) -> None:
    """Over-counting iterations MUST flip step_count comparison red.

    Uses `should_add_station=False` on every verdict so `stations` never
    reaches `_MIN_STATIONS_FOR_CONVERGENCE=3` — that gates the
    `queue_empty` branch so the loop's stop reason is purely
    `budget_exhausted`, letting us drive iteration count via
    `budget_steps` alone. `budget_steps=4` with 4 scripted actions
    produces 4 reasoning_log lines, mismatching the pinned step_count=3.
    """
    over_actions = [
        ExplorerAction(
            thought=f"probe {i}",
            tool_calls=[
                ToolCall(id=f"tc_{i}", name="echo", arguments={"path": f"p{i}.py"})
            ],
            stop=False,
        )
        for i in range(4)
    ]
    over_verdicts = [
        JudgeVerdict(
            relevance=0.3,
            should_follow_imports=False,
            should_add_station=False,
            reason="step count drift probe",
        )
        for _ in range(4)
    ]
    await _run_golden(
        workspace_dir=tmp_path,
        reasoning_actions=over_actions,
        judge_verdicts=over_verdicts,
        budget_steps=_GOLDEN_BUDGET_STEPS + 1,
    )

    expected = _load_expected()
    log_path = tmp_path / "reasoning_log.jsonl"
    log_lines = log_path.read_text(encoding="utf-8").splitlines()

    assert len(log_lines) != expected["step_count"], (
        f"sanity check: over-budget run should produce {_GOLDEN_BUDGET_STEPS + 1} "
        f"log lines but got {len(log_lines)}"
    )
    with pytest.raises(AssertionError):
        assert len(log_lines) == expected["step_count"], (
            f"reasoning_log.jsonl line count {len(log_lines)} != pinned "
            f"step_count {expected['step_count']}"
        )
