"""TDD red tests for `POST /kb/query` — Section 2 of
openspec/changes/kb-query-endpoint/tasks.md.

Backs openspec/changes/kb-query-endpoint/specs/knowledge-base/spec.md
  Requirement: POST /kb/query endpoint
and openspec/changes/kb-query-endpoint/specs/sidecar-runtime/spec.md
  Requirement: KB query endpoint registration

Strategy:
  * In-memory backend + SpyProvider for offline coverage; the production
    factory pattern is mirrored via `lambda _ws: instance` injection.
  * For the usage-tracking scenario, SpyProvider is wrapped in a real
    TrackedProvider with `default_module="kb_query"` (test-only relax of
    `ALLOWED_INNER_TYPES` matches the pattern from
    `kb-build-production-wiring` Section 6).
"""
from __future__ import annotations

import json
import secrets
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.providers.usage_tracker import UsageTracker


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def _seed_collection(backend, kb_provider, *, workspace_root: str) -> None:
    """Pre-populate the in-memory backend with a few points so query has
    something to find. Embeddings are generated through the spy provider so
    the same vector ↔ text correspondence holds at query time.
    """
    import asyncio

    from codebus_agent.kb.knowledge_base import KnowledgeBase
    from codebus_agent.scanner.models import (
        ContentTypeSummary,
        FileEntry,
        ScanResult,
        ScanStats,
    )
    from datetime import datetime, timezone

    files = [
        FileEntry(
            path=f"src/file{i}.py",
            size=20,
            kind="text",
            language="python",
            encoding="utf-8",
            content=f"def func_{i}():\n    return {i}\n",
        )
        for i in range(3)
    ]
    sr = ScanResult(
        workspace_root=workspace_root,
        scan_started_at=datetime(2026, 4, 23, 12, 0, 0, tzinfo=timezone.utc),
        scan_completed_at=datetime(2026, 4, 23, 12, 0, 1, tzinfo=timezone.utc),
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
    tracker = UsageTracker(Path("/tmp/codebus_kb_query_test_seed_usage.jsonl"))
    kb = KnowledgeBase(
        backend=backend,
        provider=kb_provider,
        usage_tracker=tracker,
        workspace_root=workspace_root,
        embedding_dim=8,
    )
    asyncio.run(kb.build(sr))


@pytest.fixture
def app_with_query_deps(bearer: str, tmp_path: Path):
    """App with KB query dependencies wired (mirrors kb-build fixture).

    `kb_provider` and `kb_query_provider` slots both point at the SAME
    SpyProvider for test simplicity — production wires distinct
    TrackedProvider instances with different `default_module` labels.
    """
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    app = create_app(bearer_token=bearer)
    spy = SpyProvider()
    shared_tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    backend = InMemoryQdrantBackend()
    app.state.kb_backend = backend
    app.state.kb_provider = lambda _ws: spy
    app.state.kb_query_provider = lambda _ws: spy
    app.state.kb_usage_tracker = lambda _ws: shared_tracker
    app.state.kb_embedding_dim = 8
    # Stash so tests can pre-seed the collection.
    app.state._spy_provider = spy
    app.state._tmp_path = tmp_path
    return app


def test_query_returns_hits_ordered_by_score(
    app_with_query_deps, bearer
) -> None:
    """Spec scenario "Successful query returns hits ordered by score"."""
    workspace_root = str(app_with_query_deps.state._tmp_path / "ws-q")
    Path(workspace_root).mkdir(exist_ok=True)
    _seed_collection(
        app_with_query_deps.state.kb_backend,
        app_with_query_deps.state._spy_provider,
        workspace_root=workspace_root,
    )

    client = TestClient(app_with_query_deps)
    resp = client.post(
        "/kb/query",
        json={
            "workspace_root": workspace_root,
            "text": "func_1",
            "top_k": 3,
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 200, resp.text
    body = resp.json()
    assert "hits" in body
    assert isinstance(body["hits"], list)
    assert len(body["hits"]) <= 3
    scores = [h["score"] for h in body["hits"]]
    assert scores == sorted(scores, reverse=True), (
        f"scores not monotonically non-increasing: {scores}"
    )
    for hit in body["hits"]:
        assert "point_id" in hit
        assert "score" in hit
        assert "payload" in hit


def test_empty_collection_returns_200_with_empty_hits(
    app_with_query_deps, bearer
) -> None:
    """Spec scenario "Empty collection returns empty hits list with 200"."""
    # Workspace never built — no collection yet.
    workspace_root = str(app_with_query_deps.state._tmp_path / "ws-empty")
    Path(workspace_root).mkdir(exist_ok=True)

    client = TestClient(app_with_query_deps)
    resp = client.post(
        "/kb/query",
        json={"workspace_root": workspace_root, "text": "anything"},
        headers=_auth(bearer),
    )
    assert resp.status_code == 200
    assert resp.json() == {"hits": []}


def test_missing_openai_key_returns_503(bearer) -> None:
    """Spec scenario "Missing OpenAI API key returns 503 KB_NOT_CONFIGURED"."""
    app = create_app(bearer_token=bearer, openai_api_key=None)
    client = TestClient(app)
    resp = client.post(
        "/kb/query",
        json={"workspace_root": "/tmp/x", "text": "hi"},
        headers=_auth(bearer),
    )
    assert resp.status_code == 503
    detail = resp.json().get("detail", {})
    assert detail.get("code") == "KB_NOT_CONFIGURED"


@pytest.mark.parametrize(
    "bad_body",
    [
        {"workspace_root": "/tmp/x"},                     # missing text
        {"text": "x"},                                    # missing workspace_root
        {"workspace_root": "/tmp/x", "text": "x", "top_k": 0},   # top_k <= 0
        {"workspace_root": "/tmp/x", "text": "x", "top_k": 51},  # top_k > 50
    ],
)
def test_invalid_body_returns_422(
    app_with_query_deps, bearer, bad_body
) -> None:
    """Spec scenario "Invalid request body returns 422"."""
    client = TestClient(app_with_query_deps)
    resp = client.post("/kb/query", json=bad_body, headers=_auth(bearer))
    assert resp.status_code == 422, (
        f"expected 422 for body {bad_body!r}, got {resp.status_code}: {resp.text}"
    )


def test_filter_path_narrows_results(app_with_query_deps, bearer) -> None:
    """Spec scenario "filter_path narrows results in HTTP path"."""
    workspace_root = str(app_with_query_deps.state._tmp_path / "ws-fp")
    Path(workspace_root).mkdir(exist_ok=True)
    _seed_collection(
        app_with_query_deps.state.kb_backend,
        app_with_query_deps.state._spy_provider,
        workspace_root=workspace_root,
    )

    client = TestClient(app_with_query_deps)
    resp = client.post(
        "/kb/query",
        json={
            "workspace_root": workspace_root,
            "text": "x",
            "filter_path": "src/file1.py",
        },
        headers=_auth(bearer),
    )
    assert resp.status_code == 200
    hits = resp.json()["hits"]
    for hit in hits:
        assert hit["payload"]["file_path"] == "src/file1.py", (
            f"filter_path violated: hit payload={hit['payload']}"
        )


def test_bearer_required(app_with_query_deps) -> None:
    """Spec scenario "Bearer token required"."""
    client = TestClient(app_with_query_deps)
    resp = client.post(
        "/kb/query", json={"workspace_root": "/tmp/x", "text": "hi"}
    )
    assert resp.status_code == 401


def test_query_records_usage_with_module_kb_query(
    bearer, tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Spec scenario "Query usage recorded with module=kb_query".

    Wraps SpyProvider in a real TrackedProvider with
    `default_module="kb_query"` (test-only ALLOWED_INNER_TYPES relax to
    accept SpyProvider, mirroring the kb-build-production-wiring pattern).
    """
    from codebus_agent.providers import (
        LLMCallLogger,
        ProviderRole,
        TrackedProvider,
    )
    from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    monkeypatch.setattr(
        TrackedProvider,
        "ALLOWED_INNER_TYPES",
        TrackedProvider.ALLOWED_INNER_TYPES | {SpyProvider},
    )

    workspace_root = str(tmp_path / "ws-rec")
    Path(workspace_root).mkdir(exist_ok=True)
    spy = SpyProvider()
    backend = InMemoryQdrantBackend()
    _seed_collection(backend, spy, workspace_root=workspace_root)

    def _query_factory(ws: Path) -> TrackedProvider:
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
            default_module="kb_query",
        )

    app = create_app(bearer_token=bearer)
    app.state.kb_backend = backend
    app.state.kb_provider = lambda _ws: spy  # build-side, irrelevant here
    app.state.kb_query_provider = _query_factory
    app.state.kb_usage_tracker = lambda ws: UsageTracker(
        Path(ws) / "token_usage.jsonl"
    )
    app.state.kb_embedding_dim = 8

    client = TestClient(app)
    resp = client.post(
        "/kb/query",
        json={"workspace_root": workspace_root, "text": "func"},
        headers=_auth(bearer),
    )
    assert resp.status_code == 200, resp.text

    usage_path = Path(workspace_root) / "token_usage.jsonl"
    assert usage_path.exists(), f"token_usage.jsonl not at {usage_path}"
    lines = [
        json.loads(line)
        for line in usage_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    query_lines = [l for l in lines if l.get("module") == "kb_query"]
    assert query_lines, (
        f"no module='kb_query' line found; module values: "
        f"{[l.get('module') for l in lines]}"
    )
