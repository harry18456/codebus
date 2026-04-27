"""TDD red tests for `POST /scan?stream=true` — Section 12 of
openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/folder-scanner/spec.md
  Requirement: POST /scan opt-in async streaming mode

Spec invariants under test:
  * Sync mode unchanged when `stream=true` is absent.
  * `?stream=true` returns `{"task_id": "scan_<hex8>"}` immediately
    (latency well below scan completion time).
  * Wire `progress` events collapse `walking` / `sanitizing` to
    `phase: "scanning"` (verified via the pure translator function —
    the live HTTP path can't race the background coroutine reliably).
  * After `done`, `GET /tasks/{id}/result` returns the full ScanResult.
"""
from __future__ import annotations

import re
import secrets
import time
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app

_TASK_ID_RE = re.compile(r"^scan_[0-9a-f]{8}$")


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


@pytest.fixture
def app_with_registry(bearer: str):
    app = create_app(bearer_token=bearer)
    return app, app.state.tasks


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def _seed_workspace(root: Path, *, n: int = 4) -> None:
    for i in range(n):
        (root / f"f{i}.py").write_text(f"x = {i}\n", encoding="utf-8")
    (root / "contacts.txt").write_text(
        "alice@example.com\nbob@example.com\n", encoding="utf-8"
    )


def test_scan_without_stream_query_returns_sync_result(
    tmp_path: Path, app_with_registry, bearer
) -> None:
    """Spec scenario "Sync mode unchanged when stream query absent"."""
    _seed_workspace(tmp_path)
    app, _registry = app_with_registry
    client = TestClient(app)

    resp = client.post(
        "/scan",
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
        headers=_auth(bearer),
    )
    assert resp.status_code == 200
    body = resp.json()
    assert "task_id" not in body
    assert "files" in body
    assert "stats" in body
    assert "workspace_root" in body


def test_scan_with_stream_true_returns_task_id_immediately(
    tmp_path: Path, app_with_registry, bearer
) -> None:
    """Spec scenario "Stream mode returns task_id and starts background work".

    Latency upper bound — the request MUST return well before scan would
    naturally complete. Tiny workspace + 2s ceiling keeps CI happy.
    """
    _seed_workspace(tmp_path, n=2)
    app, _registry = app_with_registry
    client = TestClient(app)

    started = time.monotonic()
    resp = client.post(
        "/scan?stream=true",
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
        headers=_auth(bearer),
    )
    elapsed = time.monotonic() - started

    assert resp.status_code == 202
    assert elapsed < 2.0, f"stream=true blocked for {elapsed:.3f}s"
    body = resp.json()
    assert "task_id" in body
    assert _TASK_ID_RE.fullmatch(body["task_id"]), f"bad id {body['task_id']!r}"


def test_scan_stream_phase_collapsed_to_scanning() -> None:
    """`_scanner_event_to_wire` MUST collapse both internal phases
    (`walking` / `sanitizing`) to wire `phase: "scanning"` per the
    Module 1 phase-name mapping (spec Requirement: POST /scan opt-in
    async streaming mode, paragraph "translate every ScannerProgressEvent").

    Tested at the pure-function level because the live HTTP path can't
    reliably race the background coroutine to capture in-flight events
    via `TestClient`.
    """
    from codebus_agent.api.scan import _scanner_event_to_wire
    from codebus_agent.scanner.models import ScannerProgressEvent

    walking = _scanner_event_to_wire(
        ScannerProgressEvent(
            phase="walking", current=10, total=None, current_file="a.py"
        )
    )
    sanitizing = _scanner_event_to_wire(
        ScannerProgressEvent(
            phase="sanitizing", current=3, total=5, current_file=None
        )
    )
    assert walking == {
        "type": "progress",
        "phase": "scanning",
        "current": 10,
        "total": None,
        "current_file": "a.py",
    }
    assert sanitizing == {
        "type": "progress",
        "phase": "scanning",
        "current": 3,
        "total": 5,
        "current_file": None,
    }


def test_scan_stream_done_then_result_returns_full_scan_result(
    tmp_path: Path, app_with_registry, bearer
) -> None:
    """Spec scenario "Stream done event triggers result endpoint readiness"."""
    _seed_workspace(tmp_path)
    app, registry = app_with_registry
    client = TestClient(app)

    resp = client.post(
        "/scan?stream=true",
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202
    task_id = resp.json()["task_id"]
    handle = registry.get(task_id)
    assert handle is not None

    deadline = time.monotonic() + 5.0
    while handle.status == "running" and time.monotonic() < deadline:
        time.sleep(0.05)
    assert handle.status == "done"

    result_resp = client.get(f"/tasks/{task_id}/result", headers=_auth(bearer))
    assert result_resp.status_code == 200
    body = result_resp.json()
    assert "files" in body
    assert "stats" in body
    assert "workspace_root" in body
