"""TDD red tests for D2.12 — `POST /kb/build` 統一 202 status code.

Backs Requirement `POST /kb/build async endpoint` (knowledge-base
capability) and the new Scenario
`Status code aligned with sibling task endpoints`.

Strategy:
  * `test_kb_build_returns_202_accepted` — happy path on `/kb/build`
    MUST return 202 (was 200).
  * `test_all_task_endpoints_return_202_on_success` — parameterized
    sweep of the five task-spawning endpoints; every endpoint MUST
    return 202 on the success path.
"""
from __future__ import annotations

import re
import secrets
from collections.abc import Callable
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import MagicMock

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine
from codebus_agent.scanner.models import (
    ContentTypeSummary,
    FileEntry,
    ScanResult,
    ScanStats,
)


_TASK_ID_RE = re.compile(r"^[a-z]+_[0-9a-f]{8}$")
_RULES_VERSION = "2026-04-20-1"


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def _make_scan(workspace_root: str) -> dict:
    files = [
        FileEntry(
            path=f"a{i}.py",
            size=10,
            kind="text",
            language="python",
            encoding="utf-8",
            content=f"x = {i}\n",
        )
        for i in range(2)
    ]
    sr = ScanResult(
        workspace_root=workspace_root,
        scan_started_at=datetime(2026, 4, 27, 12, 0, 0, tzinfo=timezone.utc),
        scan_completed_at=datetime(2026, 4, 27, 12, 0, 1, tzinfo=timezone.utc),
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


def _make_kb_app(bearer: str, tmp_path: Path):
    """App with KB deps wired for /kb/build."""
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    app = create_app(bearer_token=bearer)
    spy = SpyProvider()
    audit_dir = tmp_path / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    shared_tracker = UsageTracker(audit_dir / "token_usage.jsonl")
    app.state.kb_backend = InMemoryQdrantBackend()
    app.state.kb_provider = lambda _ws: spy
    app.state.kb_usage_tracker = lambda _ws: shared_tracker
    app.state.kb_embedding_dim = 8
    return app


def test_kb_build_returns_202_accepted(bearer: str, tmp_path: Path) -> None:
    """D2.12 spec scenario `Successful request returns 202 with task_id immediately`.

    The endpoint MUST return HTTP 202 (Accepted), aligning with the
    convention used by `/scan?stream=true` / `/explore` / `/generate`
    / `/qa`. Pre-fix this assertion fails because the FastAPI router
    decorator omits `status_code=`, yielding the default 200.
    """
    app = _make_kb_app(bearer, tmp_path)
    client = TestClient(app)
    ws_str = str(tmp_path)
    resp = client.post(
        "/kb/build",
        json={"workspace_root": ws_str, "scan_result": _make_scan(ws_str)},
        headers=_auth(bearer),
    )
    assert resp.status_code == 202, resp.text
    body = resp.json()
    assert "task_id" in body
    assert _TASK_ID_RE.fullmatch(body["task_id"]), f"bad id {body['task_id']!r}"
    assert body["task_id"].startswith("kb_")


# ---------------------------------------------------------------------------
# D2.12 Scenario `Status code aligned with sibling task endpoints` —
# every task-spawning endpoint MUST return 202 on the success path.
# ---------------------------------------------------------------------------


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


def _make_embed_factory(module: str) -> Callable[[Path], TrackedProvider]:
    """Wrap MockProvider in TrackedProvider so `/qa` can call set_emitter."""

    def _factory(workspace_root: Path) -> TrackedProvider:
        ws = Path(workspace_root)
        audit_dir = ws / ".codebus"
        audit_dir.mkdir(parents=True, exist_ok=True)
        return TrackedProvider(
            MockProvider(script=MockScript(), role=ProviderRole.EMBED),
            tracker=UsageTracker(audit_dir / "token_usage.jsonl"),
            logger=LLMCallLogger(audit_dir / "llm_calls.jsonl"),
            role=ProviderRole.EMBED,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl"),
            rules_version=_RULES_VERSION,
            default_module=module,
        )

    return _factory


def _wire_full_app(bearer: str, tmp_path: Path):
    """Wire every dep slot used by the 5 task endpoints under one app."""
    from tests.kb.conftest import InMemoryQdrantBackend, SpyProvider

    app = create_app(bearer_token=bearer)
    audit_dir = tmp_path / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)

    # KB build path uses raw SpyProvider via /kb/build (KnowledgeBase.build
    # bypasses TrackedProvider); KB query path goes through TrackedProvider
    # because /qa calls `kb_provider.set_emitter(emitter)`.
    spy = SpyProvider()
    shared_tracker = UsageTracker(audit_dir / "token_usage.jsonl")
    app.state.kb_backend = InMemoryQdrantBackend()
    app.state.kb_provider = lambda _ws: spy
    app.state.kb_usage_tracker = lambda _ws: shared_tracker
    app.state.kb_query_provider = _make_embed_factory("kb_query")
    app.state.kb_embedding_dim = 8

    # Explore / Generate / QA chat-side
    app.state.llm_reasoning_provider = _make_chat_factory(
        ProviderRole.REASONING, "reasoning"
    )
    app.state.llm_judge_provider = _make_chat_factory(ProviderRole.JUDGE, "judge")
    app.state.llm_coverage_provider = _make_chat_factory(ProviderRole.JUDGE, "coverage")
    app.state.llm_generate_provider = _make_chat_factory(ProviderRole.CHAT, "generate")
    app.state.llm_chat_provider = _make_chat_factory(ProviderRole.CHAT, "chat")
    app.state.llm_qa_provider = _make_chat_factory(ProviderRole.CHAT, "qa_agent")

    # KB growth logger factory for /qa
    from codebus_agent.kb.growth_logger import KBGrowthLogger

    def _growth_factory(ws):
        return KBGrowthLogger(Path(ws) / ".codebus" / "kb_growth.jsonl")

    app.state.kb_growth_logger_factory = _growth_factory
    return app


def test_all_task_endpoints_return_202_on_success(
    bearer: str, tmp_path: Path
) -> None:
    """D2.12 spec scenario `Status code aligned with sibling task endpoints`.

    Parameterized sweep — each task-spawning endpoint MUST return 202
    on the happy path. Pre-fix `/kb/build` and `/scan?stream=true`
    return 200 (this test catches both at once).
    """
    app = _wire_full_app(bearer, tmp_path)
    client = TestClient(app)
    ws_str = str(tmp_path)

    cases: list[tuple[str, dict]] = [
        (
            "/scan?stream=true",
            {"workspace_type": "folder", "workspace_root": ws_str},
        ),
        (
            "/kb/build",
            {"workspace_root": ws_str, "scan_result": _make_scan(ws_str)},
        ),
        (
            "/explore",
            {
                "workspace_root": ws_str,
                "task": "trace storage",
                "budget_steps": 0,
                "budget_tokens": 1000,
            },
        ),
        (
            "/generate",
            {
                "workspace_root": ws_str,
                "task": "make a tutorial",
                "stations": [],
                "options": {"mode": "interactive", "target_persona": "x"},
            },
        ),
        (
            "/qa",
            {"workspace_root": ws_str, "question": "what does this code do?"},
        ),
    ]

    for path, body in cases:
        # Reset registry between calls — single-slot TaskRegistry. We
        # only assert the status code; we don't wait for background
        # tasks to drain. Setting `_slot = None` lets each new POST
        # acquire the slot without 409.
        app.state.tasks._slot = None
        resp = client.post(path, json=body, headers=_auth(bearer))
        assert resp.status_code == 202, (
            f"{path} MUST return 202 on success; got {resp.status_code} "
            f"({resp.text!r})"
        )
