"""Tests for ``POST /generate`` ``target_stations`` partial-regen field.

Backs spec ADDED Requirement ``Partial regen via target_stations
preserves unrelated stations`` in
``openspec/changes/phase6-step29-intervention-points/specs/module-5-generator/spec.md``.

The endpoint MUST:
- Accept ``target_stations: list[str] | None`` (default None — equivalent
  to existing full-tutorial behavior).
- When present, every id MUST resolve to one of the deterministic
  station ids derivable from ``state.stations`` (using the same
  ``generate_station_id`` pipeline ``run_generator`` uses internally).
- Otherwise raise HTTP 400 ``GENERATE_TARGET_STATION_INVALID`` carrying
  the offending id; the background task MUST NOT be spawned.
"""
from __future__ import annotations

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


@pytest.fixture
def app_with_generate_deps(bearer: str):
    app = create_app(bearer_token=bearer)
    script = MockScript()
    app.state.llm_generate_provider = _make_generate_factory(script)
    app.state._generate_script = script
    return app


def _wait_terminal(handle, timeout: float = 4.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if handle.status != "running":
            return
        time.sleep(0.02)


def test_target_stations_unknown_id_rejected_400(
    app_with_generate_deps, bearer: str, tmp_path: Path
) -> None:
    """Unknown station id in target_stations → 400 GENERATE_TARGET_STATION_INVALID."""
    client = TestClient(app_with_generate_deps)
    resp = client.post(
        "/generate",
        json={
            "workspace_root": str(tmp_path),
            "task": "ingest mqtt feed",
            "stations": [
                {"path": "src/a.ts", "role": "interface", "relevance": 0.5, "why": "."},
                {"path": "src/b.ts", "role": "interface", "relevance": 0.5, "why": "."},
            ],
            "target_stations": ["s99-not-real"],
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 400, resp.text
    detail = resp.json()["detail"]
    assert detail["code"] == "GENERATE_TARGET_STATION_INVALID"
    # The bad id MUST appear so the caller can surface it back to the user.
    assert detail["station_id"] == "s99-not-real"


def test_target_stations_default_none_uses_full_path(
    app_with_generate_deps, bearer: str, tmp_path: Path
) -> None:
    """Omitting target_stations behaves identically to the existing full path."""
    client = TestClient(app_with_generate_deps)
    resp = client.post(
        "/generate",
        json={
            "workspace_root": str(tmp_path),
            "task": "x",
            "stations": [],
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 202, resp.text
    body = resp.json()
    assert _TASK_ID_RE.fullmatch(body["task_id"])


def test_target_stations_valid_id_accepted_202(
    app_with_generate_deps, bearer: str, tmp_path: Path
) -> None:
    """A target_stations id matching a derivable station id is accepted (202)."""
    # Two stations: src/a.ts → "A" → s01-a; src/b.ts → "B" → s02-b
    client = TestClient(app_with_generate_deps)
    resp = client.post(
        "/generate",
        json={
            "workspace_root": str(tmp_path),
            "task": "x",
            "stations": [
                {"path": "src/a.ts", "role": "interface", "relevance": 0.5, "why": "."},
                {"path": "src/b.ts", "role": "interface", "relevance": 0.5, "why": "."},
            ],
            "target_stations": ["s02-b"],
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 202, resp.text
    body = resp.json()
    assert _TASK_ID_RE.fullmatch(body["task_id"])

    # The background task may fail if route.json doesn't exist yet (partial
    # mode requires prior full run), which is acceptable for this smoke
    # test — we're verifying the endpoint accepts the valid id, not the
    # full partial-regen runner path (covered in test_runner_partial_regen).
    handle = app_with_generate_deps.state.tasks.get(body["task_id"])
    _wait_terminal(handle)
