"""RED tests for POST /explore endpoint (agent-sse-wiring §10).

Backs openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: POST /explore endpoint spawns Explorer under task registry

Strategy:
  * Build the real `create_app` and inject fake `llm_reasoning_provider`
    / `llm_judge_provider` factories via `app.state` — mirrors the pattern
    used by `test_kb_build.py::app_with_kb_deps`.
  * Tests drive behaviour through `TestClient.post("/explore", ...)`;
    background Explorer coroutines run under the task wrapper so the
    endpoint returns immediately without waiting on MockProvider output.
"""
from __future__ import annotations

import re
import secrets
from collections.abc import Callable
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


_TASK_ID_RE = re.compile(r"^explore_[0-9a-f]{8}$")
_RULES_VERSION = "2026-04-20-1"


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


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


@pytest.fixture
def app_with_explore_deps(bearer: str):
    """Build the real app and inject `llm_reasoning_provider` /
    `llm_judge_provider` / `llm_coverage_provider` factories that satisfy
    the endpoint's expectations.

    `coverage-gap-recurse` added the `llm_coverage_provider` slot —
    the endpoint's 503 gate requires all three factories to be wired.
    """
    app = create_app(bearer_token=bearer)
    reasoning_script = MockScript()
    judge_script = MockScript()
    coverage_script = MockScript()
    app.state.llm_reasoning_provider = _make_tracked_factory(
        ProviderRole.REASONING, "reasoning", reasoning_script
    )
    app.state.llm_judge_provider = _make_tracked_factory(
        ProviderRole.JUDGE, "judge", judge_script
    )
    app.state.llm_coverage_provider = _make_tracked_factory(
        ProviderRole.JUDGE, "coverage", coverage_script
    )
    return app


def test_happy_path_returns_202_with_task_id(
    app_with_explore_deps, bearer: str, tmp_path: Path
) -> None:
    """Spec scenario `Happy path returns 202 with task_id`."""
    client = TestClient(app_with_explore_deps)
    resp = client.post(
        "/explore",
        json={
            "workspace_root": str(tmp_path),
            "task": "trace how storage is wired",
            "budget_steps": 0,  # zero → loop short-circuits, no LLM needed
            "budget_tokens": 10_000,
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 202, resp.text
    body = resp.json()
    assert "task_id" in body
    assert _TASK_ID_RE.fullmatch(body["task_id"]), f"bad id {body['task_id']!r}"


def test_concurrent_task_rejected_409(
    app_with_explore_deps, bearer: str, tmp_path: Path
) -> None:
    """Spec scenario `Concurrent task rejected`."""
    # Pre-occupy the single-slot registry with a running scan task.
    registry = app_with_explore_deps.state.tasks
    occupant = registry.create("scan")
    assert occupant is not None
    assert occupant.status == "running"

    client = TestClient(app_with_explore_deps)
    resp = client.post(
        "/explore",
        json={
            "workspace_root": str(tmp_path),
            "task": "x",
            "budget_steps": 0,
            "budget_tokens": 10_000,
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 409, resp.text
    assert resp.json()["detail"]["code"] == "TASK_IN_FLIGHT"


def test_missing_workspace_root_rejected(
    app_with_explore_deps, bearer: str
) -> None:
    """Spec scenario `Missing workspace root rejected`."""
    client = TestClient(app_with_explore_deps)
    resp = client.post(
        "/explore",
        json={
            "workspace_root": "/absolutely/does/not/exist/here/anywhere",
            "task": "x",
            "budget_steps": 0,
            "budget_tokens": 10_000,
        },
        headers=_auth(bearer),
    )
    assert resp.status_code in {400, 404}, resp.text
    # No task MUST have been registered.
    assert app_with_explore_deps.state.tasks.current_running() is None


def test_bearer_authentication_enforced(app_with_explore_deps, tmp_path: Path) -> None:
    """Spec scenario `Bearer authentication enforced`."""
    client = TestClient(app_with_explore_deps)
    resp = client.post(
        "/explore",
        json={
            "workspace_root": str(tmp_path),
            "task": "x",
            "budget_steps": 0,
            "budget_tokens": 10_000,
        },
    )
    assert resp.status_code == 401


def test_explore_endpoint_requires_llm_coverage_provider(
    app_with_explore_deps, bearer: str, tmp_path: Path
) -> None:
    """Spec scenario `Explore endpoint requires coverage provider`.

    Backs `coverage-gap-recurse` task 8.1 + design Decision 7: the
    `llm_coverage_provider` slot is now a required dep alongside
    `llm_reasoning_provider` / `llm_judge_provider`. When the slot is
    None the endpoint MUST 503 with `EXPLORE_NOT_CONFIGURED` and the
    `missing` detail list MUST include `llm_coverage_provider`.
    """
    app_with_explore_deps.state.llm_coverage_provider = None
    client = TestClient(app_with_explore_deps)
    resp = client.post(
        "/explore",
        json={
            "workspace_root": str(tmp_path),
            "task": "x",
            "budget_steps": 0,
            "budget_tokens": 10_000,
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 503, resp.text
    detail = resp.json()["detail"]
    assert detail["code"] == "EXPLORE_NOT_CONFIGURED"
    assert "llm_coverage_provider" in detail["missing"], (
        f"missing list must surface `llm_coverage_provider`; saw {detail['missing']}"
    )
