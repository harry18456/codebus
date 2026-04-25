"""Tests for ``POST /generate`` endpoint (Section 15).

Backs Requirements
``task_id format`` (generate kind) and
``Background task error containment`` (GENERATE_FAILED) in
``openspec/changes/module-5-generator-p0/specs/sidecar-runtime/spec.md``.
"""
from __future__ import annotations

import json
import re
import secrets
import time
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


_TASK_ID_RE = re.compile(r"^generate_[0-9a-f]{8}$")
_RULES_VERSION = "2026-04-20-1"


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def _make_generate_factory(script: MockScript) -> Callable[[Path], TrackedProvider]:
    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        audit_dir = ws / ".codebus"
        audit_dir.mkdir(parents=True, exist_ok=True)
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


def _make_chat_factory(role: ProviderRole, module: str) -> Callable[[Path], TrackedProvider]:
    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        audit_dir = ws / ".codebus"
        audit_dir.mkdir(parents=True, exist_ok=True)
        return TrackedProvider(
            MockProvider(script=MockScript(), role=role),
            tracker=UsageTracker(audit_dir / "token_usage.jsonl"),
            logger=LLMCallLogger(audit_dir / "llm_calls.jsonl"),
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl"),
            rules_version=_RULES_VERSION,
            default_module=module,
        )

    return _factory


@pytest.fixture
def app_with_generate_deps(bearer: str):
    app = create_app(bearer_token=bearer)
    script = MockScript()
    app.state.llm_generate_provider = _make_generate_factory(script)
    # Wire the explore-side factories too so the slot-blocking test
    # reaches the registry check on /explore (the 503 dep-gate fires
    # before the 409 registry check otherwise). Each gets its own
    # MockScript — none of these factories actually drive an LLM call
    # in the slot-blocking test path.
    app.state.llm_reasoning_provider = _make_chat_factory(
        ProviderRole.REASONING, "reasoning"
    )
    app.state.llm_judge_provider = _make_chat_factory(ProviderRole.JUDGE, "judge")
    app.state.llm_coverage_provider = _make_chat_factory(
        ProviderRole.JUDGE, "coverage"
    )
    app.state._generate_script = script  # store on app.state for tests to push to
    return app


def test_generate_endpoint_requires_bearer_token(
    app_with_generate_deps, tmp_path: Path
) -> None:
    client = TestClient(app_with_generate_deps)
    resp = client.post(
        "/generate",
        json={"workspace_root": str(tmp_path), "task": "x", "stations": []},
    )
    assert resp.status_code == 401


def test_generate_kind_follows_same_shape(
    app_with_generate_deps, bearer: str, tmp_path: Path
) -> None:
    client = TestClient(app_with_generate_deps)
    resp = client.post(
        "/generate",
        json={"workspace_root": str(tmp_path), "task": "x", "stations": []},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202, resp.text
    body = resp.json()
    assert "task_id" in body
    assert _TASK_ID_RE.fullmatch(body["task_id"]), f"bad id {body['task_id']!r}"


def test_in_flight_generate_blocks_other_task_creation(
    app_with_generate_deps, bearer: str, tmp_path: Path
) -> None:
    # Pre-occupy the registry with a running generate task.
    registry = app_with_generate_deps.state.tasks
    occupant = registry.create("generate")
    assert occupant is not None
    assert occupant.status == "running"

    # /scan (sync) does not reserve a slot — only `/scan?stream=true`
    # does. /kb/build needs KB deps wired (out of scope for this fixture
    # — covered by `test_kb_build.py`'s own concurrent test). The slot
    # enforcement is generic across endpoints; we exercise the three
    # representative paths here.
    client = TestClient(app_with_generate_deps)
    cases: list[tuple[str, dict]] = [
        (
            "/scan?stream=true",
            {"workspace_type": "folder", "workspace_root": str(tmp_path)},
        ),
        (
            "/explore",
            {
                "workspace_root": str(tmp_path),
                "task": "x",
                "budget_steps": 0,
                "budget_tokens": 1000,
            },
        ),
        (
            "/generate",
            {
                "workspace_root": str(tmp_path),
                "task": "x",
                "stations": [],
                "options": {"mode": "interactive", "target_persona": "x"},
            },
        ),
    ]
    for path, body in cases:
        resp = client.post(path, json=body, headers=_auth(bearer))
        assert resp.status_code == 409, (
            f"{path} must 409 while a task is in flight; got {resp.status_code} "
            f"({resp.text})"
        )
        assert resp.json()["detail"]["code"] == "TASK_IN_FLIGHT", path


def test_generate_task_exception_surfaces_as_safe_error_event(
    app_with_generate_deps, bearer: str, tmp_path: Path, monkeypatch
) -> None:
    """Force run_generator to raise; assert wire emits GENERATE_FAILED."""
    from codebus_agent.api import generate as generate_module

    async def _explode(**kwargs):
        raise RuntimeError("boom-internal-detail-do-not-leak")

    monkeypatch.setattr(generate_module, "run_generator", _explode)

    client = TestClient(app_with_generate_deps)
    resp = client.post(
        "/generate",
        json={"workspace_root": str(tmp_path), "task": "x", "stations": []},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202, resp.text
    task_id = resp.json()["task_id"]

    # Poll the registry for terminal state — the background task wrapper
    # turns the RuntimeError into a sanitized error event.
    handle = app_with_generate_deps.state.tasks.get(task_id)
    for _ in range(40):
        if handle.status != "running":
            break
        time.sleep(0.05)

    assert handle.status == "error", handle.status
    assert handle.error_event is not None
    event = handle.error_event
    assert event["code"] == "GENERATE_FAILED", event
    assert "boom-internal-detail-do-not-leak" not in json.dumps(event), (
        f"sanitized error event MUST NOT echo internal detail; got {event!r}"
    )
