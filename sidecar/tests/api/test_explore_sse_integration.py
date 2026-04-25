"""End-to-end SSE stream test for POST /explore (agent-sse-wiring §13).

Drives the full pipeline: POST creates the background Explorer task, the
test subscribes to the `TaskHandle` directly (cheap proxy for
`/tasks/{id}/events` — same fan-out machinery under the hood), and
asserts the expected event-type set lands in the stream before `done`.

We inject MockScripts with three canned `ExplorerAction`s so the loop
runs three iterations deterministically without hitting OpenAI. Judge
script is seeded in parallel so every iteration finds its verdict.
"""
from __future__ import annotations

import asyncio
import secrets
from collections.abc import Callable
from pathlib import Path

import httpx
import pytest

from codebus_agent.agent.types import (
    CoverageResult,
    ExplorerAction,
    JudgeVerdict,
    ToolCall,
)
from codebus_agent.api import create_app
from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


_RULES_VERSION = "2026-04-20-1"


def _make_tracked_factory(
    role: ProviderRole, default_module: str, script: MockScript
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


@pytest.mark.asyncio
async def test_explore_endpoint_emits_full_event_sequence(tmp_path: Path) -> None:
    """Spec: end-to-end event stream includes agent_thought / action_result /
    judge_verdict / usage_delta / llm_call / progress / done in a single run.
    """
    bearer = secrets.token_urlsafe(32)

    reasoning_script = MockScript()
    judge_script = MockScript()
    coverage_script = MockScript()

    # Seed three ExplorerActions — two with tool_calls, one empty — plus
    # three JudgeVerdicts so each iteration resolves deterministically.
    reasoning_script.push(
        ExplorerAction(
            thought="step0-explore",
            tool_calls=[ToolCall(id="tc_1", name="primary_search", arguments={"query": "a"})],
            stop=False,
        )
    )
    reasoning_script.push(
        ExplorerAction(
            thought="step1-read",
            tool_calls=[ToolCall(id="tc_2", name="primary_search", arguments={"query": "b"})],
            stop=False,
        )
    )
    reasoning_script.push(
        ExplorerAction(thought="step2-wrap", tool_calls=[], stop=False)
    )
    for _ in range(3):
        judge_script.push(
            JudgeVerdict(
                relevance=0.5,
                should_follow_imports=False,
                should_add_station=False,
                reason="ok",
            )
        )
    # `coverage-gap-recurse`: run_explorer calls coverage.check once
    # after main-loop convergence. Budget=3 drains to 0 → the three
    # iterations above empty the reasoning + judge scripts; coverage
    # then fires and reads a pinned CoverageResult. Empty gaps block
    # recursion (skip_reason="no_gaps") so no second `check` call.
    coverage_script.push(CoverageResult(gaps=[]))

    app = create_app(bearer_token=bearer)
    app.state.llm_reasoning_provider = _make_tracked_factory(
        ProviderRole.REASONING, "reasoning", reasoning_script
    )
    app.state.llm_judge_provider = _make_tracked_factory(
        ProviderRole.JUDGE, "judge", judge_script
    )
    app.state.llm_coverage_provider = _make_tracked_factory(
        ProviderRole.JUDGE, "coverage", coverage_script
    )

    transport = httpx.ASGITransport(app=app)
    async with httpx.AsyncClient(transport=transport, base_url="http://test") as client:
        resp = await client.post(
            "/explore",
            json={
                "workspace_root": str(tmp_path),
                "task": "trace auth",
                "budget_steps": 3,
                "budget_tokens": 10_000,
            },
            headers={"Authorization": f"Bearer {bearer}"},
        )
        assert resp.status_code == 202, resp.text
        task_id = resp.json()["task_id"]

        handle = app.state.tasks.get(task_id)
        assert handle is not None

        queue = handle.subscribe()

        # Drive the loop to completion — yield enough times for the 3
        # iterations of Think → Act → Judge → Log → Update to resolve.
        events: list[dict] = []
        for _ in range(400):
            try:
                ev = await asyncio.wait_for(queue.get(), timeout=1.0)
            except asyncio.TimeoutError:
                break
            if not isinstance(ev, dict):
                break
            if ev.get("__close__"):
                break
            events.append(ev)
            if ev.get("type") == "done":
                break

    types_seen = {e["type"] for e in events if isinstance(e, dict)}
    for required in (
        "agent_thought",
        "agent_action_result",
        "judge_verdict",
        "usage_delta",
        "llm_call",
        "progress",
        "done",
    ):
        assert required in types_seen, (
            f"missing {required!r} in event stream; saw {sorted(types_seen)}"
        )

    # `step` values MUST be in [0, budget_steps) for the mandatory types.
    for e in events:
        if e.get("type") in {"agent_thought", "agent_action_result", "judge_verdict"}:
            assert 0 <= e["step"] < 3, f"step out of range: {e}"

    # usage_delta lines carry the provider's default_module.
    # `coverage-gap-recurse` adds a third evaluator — coverage runs once
    # after main-loop convergence so its `module="coverage"` tag joins
    # the set alongside reasoning / judge.
    usage_events = [e for e in events if e.get("type") == "usage_delta"]
    modules = {e["module"] for e in usage_events}
    assert modules <= {"reasoning", "judge", "coverage"}, (
        f"unexpected module labels on usage_delta: {modules}"
    )

    # `context-compression-token-budget`: every usage_delta must carry a
    # non-negative `session_total_tokens` int reflecting the post-call
    # TrackedProvider aggregate.
    for ev in usage_events:
        assert "session_total_tokens" in ev, (
            f"usage_delta missing session_total_tokens: {ev}"
        )
        assert isinstance(ev["session_total_tokens"], int)
        assert ev["session_total_tokens"] >= 0

    # `context-compression-token-budget`: budget_warning is optional —
    # short runs may not cross 80%. If emitted, the event must carry
    # the documented wire schema.
    warning_events = [e for e in events if e.get("type") == "budget_warning"]
    for ev in warning_events:
        assert ev["kind"] in {"tokens", "steps"}, ev
        assert isinstance(ev["current"], int)
        assert isinstance(ev["budget"], int)
        assert isinstance(ev["pct"], (int, float))

    # llm_call lines carry preview ≤ 200 chars.
    llm_events = [e for e in events if e.get("type") == "llm_call"]
    for e in llm_events:
        assert len(e["preview"]) <= 200
