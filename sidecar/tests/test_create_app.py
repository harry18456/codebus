"""``create_app`` lifecycle tests — backs SHALL clauses in
openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
  Requirement: Async Qdrant client lifecycle bound to FastAPI app

These tests pin the four scenarios under "Async Qdrant client lifecycle":
client attached when URL is provided, absent when omitted, construction
is non-blocking (degraded-but-alive, per design), and ``close()`` fires
exactly once on shutdown. The paired /healthz connectivity cases live
in ``tests/test_healthz.py`` alongside existing healthz fixtures.
"""
from __future__ import annotations

import secrets
import time
from unittest.mock import AsyncMock

import pytest
from fastapi.testclient import TestClient
from qdrant_client import AsyncQdrantClient

from codebus_agent.api import create_app


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


class TestQdrantClientLifecycle:
    def test_no_client_when_url_omitted(self, bearer: str) -> None:
        """Scenario: No client when URL is omitted."""
        app = create_app(bearer_token=bearer)
        assert getattr(app.state, "qdrant_client", None) is None

    def test_client_attached_when_url_provided(self, bearer: str) -> None:
        """Scenario: Client attached when URL is provided."""
        app = create_app(bearer_token=bearer, qdrant_url="http://127.0.0.1:6333")
        try:
            assert isinstance(app.state.qdrant_client, AsyncQdrantClient)
        finally:
            # Drive the shutdown hook so we don't leak the client on assert-fail paths.
            with TestClient(app):
                pass

    def test_construction_is_non_blocking(self, bearer: str) -> None:
        """Scenario: Construction is non-blocking.

        Design「Startup policy：degraded-but-alive」— even with Qdrant
        unreachable, ``create_app`` must not stall waiting on TCP.
        """
        start = time.monotonic()
        app = create_app(bearer_token=bearer, qdrant_url="http://127.0.0.1:1")
        elapsed = time.monotonic() - start
        try:
            assert elapsed < 1.0, (
                f"create_app took {elapsed:.3f}s with unreachable Qdrant — "
                "construction must not perform network I/O"
            )
        finally:
            with TestClient(app):
                pass

    def test_client_closed_exactly_once_on_shutdown(self, bearer: str) -> None:
        """Scenario: Client closed on app shutdown.

        We replace ``app.state.qdrant_client`` with an ``AsyncMock`` so we
        can assert ``close()`` is invoked exactly once during the
        FastAPI lifespan shutdown phase (TestClient context-exit drives it).
        """
        app = create_app(bearer_token=bearer, qdrant_url="http://127.0.0.1:6333")

        # Close the real client the factory constructed, then swap in a mock.
        real = app.state.qdrant_client
        stub = AsyncMock(spec=AsyncQdrantClient)
        app.state.qdrant_client = stub

        with TestClient(app) as client:
            client.get("/healthz", headers=_auth(bearer))
        # Close the real client we displaced so it doesn't leak sockets.
        import asyncio

        asyncio.run(real.close())

        stub.close.assert_awaited_once()
