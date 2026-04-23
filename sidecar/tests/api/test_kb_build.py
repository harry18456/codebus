"""TDD red tests for `POST /kb/build` — Section 14 of
openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/knowledge-base/spec.md
  Requirement: POST /kb/build async endpoint
  Requirement: KB progress phase translation to wire schema

Strategy:
  * The HTTP-shaped tests (14.1-14.3) drive the live endpoint with
    test-only injection of `(backend, provider)` via ``app.state`` so
    Qdrant is not required.
  * The wire-translation tests (14.4-14.6) hit the pure adapter
    function ``_kb_event_to_wire`` directly — racing the background
    task to capture in-flight events through ``TestClient`` is not
    reliable, and the load-bearing logic is in the adapter.
"""
from __future__ import annotations

import re
import secrets
import time
from datetime import datetime, timezone
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.scanner.models import (
    ContentTypeSummary,
    FileEntry,
    ScanResult,
    ScanStats,
)

_TASK_ID_RE = re.compile(r"^kb_[0-9a-f]{8}$")


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def _make_scan(workspace_root: str = "/abs/workspace/demo") -> dict:
    """Build a tiny valid ScanResult JSON body (3 small text files)."""
    files = [
        FileEntry(
            path=f"a{i}.py",
            size=10,
            kind="text",
            language="python",
            encoding="utf-8",
            content=f"x = {i}\n",
        )
        for i in range(3)
    ]
    sr = ScanResult(
        workspace_root=workspace_root,
        scan_started_at=datetime(2026, 4, 22, 12, 0, 0, tzinfo=timezone.utc),
        scan_completed_at=datetime(2026, 4, 22, 12, 0, 1, tzinfo=timezone.utc),
        files=files,
        symlinks=[],
        content_summary=ContentTypeSummary(
            total_files=len(files),
            kind_counts={},
            language_counts={},
            category_counts={},
            dominant_category="code",
            dominant_languages=[],
            has_tests=False,
            has_docs=False,
            is_monorepo=False,
        ),
        stats=ScanStats(
            total_files_walked=len(files),
            total_files_included=len(files),
            total_bytes_read=sum(f.size for f in files),
            duration_seconds=0.5,
            quarantined_count=0,
            skipped_count=0,
        ),
    )
    return sr.model_dump(mode="json")


@pytest.fixture
def app_with_kb_deps(bearer: str):
    """Build an app and inject in-memory KB dependencies via app.state.

    The endpoint resolves `(backend, provider_factory, tracker_factory)` from
    `app.state` per `kb-build-production-wiring` A-plan (decision 3):
    provider + tracker are workspace-scoped factories. Tests wrap the
    existing singleton doubles in ``lambda _ws: instance`` so the same
    spy / in-memory backend backs every build.
    """
    from codebus_agent.providers.usage_tracker import UsageTracker

    # Reuse the offline doubles from tests/kb/conftest.py without forcing
    # the conftest into this directory: import them directly.
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    app = create_app(bearer_token=bearer)
    spy_provider = SpyProvider()
    shared_tracker = UsageTracker(Path("/tmp/codebus_kb_api_test_usage.jsonl"))
    app.state.kb_backend = InMemoryQdrantBackend()
    app.state.kb_provider = lambda _ws: spy_provider
    app.state.kb_usage_tracker = lambda _ws: shared_tracker
    app.state.kb_embedding_dim = 8
    return app


def test_kb_build_returns_task_id_immediately(
    app_with_kb_deps, bearer
) -> None:
    """Spec scenario "Successful request returns task_id immediately"."""
    client = TestClient(app_with_kb_deps)
    started = time.monotonic()
    resp = client.post(
        "/kb/build",
        json={
            "workspace_root": "/abs/workspace/demo",
            "scan_result": _make_scan(),
        },
        headers=_auth(bearer),
    )
    elapsed = time.monotonic() - started

    assert resp.status_code == 200
    assert elapsed < 2.0, f"endpoint blocked for {elapsed:.3f}s"
    body = resp.json()
    assert "task_id" in body
    assert _TASK_ID_RE.fullmatch(body["task_id"]), f"bad id {body['task_id']!r}"


def test_kb_build_rejects_concurrent_request_with_409(
    app_with_kb_deps, bearer
) -> None:
    """Spec scenario "Concurrent task in flight rejected with 409"."""
    # Force a stuck running task by occupying the slot directly.
    registry = app_with_kb_deps.state.tasks
    occupant = registry.create("kb")
    assert occupant is not None  # status defaults to "running"

    client = TestClient(app_with_kb_deps)
    resp = client.post(
        "/kb/build",
        json={
            "workspace_root": "/abs/workspace/demo",
            "scan_result": _make_scan(),
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 409
    body = resp.json()
    detail = body.get("detail", body)
    assert detail.get("code") == "TASK_IN_FLIGHT"
    assert detail.get("running_task_id") == occupant.id


def test_kb_build_done_then_result_returns_kbstats(
    app_with_kb_deps, bearer
) -> None:
    """Spec scenario "Done event makes KBStats reachable via result endpoint"."""
    client = TestClient(app_with_kb_deps)
    resp = client.post(
        "/kb/build",
        json={
            "workspace_root": "/abs/workspace/demo",
            "scan_result": _make_scan(),
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 200
    task_id = resp.json()["task_id"]

    handle = app_with_kb_deps.state.tasks.get(task_id)
    assert handle is not None
    deadline = time.monotonic() + 5.0
    while handle.status == "running" and time.monotonic() < deadline:
        time.sleep(0.05)
    assert handle.status == "done", f"build never finished: {handle.status}"

    result_resp = client.get(f"/tasks/{task_id}/result", headers=_auth(bearer))
    assert result_resp.status_code == 200
    body = result_resp.json()
    # KBStats shape — must have these counters.
    assert "chunks_emitted" in body
    assert "points_upserted" in body
    assert "workspace_id" in body
    assert "collection_name" in body


# ---------------------------------------------------------------------------
# Wire translation tests — pure-function level
# ---------------------------------------------------------------------------


def test_kb_phase_collapsed_to_embedding_in_wire_events() -> None:
    """Spec scenario "All non-done source phases collapse to embedding".

    Both `chunking` and `upserting` source events MUST translate to wire
    events with `phase: "embedding"`.
    """
    from codebus_agent.api.kb import _KBProgressAdapter
    from codebus_agent.kb.payload import KBProgressEvent

    adapter = _KBProgressAdapter()
    chunk = adapter.translate(
        KBProgressEvent(phase="chunking", current=10, total=10, workspace_id="w")
    )
    embed = adapter.translate(
        KBProgressEvent(phase="embedding", current=5, total=10, workspace_id="w")
    )
    upsert = adapter.translate(
        KBProgressEvent(phase="upserting", current=10, total=10, workspace_id="w")
    )
    for w in (chunk, embed, upsert):
        assert w is not None
        assert w["type"] == "progress"
        assert w["phase"] == "embedding"


def test_kb_wire_progress_monotonic_and_reaches_total() -> None:
    """Spec scenario "Wire stream is monotonic and reaches total".

    The wire stream MUST contain at least one event with current==0 AND
    one with current==total, with monotonic non-decreasing currents.
    """
    from codebus_agent.api.kb import _KBProgressAdapter
    from codebus_agent.kb.payload import KBProgressEvent

    adapter = _KBProgressAdapter()
    # Simulate a build that emits N=10 chunks but only M=8 to embed
    # (some dedup) and finally P=8 points to upsert.
    sequence = [
        KBProgressEvent(phase="chunking", current=10, total=10, workspace_id="w"),
        KBProgressEvent(phase="embedding", current=0, total=8, workspace_id="w"),
        KBProgressEvent(phase="embedding", current=4, total=8, workspace_id="w"),
        KBProgressEvent(phase="embedding", current=8, total=8, workspace_id="w"),
        KBProgressEvent(phase="upserting", current=0, total=8, workspace_id="w"),
        KBProgressEvent(phase="upserting", current=8, total=8, workspace_id="w"),
    ]
    wire = [adapter.translate(e) for e in sequence]
    wire = [w for w in wire if w is not None]

    assert wire, "expected at least one wire event"
    currents = [w["current"] for w in wire]
    totals = [w["total"] for w in wire]
    # Monotonic non-decreasing.
    assert all(b >= a for a, b in zip(currents, currents[1:])), (
        f"wire currents not monotonic: {currents!r}"
    )
    # Endpoints satisfied.
    assert any(c == 0 for c in currents), f"missing current==0 in {currents!r}"
    final_total = totals[-1]
    assert any(c == final_total for c in currents), (
        f"missing current==total in {currents!r} (total={final_total})"
    )


def test_kb_source_done_phase_does_not_emit_wire_progress() -> None:
    """Spec scenario "Source done phase becomes wire done event".

    The adapter MUST NOT translate `done` into a `progress` wire event.
    The terminal SSE `done` is emitted by the task wrapper.
    """
    from codebus_agent.api.kb import _KBProgressAdapter
    from codebus_agent.kb.payload import KBProgressEvent

    adapter = _KBProgressAdapter()
    out = adapter.translate(
        KBProgressEvent(phase="done", current=10, total=10, workspace_id="w")
    )
    assert out is None
