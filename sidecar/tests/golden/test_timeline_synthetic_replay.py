"""Full-stack scripted golden replay against `tests/golden/timeline-storage-adapter-synthetic/`.

Backs SHALL clauses in
openspec/changes/golden-sample-baseline/specs/explorer-golden/spec.md
  Requirement: Timeline-storage-adapter-synthetic fixture pins ideal-route stations
  Requirement: Full-stack golden replay wires Coverage, token probe, and SSE emitter

The harness drives `run_explorer` end-to-end with the entire Module 4
stack wired in production-shape: scripted reasoning + judge MockProviders,
an `LLMCoverageChecker` over a scripted coverage MockProvider, an
`AggregatedTokenProbe` aggregating all three TrackedProvider instances,
and a spy `SSEEmitter` that captures every emitted event for assertion.

Five scripted iterations visit the five `must_have` paths from
`ideal-route.json` exactly once each → produced station path set equals
must_have, recall = 1.0, noise = 0.0, composite_score = 1.0 under
default D-006 weights.

The replay deliberately stays scripted (no live LLM); D-006's
`[ ] 打磨期: 真 LLM snapshot` is a follow-up change.
"""
from __future__ import annotations

import json
from collections.abc import AsyncIterator, Callable
from pathlib import Path
from typing import Any

import pytest

from codebus_agent.agent.budget import AggregatedTokenProbe
from codebus_agent.agent.coverage import LLMCoverageChecker
from codebus_agent.agent.explorer import run_explorer
from codebus_agent.agent.judge import LLMJudge
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.types import (
    CoverageResult,
    ExplorerAction,
    ExplorerResult,
    ExplorerState,
    JudgeVerdict,
    ToolCall,
)
from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

from .scoring import (
    IdealRoute,
    composite_score,
    station_noise,
    station_recall,
)


# ============================== constants =================================


_RULES_VERSION = "2026-04-20-1"
_BUDGET_STEPS = 5
_BUDGET_TOKENS = 10_000


# ============================== helpers (task 7.x) ========================


def _timeline_synthetic_root() -> Path:
    """Resolve `tests/golden/timeline-storage-adapter-synthetic/` via `__file__`.

    Mirrors `test_explorer_replay.py::_golden_root` but points at the new
    fixture. Layout:
        <repo_root>/sidecar/tests/golden/test_timeline_synthetic_replay.py
                    ^- parents[3] = <repo_root>
    """
    return (
        Path(__file__).resolve().parents[3]
        / "tests"
        / "golden"
        / "timeline-storage-adapter-synthetic"
    )


def _load_ideal_route() -> IdealRoute:
    """Load the fixture's `ideal-route.json` into the IdealRoute schema."""
    raw = (_timeline_synthetic_root() / "ideal-route.json").read_text(
        encoding="utf-8"
    )
    return IdealRoute.model_validate_json(raw)


def _make_factory(
    *, role: ProviderRole, default_module: str, script: MockScript
) -> Callable[[Path], TrackedProvider]:
    """Workspace-scoped `TrackedProvider` factory.

    Inline mirror of `sidecar/tests/agent/conftest.py::_make_factory` —
    `tests/golden/` lives in a sibling directory, so per-directory
    conftest fixtures are out of reach. Keeping the factory local mirrors
    `test_explorer_replay.py` and gives the golden test zero
    cross-directory fixture coupling.
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


class _SpyEmitter:
    """Structurally satisfies `SSEEmitter`; appends every event to a list.

    `@runtime_checkable SSEEmitter` only checks for the `emit(event)`
    method — no inheritance required. Tests read `.events` post-run to
    assert SSE shape + ordering.
    """

    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


class _EchoTools:
    """Tool implementation where `echo` returns `f"echo:path={path}"`.

    Mirrors `test_explorer_replay.py::_EchoTools` — the produced station
    path lifts from `call.arguments["path"]` via `_update_state`'s
    seed_path branch, so the echo body itself is irrelevant. We accept
    arbitrary kwargs so future scripted actions can pass extra metadata
    without breaking the dispatcher.
    """

    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []

    async def echo(self, **kwargs: Any) -> str:
        self.calls.append(("echo", dict(kwargs)))
        return f"echo:path={kwargs.get('path', '')}"


async def _run_full_stack_replay(
    workspace_dir: Path, must_have_paths: list[str]
) -> tuple[ExplorerResult, list[dict]]:
    """Drive `run_explorer` against the synthetic Timeline fixture.

    Builds the full Module 4 production-shape stack:
    - 3× scripted MockProvider (reasoning / judge / coverage)
    - LLMCoverageChecker over the coverage factory
    - AggregatedTokenProbe across all three TrackedProviders
    - _SpyEmitter wired through every collaborator's `set_emitter`

    Returns `(ExplorerResult, captured_spy_events)`.
    """
    reasoning_script = MockScript()
    for i, path in enumerate(must_have_paths):
        reasoning_script.push(
            ExplorerAction(
                thought=f"probe {path}",
                tool_calls=[
                    ToolCall(
                        id=f"tc_{i}",
                        name="echo",
                        arguments={"path": path},
                    )
                ],
                stop=False,
            )
        )

    judge_script = MockScript()
    for path in must_have_paths:
        judge_script.push(
            JudgeVerdict(
                relevance=0.8,
                # `should_follow_imports=True` keeps `pending_queue` non-empty
                # across iterations: `_MIN_STATIONS_FOR_CONVERGENCE=3` would
                # otherwise fire `queue_empty` at iter 4 once three stations
                # accumulate, cutting the run short of all five must_have
                # paths. Each verdict appends `r.tool_name="echo"` to the
                # queue (stale but non-empty) so the loop survives until
                # `budget_steps=5` exhausts naturally.
                should_follow_imports=True,
                should_add_station=True,
                reason=f"timeline synthetic must_have hit: {path}",
            )
        )

    coverage_script = MockScript()
    coverage_script.push(CoverageResult(gaps=[]))

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
    coverage_factory = _make_factory(
        role=ProviderRole.JUDGE,
        default_module="coverage",
        script=coverage_script,
    )

    reasoning_provider = reasoning_factory(workspace_dir)
    judge = LLMJudge(judge_factory, workspace_dir)
    coverage = LLMCoverageChecker(coverage_factory, workspace_dir)

    probe = AggregatedTokenProbe(
        [reasoning_provider, judge.provider, coverage.provider]
    )

    spy = _SpyEmitter()
    reasoning_provider.set_emitter(spy)
    judge.set_emitter(spy)
    coverage.set_emitter(spy)

    logger = ReasoningLogger(workspace_dir / "reasoning_log.jsonl")

    state = ExplorerState(
        task="在 Timeline 專案新增 Google Drive Adapter 同步功能",
        budget_steps_left=_BUDGET_STEPS,
        budget_tokens_left=_BUDGET_TOKENS,
    )

    result = await run_explorer(
        state=state,
        provider=reasoning_provider,
        tools=_EchoTools(),
        judge=judge,
        coverage=coverage,
        logger=logger,
        emitter=spy,
        token_probe=probe,
    )
    return result, list(spy.events)


# ============================== shared replay fixture ====================


@pytest.fixture
async def replay_run(tmp_path: Path) -> tuple[ExplorerResult, list[dict]]:
    """One full-stack replay shared across all section-6 tests.

    Per-test `tmp_path` scope keeps audit JSONL writes isolated; the run
    is otherwise deterministic (scripted MockProvider) so each test sees
    bit-identical `(result, spy_events)`.
    """
    ideal = _load_ideal_route()
    return await _run_full_stack_replay(tmp_path, list(ideal.must_have))


# ============================ Section 5 RED — fixture schema =============


def test_fixture_provides_exactly_five_must_have_entries() -> None:
    """Spec scenario `Fixture provides exactly five must_have entries`."""
    ideal = _load_ideal_route()
    assert len(ideal.must_have) == 5, (
        f"must_have should hold exactly 5 paths; got {len(ideal.must_have)}: "
        f"{ideal.must_have!r}"
    )
    for path in ideal.must_have:
        assert path.startswith("workspace/app/"), (
            f"must_have path {path!r} should be relative under workspace/app/"
        )


def test_fixture_nice_to_have_captures_secondary_consumers() -> None:
    """Spec scenario `Fixture nice_to_have list captures secondary consumers`."""
    ideal = _load_ideal_route()
    assert len(ideal.nice_to_have) >= 2, (
        f"nice_to_have should hold at least 2 secondary-consumer paths; "
        f"got {len(ideal.nice_to_have)}: {ideal.nice_to_have!r}"
    )
    overlap = set(ideal.nice_to_have) & set(ideal.must_have)
    assert overlap == set(), (
        f"nice_to_have must NOT overlap must_have; overlap={sorted(overlap)!r}"
    )


def test_fixture_noise_paths_captures_off_route_files() -> None:
    """Spec scenario `Fixture noise_paths list captures off-route files`."""
    ideal = _load_ideal_route()
    assert len(ideal.noise_paths) >= 1, (
        f"noise_paths should hold at least 1 off-route path; "
        f"got {len(ideal.noise_paths)}: {ideal.noise_paths!r}"
    )
    must_overlap = set(ideal.noise_paths) & set(ideal.must_have)
    nice_overlap = set(ideal.noise_paths) & set(ideal.nice_to_have)
    assert must_overlap == set(), (
        f"noise_paths must NOT overlap must_have; overlap={sorted(must_overlap)!r}"
    )
    assert nice_overlap == set(), (
        f"noise_paths must NOT overlap nice_to_have; overlap={sorted(nice_overlap)!r}"
    )


def test_all_workspace_files_appear_in_ideal_route_schema() -> None:
    """Spec scenario `All workspace files appear in the ideal route schema`.

    Walk the fixture's `workspace/` tree and assert every file appears
    in exactly one of `must_have ∪ nice_to_have ∪ noise_paths` — no
    orphans, no duplicates. Drift guard: adding a file to `workspace/`
    without classifying it in `ideal-route.json` flips the test red.
    """
    ideal = _load_ideal_route()
    fixture_root = _timeline_synthetic_root()
    workspace_root = fixture_root / "workspace"

    walked: set[str] = set()
    for sub in workspace_root.rglob("*"):
        if not sub.is_file():
            continue
        rel = sub.relative_to(fixture_root).as_posix()
        walked.add(rel)

    classified = set(ideal.must_have) | set(ideal.nice_to_have) | set(ideal.noise_paths)
    orphans = walked - classified
    phantoms = classified - walked
    assert orphans == set(), (
        f"workspace/ files missing from ideal-route.json (orphans): "
        f"{sorted(orphans)!r}"
    )
    assert phantoms == set(), (
        f"ideal-route.json paths missing on disk (phantoms): "
        f"{sorted(phantoms)!r}"
    )

    # No duplicates across the three lists.
    counts: dict[str, int] = {}
    for p in (
        list(ideal.must_have)
        + list(ideal.nice_to_have)
        + list(ideal.noise_paths)
    ):
        counts[p] = counts.get(p, 0) + 1
    duplicates = {p for p, c in counts.items() if c > 1}
    assert duplicates == set(), (
        f"paths appear in more than one classification list: "
        f"{sorted(duplicates)!r}"
    )


# ============================ Section 6 RED — full-stack replay ==========


async def test_replay_achieves_recall_one_on_synthetic_timeline_fixture(
    replay_run: tuple[ExplorerResult, list[dict]],
) -> None:
    """Spec scenario `Replay achieves recall 1.0 on the synthetic Timeline fixture`."""
    result, _events = replay_run
    ideal = _load_ideal_route()
    produced = {st.path for st in result.stations}
    must_have = set(ideal.must_have)
    assert produced == must_have, (
        f"produced station path set must equal must_have:\n"
        f"  produced={sorted(produced)!r}\n"
        f"  must_have={sorted(must_have)!r}"
    )
    assert station_recall(produced, must_have) == 1.0


async def test_replay_reports_zero_noise_on_clean_run(
    replay_run: tuple[ExplorerResult, list[dict]],
) -> None:
    """Spec scenario `Replay reports zero noise on a clean run`."""
    result, _events = replay_run
    ideal = _load_ideal_route()
    produced = {st.path for st in result.stations}
    assert station_noise(
        produced, set(ideal.must_have), set(ideal.nice_to_have)
    ) == 0.0


async def test_composite_score_crosses_threshold(
    replay_run: tuple[ExplorerResult, list[dict]],
) -> None:
    """Spec scenario `Composite score crosses 0.9 threshold under default weights`.

    `depth=1.0` is a placeholder pending Module 5 dep-chain landing per
    `context-compression-token-budget` design Decision 6. Under default
    D-006 weights and recall=1.0 / noise=0.0 / depth=1.0 the score
    equals exactly 1.0; the assertion is `>= 0.9` to leave headroom for
    future depth realism.
    """
    result, _events = replay_run
    ideal = _load_ideal_route()
    produced = {st.path for st in result.stations}
    recall = station_recall(produced, set(ideal.must_have))
    noise = station_noise(
        produced, set(ideal.must_have), set(ideal.nice_to_have)
    )
    score = composite_score(recall, noise, depth=1.0)
    assert score >= 0.9, (
        f"composite score below threshold: recall={recall}, noise={noise}, "
        f"score={score}"
    )


async def test_coverage_round_emits_coverage_gaps_event_under_spy_emitter(
    replay_run: tuple[ExplorerResult, list[dict]],
) -> None:
    """Spec scenario `Coverage round emits coverage_gaps event under spy emitter`."""
    _result, events = replay_run
    coverage_events = [e for e in events if e.get("type") == "coverage_gaps"]
    assert len(coverage_events) >= 1, (
        f"coverage_gaps event MUST be emitted at least once; "
        f"saw event types: {sorted({e.get('type') for e in events})!r}"
    )
    first = coverage_events[0]
    assert first["will_recurse"] is False, (
        f"coverage_gaps.will_recurse must be False under empty-gaps script; "
        f"got {first!r}"
    )
    assert first["skip_reason"] == "no_gaps", (
        f"coverage_gaps.skip_reason must be 'no_gaps' when scripted "
        f"CoverageResult.gaps is empty; got {first!r}"
    )


async def test_five_step_run_emits_one_steps_warning_at_eighty_percent_boundary(
    replay_run: tuple[ExplorerResult, list[dict]],
) -> None:
    """Spec scenario `Five-step run emits exactly one steps budget_warning at the 80% boundary`.

    Production `_maybe_emit_budget_warning` uses `>=` against
    `_BUDGET_WARNING_PCT = 0.8`; with `budget_steps=5` the natural
    boundary is `consumed=4 → 4/5=0.8 → fires`. Token consumption
    across 5 scripted iterations stays well under 8_000 (80% of
    `budget_tokens=10_000`) so `kind="tokens"` MUST NOT fire. See
    `sidecar/tests/agent/test_budget_warning_event.py::
    test_first_iteration_crossing_step_threshold_emits_warning` for
    the unit-level pinned behaviour.
    """
    result, events = replay_run
    warnings = [e for e in events if e.get("type") == "budget_warning"]
    steps_warnings = [w for w in warnings if w["kind"] == "steps"]
    tokens_warnings = [w for w in warnings if w["kind"] == "tokens"]

    assert len(steps_warnings) == 1, (
        f"exactly one kind=steps budget_warning expected at the 4/5=0.8 "
        f"boundary; got {len(steps_warnings)}: {steps_warnings!r}"
    )
    ev = steps_warnings[0]
    assert ev["current"] == 4, f"current must be 4; got {ev!r}"
    assert ev["budget"] == _BUDGET_STEPS, f"budget must be 5; got {ev!r}"
    assert ev["pct"] == 0.8, f"pct must be 0.8; got {ev!r}"

    assert len(tokens_warnings) == 0, (
        f"no kind=tokens budget_warning expected (token usage stays under "
        f"80% of budget_tokens={_BUDGET_TOKENS}); got {tokens_warnings!r}"
    )

    assert result.stopped_reason == "budget_exhausted", (
        f"stopped_reason must equal 'budget_exhausted' when the loop drains "
        f"all five iterations; got {result.stopped_reason!r}"
    )


async def test_usage_delta_events_carry_session_total_tokens_additive_field(
    replay_run: tuple[ExplorerResult, list[dict]],
) -> None:
    """Spec scenario `usage_delta events carry session_total_tokens additive field`."""
    _result, events = replay_run
    usage_deltas = [e for e in events if e.get("type") == "usage_delta"]
    assert len(usage_deltas) >= 1, (
        "expected at least one usage_delta event across the 5-iter replay; "
        f"saw event types: {sorted({e.get('type') for e in events})!r}"
    )
    for ev in usage_deltas:
        assert "session_total_tokens" in ev, (
            f"usage_delta event missing session_total_tokens field: {ev!r}"
        )
        assert isinstance(ev["session_total_tokens"], int), (
            f"session_total_tokens must be int; got "
            f"{type(ev['session_total_tokens']).__name__}: {ev!r}"
        )
        assert ev["session_total_tokens"] >= 0, (
            f"session_total_tokens must be non-negative; got {ev!r}"
        )
