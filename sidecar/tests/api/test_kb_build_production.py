"""TDD red tests for production KB build paths — Section 6 of
openspec/changes/kb-build-production-wiring/tasks.md.

Backs openspec/changes/kb-build-production-wiring/specs/knowledge-base/spec.md
  Requirement: KB build production dependency wiring

Strategy:
  * Happy-path and usage-tracker scenarios use in-memory doubles
    (SpyProvider, InMemoryQdrantBackend) with factory-shaped injection
    so the TestClient does not need a real Qdrant or OpenAI API.
  * Graceful-degrade and rate-limited scenarios test the error paths:
    503 KB_NOT_CONFIGURED when deps aren't wired, and SSE error event
    with `OPENAI_RATE_LIMITED` code when the provider raises that type.
"""
from __future__ import annotations

import secrets
import time
from datetime import datetime, timezone
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.providers import OpenAIRateLimitError
from codebus_agent.providers.protocol import EmbedResponse, Usage
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.scanner.models import (
    ContentTypeSummary,
    FileEntry,
    ScanResult,
    ScanStats,
)


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def _make_scan_dict(ws: str = "/abs/workspace/demo") -> dict:
    files = [
        FileEntry(
            path=f"f{i}.py",
            size=20,
            kind="text",
            language="python",
            encoding="utf-8",
            content=f"x = {i}\n",
        )
        for i in range(3)
    ]
    sr = ScanResult(
        workspace_root=ws,
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


def test_missing_openai_key_returns_503_kb_not_configured(bearer: str) -> None:
    """Spec scenario "Missing OpenAI API key returns 503 KB_NOT_CONFIGURED"."""
    # create_app without openai_api_key leaves kb_provider / kb_embedding_dim None.
    app = create_app(bearer_token=bearer, openai_api_key=None)
    client = TestClient(app)

    resp = client.post(
        "/kb/build",
        json={"workspace_root": "/abs/workspace/demo", "scan_result": _make_scan_dict()},
        headers=_auth(bearer),
    )
    assert resp.status_code == 503, resp.text
    detail = resp.json().get("detail", {})
    assert detail.get("code") == "KB_NOT_CONFIGURED"
    assert "missing" in detail
    assert any("provider" in m for m in detail["missing"])


def test_happy_path_kbstats_nonzero_counters(bearer: str, tmp_path: Path) -> None:
    """Spec scenario "Happy path returns KBStats via result endpoint"."""
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    app = create_app(bearer_token=bearer)
    spy = SpyProvider()
    ws_path = tmp_path / "ws-happy"
    ws_path.mkdir()
    ws_str = str(ws_path)

    shared_tracker = UsageTracker(ws_path / "token_usage.jsonl")
    app.state.kb_backend = InMemoryQdrantBackend()
    app.state.kb_provider = lambda _ws: spy
    app.state.kb_usage_tracker = lambda _ws: shared_tracker
    app.state.kb_embedding_dim = 8

    client = TestClient(app)
    resp = client.post(
        "/kb/build",
        json={"workspace_root": ws_str, "scan_result": _make_scan_dict(ws=ws_str)},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202
    task_id = resp.json()["task_id"]

    handle = app.state.tasks.get(task_id)
    assert handle is not None
    deadline = time.monotonic() + 5.0
    while handle.status == "running" and time.monotonic() < deadline:
        time.sleep(0.05)
    assert handle.status == "done", f"build never finished: {handle.status}"

    result = client.get(f"/tasks/{task_id}/result", headers=_auth(bearer)).json()
    assert result["chunks_emitted"] > 0
    assert result["points_upserted"] > 0
    assert result["workspace_id"]
    assert result["collection_name"]


def test_usage_tracker_writes_to_workspace_scoped_path(
    bearer: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Spec scenario "UsageTracker records embedding call for the requesting workspace".

    Post `usage-tracker-dedup`: provider factory wraps SpyProvider in
    TrackedProvider with `default_module="kb_build"` to mirror the
    production wiring path. KnowledgeBase no longer writes to the
    tracker itself, so the wrapping is now load-bearing.
    """
    from codebus_agent.providers import (
        LLMCallLogger,
        ProviderRole,
        TrackedProvider,
    )
    from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    # Test-only relaxation of TrackedProvider's allowed inner types so we
    # can wrap SpyProvider without spinning up real OpenAI traffic.
    monkeypatch.setattr(
        TrackedProvider,
        "ALLOWED_INNER_TYPES",
        TrackedProvider.ALLOWED_INNER_TYPES | {SpyProvider},
    )

    app = create_app(bearer_token=bearer)
    spy = SpyProvider()

    ws_path = tmp_path / "ws-u"
    ws_path.mkdir()
    ws_str = str(ws_path)
    usage_path = ws_path / "token_usage.jsonl"

    def _provider_factory(ws: Path) -> TrackedProvider:
        # Mirror production: wrap raw provider in TrackedProvider with
        # workspace-scoped audit logs + default_module="kb_build".
        return TrackedProvider(
            spy,
            tracker=UsageTracker(Path(ws) / "token_usage.jsonl"),
            logger=LLMCallLogger(Path(ws) / "llm_calls.jsonl"),
            role=ProviderRole.EMBED,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(
                Path(ws) / ".codebus" / "sanitize_audit.jsonl"
            ),
            rules_version="2026-04-20-1",
            default_module="kb_build",
        )

    def _tracker_factory(ws: Path) -> UsageTracker:
        # Path MUST derive from caller's workspace_root.
        return UsageTracker(Path(ws) / "token_usage.jsonl")

    app.state.kb_backend = InMemoryQdrantBackend()
    app.state.kb_provider = _provider_factory
    app.state.kb_usage_tracker = _tracker_factory
    app.state.kb_embedding_dim = 8

    client = TestClient(app)
    resp = client.post(
        "/kb/build",
        json={"workspace_root": ws_str, "scan_result": _make_scan_dict(ws=ws_str)},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202
    task_id = resp.json()["task_id"]

    handle = app.state.tasks.get(task_id)
    deadline = time.monotonic() + 5.0
    while handle.status == "running" and time.monotonic() < deadline:
        time.sleep(0.05)
    assert handle.status == "done"

    assert usage_path.exists(), f"token_usage.jsonl NOT written at {usage_path}"
    lines = usage_path.read_text(encoding="utf-8").strip().splitlines()
    assert lines, "token_usage.jsonl is empty"
    import json

    embed_lines = [
        json.loads(line) for line in lines if json.loads(line).get("operation") == "embed"
    ]
    assert embed_lines, "no 'operation: embed' line found in token_usage.jsonl"
    # `usage-tracker-dedup` Requirement contract:
    # `UsageTracker writes token_usage.jsonl` Scenario "Module field
    # reflects TrackedProvider's default_module" — every embed line MUST
    # carry the module label, AND no duplicate must be written by any
    # other layer (pre-fix bug: KnowledgeBase.build also called
    # tracker.record() so each batch produced 2 lines).
    assert all(e.get("module") == "kb_build" for e in embed_lines), (
        f"every embed line MUST carry module='kb_build'; got modules "
        f"{[e.get('module') for e in embed_lines]}"
    )
    # Number of embed lines MUST equal `batches_embedded` from KBStats —
    # exactly one record per batch, never two.
    result = client.get(
        f"/tasks/{task_id}/result", headers=_auth(bearer)
    ).json()
    assert len(embed_lines) == result["batches_embedded"], (
        f"token_usage.jsonl has {len(embed_lines)} embed lines but "
        f"KBStats reports {result['batches_embedded']} batches embedded — "
        f"this is the dedup invariant from `usage-tracker-dedup`."
    )


def test_openai_rate_limited_surfaces_as_sse_error_event(
    bearer: str, tmp_path: Path
) -> None:
    """Spec scenario "OpenAI rate limit surfaces as sanitized error event"."""
    from tests.kb.conftest import InMemoryQdrantBackend

    class _RateLimitingProvider:
        name = "openai-embedding-mock"
        model = "text-embedding-3-small"
        embed_calls: list[list[str]] = []

        async def embed(self, texts: list[str]) -> EmbedResponse:
            self.embed_calls.append(list(texts))
            raise OpenAIRateLimitError("rate limited after retries")

    app = create_app(bearer_token=bearer)
    ws_path = tmp_path / "ws-rl"
    ws_path.mkdir()
    ws_str = str(ws_path)

    shared_tracker = UsageTracker(ws_path / "token_usage.jsonl")
    app.state.kb_backend = InMemoryQdrantBackend()
    app.state.kb_provider = lambda _ws: _RateLimitingProvider()
    app.state.kb_usage_tracker = lambda _ws: shared_tracker
    app.state.kb_embedding_dim = 8

    client = TestClient(app)
    resp = client.post(
        "/kb/build",
        json={"workspace_root": ws_str, "scan_result": _make_scan_dict(ws=ws_str)},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202
    task_id = resp.json()["task_id"]

    handle = app.state.tasks.get(task_id)
    deadline = time.monotonic() + 5.0
    while handle.status == "running" and time.monotonic() < deadline:
        time.sleep(0.05)
    assert handle.status == "error", f"expected error status, got {handle.status}"

    err = handle.error_event
    assert err is not None
    assert err["type"] == "error"
    assert err["code"] == "OPENAI_RATE_LIMITED"
    # Sanitized — no raw traceback / exception repr.
    assert "Traceback" not in err.get("message", "")
    assert "rate limit" in err["message"].lower()
