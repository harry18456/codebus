"""Tests for `POST /qa` endpoint.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/sidecar-runtime/spec.md
  Requirement: Q&A task spawn endpoint
  Requirement: Background task error containment (QA_FAILED)
"""
from __future__ import annotations

import asyncio
import json
import re
import secrets
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app


def _bearer() -> str:
    return secrets.token_hex(32)


def _make_app(*, with_qa_growth: bool = True, tmp_path: Path | None = None):
    """Construct an app with manually injected mocks for the /qa deps.

    `create_app` only wires KB factories when openai_api_key is set, but
    those factories try to construct real OpenAI providers. For unit
    tests, we build the app sans openai_api_key and then plant mocks.
    """
    bearer = _bearer()
    app = create_app(bearer)

    workspace = tmp_path / "ws" if tmp_path else Path.cwd()
    if tmp_path:
        workspace.mkdir(exist_ok=True)

    # Wire mock factories for Q&A deps.
    fake_provider = MagicMock()
    fake_provider.set_emitter = MagicMock()

    def _provider_factory(_ws):
        return fake_provider

    def _tracker_factory(_ws):
        tracker = MagicMock()
        tracker.record = MagicMock()
        return tracker

    def _kb_growth_factory(ws):
        # Minimal KBGrowthLogger stub
        from codebus_agent.kb.growth_logger import KBGrowthLogger
        return KBGrowthLogger(Path(ws) / ".codebus" / "kb_growth.jsonl")

    app.state.kb_provider = _provider_factory
    app.state.kb_query_provider = _provider_factory
    app.state.kb_usage_tracker = _tracker_factory
    app.state.kb_embedding_dim = 8
    app.state.kb_backend = MagicMock()
    app.state.llm_chat_provider = _provider_factory
    app.state.llm_judge_provider = _provider_factory
    app.state.llm_qa_provider = _provider_factory
    if with_qa_growth:
        app.state.kb_growth_logger_factory = _kb_growth_factory
    else:
        app.state.kb_growth_logger_factory = None

    return app, bearer, workspace


def test_empty_question_returns_422(tmp_path: Path) -> None:
    app, bearer, ws = _make_app(tmp_path=tmp_path)
    client = TestClient(app)
    r = client.post(
        "/qa",
        headers={"Authorization": f"Bearer {bearer}"},
        json={"workspace_root": str(ws), "question": ""},
    )
    assert r.status_code == 422
    body = r.text
    assert "question" in body


def test_oversize_question_returns_422(tmp_path: Path) -> None:
    app, bearer, ws = _make_app(tmp_path=tmp_path)
    client = TestClient(app)
    r = client.post(
        "/qa",
        headers={"Authorization": f"Bearer {bearer}"},
        json={"workspace_root": str(ws), "question": "x" * 4001},
    )
    assert r.status_code == 422


def test_invalid_originating_station_id_returns_422(tmp_path: Path) -> None:
    app, bearer, ws = _make_app(tmp_path=tmp_path)
    client = TestClient(app)
    r = client.post(
        "/qa",
        headers={"Authorization": f"Bearer {bearer}"},
        json={
            "workspace_root": str(ws),
            "question": "ok",
            "originating_station_id": "bad",
        },
    )
    assert r.status_code == 422


def test_missing_dependency_returns_503_with_detail(tmp_path: Path) -> None:
    app, bearer, ws = _make_app(with_qa_growth=False, tmp_path=tmp_path)
    client = TestClient(app)
    r = client.post(
        "/qa",
        headers={"Authorization": f"Bearer {bearer}"},
        json={"workspace_root": str(ws), "question": "hello world"},
    )
    assert r.status_code == 503
    body = r.json()
    assert body["detail"]["code"] == "QA_NOT_CONFIGURED"
    assert "kb_growth_logger_factory" in body["detail"]["detail"]


def test_question_text_never_echoed_in_safe_message_table() -> None:
    """Safe error message MUST NOT carry user question content."""
    from codebus_agent.api.tasks import _safe_error_message

    msg = _safe_error_message("QA_FAILED")
    assert "question" not in msg.lower() or "Q&A" in msg
    assert "secret payload" not in msg


def test_qa_endpoint_router_registered() -> None:
    """`/qa` is wired in the FastAPI app's router list."""
    bearer = _bearer()
    app = create_app(bearer)
    routes = [r.path for r in app.routes]
    assert "/qa" in routes
