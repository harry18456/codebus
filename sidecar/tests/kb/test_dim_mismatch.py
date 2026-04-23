"""TDD red tests for KB dim-mismatch guard — Section 6 of
openspec/changes/kb-build-production-wiring/tasks.md.

Backs openspec/changes/kb-build-production-wiring/specs/knowledge-base/spec.md
  Requirement: KB build production dependency wiring
    Scenario: Existing collection with wrong vector dimension returns 409 KB_DIM_MISMATCH

D-032 decision 4 places the guard in `KnowledgeBase.build` so mismatch
is detected BEFORE any embed API call — ``_ensure_ready`` does payload
indices (cheap local work), then we chunk (local, free), then we call
``backend.ensure_collection`` with the provider's declared dim. A
mismatch here means the caller can fix it without burning OpenAI cost.
"""
from __future__ import annotations

from datetime import datetime, timezone

import pytest

from codebus_agent.kb.backend import KBDimMismatchError
from codebus_agent.kb.knowledge_base import KnowledgeBase
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.scanner.models import (
    ContentTypeSummary,
    FileEntry,
    ScanResult,
    ScanStats,
)

from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider


def _make_scan() -> ScanResult:
    files = [
        FileEntry(
            path=f"a{i}.py",
            size=20,
            kind="text",
            language="python",
            encoding="utf-8",
            content=f"print({i})\n",
        )
        for i in range(3)
    ]
    return ScanResult(
        workspace_root="/abs/workspace/demo",
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


async def test_kb_build_aborts_before_embed_on_dim_mismatch(
    tmp_path,
) -> None:
    """Spec scenario "Existing collection with wrong vector dimension".

    D-032 decision 4: the guard MUST fire after chunking but BEFORE
    embedding. We assert the spy never saw an `embed()` call.
    """
    backend = InMemoryQdrantBackend()
    provider = SpyProvider()
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")

    ws_root = "/abs/workspace/demo"
    kb = KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root=ws_root,
        embedding_dim=8,
    )

    # Pre-seed the backend's dim-map to a different dim than the KB is
    # constructed for. This simulates an existing collection that was
    # built against a different model.
    await backend.ensure_collection(kb.collection_name, expected_dim=16)

    scan = _make_scan()

    with pytest.raises(KBDimMismatchError) as excinfo:
        await kb.build(scan)

    # Guard MUST have fired before any embed call.
    assert provider.embed_calls == [], (
        f"provider.embed() was called before dim check! "
        f"{len(provider.embed_calls)} call(s) made"
    )

    err = excinfo.value
    assert err.expected_dim == 8
    assert err.actual_dim == 16


def test_dim_mismatch_error_event_contains_expected_and_actual() -> None:
    """Spec scenario implicit in body shape: SSE error event body.

    Confirms `_enrich_error_event` produces the spec-required fields
    (`expected_dim`, `actual_dim`, `suggestion`) when classify returns
    `KB_DIM_MISMATCH`.
    """
    from codebus_agent.api.tasks import _enrich_error_event

    err = KBDimMismatchError(
        collection="codebus_abc",
        expected_dim=1536,
        actual_dim=384,
    )
    extras = _enrich_error_event("KB_DIM_MISMATCH", err)
    assert extras["expected_dim"] == 1536
    assert extras["actual_dim"] == 384
    assert "suggestion" in extras
    assert "delete" in extras["suggestion"].lower()
