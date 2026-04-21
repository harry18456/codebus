"""TDD tests for ``codebus_agent.kb.qdrant_client``.

Backs openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md

Sections are kept in the same order as ``tasks.md`` so that each
``uv run pytest -k <marker>`` run matches one task group.
"""
from __future__ import annotations

import asyncio
import socket
import threading
import time
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Iterator

import pytest
from qdrant_client import AsyncQdrantClient

from codebus_agent.kb import qdrant_client


class _StubHandler(BaseHTTPRequestHandler):
    """Answer ``GET /readyz`` with a configurable status code."""

    status_code: int = 200

    def do_GET(self) -> None:  # noqa: N802 — stdlib override
        if self.path.rstrip("/") == "/readyz":
            self.send_response(self.__class__.status_code)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(404)
        self.end_headers()

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002
        return


def _make_handler(status: int) -> type[_StubHandler]:
    return type(
        f"StubHandler{status}",
        (_StubHandler,),
        {"status_code": status},
    )


@contextmanager
def _stub_server(status: int = 200) -> Iterator[str]:
    server = HTTPServer(("127.0.0.1", 0), _make_handler(status))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        host, port = server.server_address
        yield f"http://{host}:{port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def _unbound_loopback_url() -> str:
    """Bind, read the port, close — the port is now guaranteed unreachable."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    return f"http://127.0.0.1:{port}"


# ---------------------------------------------------------------------------
# Requirement: CODEBUS_QDRANT_URL resolution has a single source of truth
# ---------------------------------------------------------------------------


class TestResolveUrl:
    def test_explicit_argument_wins_over_environment(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        monkeypatch.setenv("CODEBUS_QDRANT_URL", "http://env.invalid:9000")
        assert (
            qdrant_client.resolve_url("http://override.invalid:7000")
            == "http://override.invalid:7000"
        )

    def test_environment_used_when_no_override(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        monkeypatch.setenv("CODEBUS_QDRANT_URL", "http://env.invalid:9000")
        assert qdrant_client.resolve_url() == "http://env.invalid:9000"

    def test_default_when_nothing_configured(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        monkeypatch.delenv("CODEBUS_QDRANT_URL", raising=False)
        assert qdrant_client.resolve_url() == "http://127.0.0.1:6333"


# ---------------------------------------------------------------------------
# Requirement: Qdrant connection probe
# ---------------------------------------------------------------------------


class TestProbe:
    def test_reachable_readyz_returns_ok(self) -> None:
        """Scenario: Reachable Qdrant reports ok."""
        with _stub_server(status=200) as url:
            status = qdrant_client.probe(url)
        assert status.ok is True
        assert status.detail == url

    def test_unreachable_port_returns_not_ok_without_raising(self) -> None:
        """Scenario: Unreachable Qdrant reports degraded without raising.

        detail MUST include both the URL and the exception *type* name
        (e.g. ``URLError``), but never the exception's ``str()`` body —
        that invariant is pinned by ``test_detail_never_leaks_exception_message``.
        """
        url = _unbound_loopback_url()
        status = qdrant_client.probe(url, timeout_seconds=1.0)
        assert status.ok is False
        assert url in status.detail
        # At least one of the catch-able exception types must surface by name.
        assert any(
            name in status.detail
            for name in ("URLError", "ConnectionError", "OSError", "TimeoutError")
        ), f"expected exception type name in detail, got: {status.detail!r}"

    def test_non_200_response_returns_not_ok(self) -> None:
        """Scenario: Non-200 response reported as not ok."""
        with _stub_server(status=503) as url:
            status = qdrant_client.probe(url)
        assert status.ok is False

    def test_probe_never_raises_on_network_failure(self) -> None:
        """Probe MUST NOT raise — it is a status function, not a control flow hook."""
        url = _unbound_loopback_url()
        try:
            qdrant_client.probe(url, timeout_seconds=1.0)
        except Exception as exc:  # pragma: no cover — the assertion is the test
            pytest.fail(f"probe raised unexpectedly: {exc!r}")

    def test_detail_never_leaks_exception_message(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Scenario: Probe detail never leaks exception message.

        Design note: Probe 失敗不 log 原始 exception — detail carries the
        exception *type* name for diagnosability but never its ``str()``
        body (which may embed local paths or other host-side sentinels).
        """
        sentinel = "SECRET-EXCEPTION-BODY-THAT-MUST-NOT-LEAK"

        class _LeakyError(OSError):
            def __str__(self) -> str:  # noqa: D401 — test-only override
                return sentinel

        def _raise(*_args: object, **_kwargs: object) -> object:
            raise _LeakyError(sentinel)

        monkeypatch.setattr(qdrant_client, "urlopen", _raise, raising=False)
        import urllib.request as _urllib_request

        monkeypatch.setattr(_urllib_request, "urlopen", _raise)

        url = "http://127.0.0.1:1"
        status = qdrant_client.probe(url, timeout_seconds=0.1)
        assert status.ok is False
        assert sentinel not in status.detail


# ---------------------------------------------------------------------------
# Requirement: Async Qdrant client lifecycle bound to FastAPI app
# (build_client half — create_app scenarios live in tests/test_create_app.py)
# ---------------------------------------------------------------------------


class TestBuildClient:
    def test_returns_async_qdrant_client(self) -> None:
        client = qdrant_client.build_client("http://127.0.0.1:6333")
        try:
            assert isinstance(client, AsyncQdrantClient)
        finally:
            asyncio.run(client.close())

    def test_construction_does_no_network_io(self) -> None:
        """Design: Client 生命週期 — construction non-blocking.

        build_client against an unbound loopback port MUST return well
        within one second; if the SDK eagerly opened a TCP connection
        we would see either a raise or a multi-second block.
        """
        url = _unbound_loopback_url()
        start = time.monotonic()
        client = qdrant_client.build_client(url)
        elapsed = time.monotonic() - start
        try:
            assert elapsed < 1.0, f"build_client took {elapsed:.3f}s — looks blocking"
        finally:
            asyncio.run(client.close())

    def test_close_does_not_raise(self) -> None:
        client = qdrant_client.build_client("http://127.0.0.1:6333")
        asyncio.run(client.close())


# ---------------------------------------------------------------------------
# Requirement: Idempotent collection provisioning
# ---------------------------------------------------------------------------


class _StubCollectionInfo:
    """Duck-typed stand-in for ``CollectionInfo.config.params.vectors``.

    ``ensure_collection`` only reads ``.config.params.vectors.size`` and
    ``.config.params.vectors.distance`` — we don't need to stand up the
    whole pydantic hierarchy.
    """

    def __init__(self, size: int, distance: str) -> None:
        vectors = type("V", (), {"size": size, "distance": distance})()
        params = type("P", (), {"vectors": vectors})()
        self.config = type("C", (), {"params": params})()


class TestEnsureCollection:
    @staticmethod
    def _collection_missing_error() -> Exception:
        """Mimic the SDK's 404 behaviour — ``UnexpectedResponse`` with a 404."""
        from qdrant_client.http.exceptions import UnexpectedResponse

        # Construct via the standard 404-ish shape; the wrapper SHALL treat
        # any ``UnexpectedResponse`` / ``ValueError`` as "not found".
        try:
            return UnexpectedResponse(
                status_code=404,
                reason_phrase="Not Found",
                content=b"",
                headers=None,
            )
        except TypeError:
            # Some SDK minor versions have a simpler ctor; fall back.
            return ValueError("Collection not found")

    async def test_creates_collection_when_absent(self) -> None:
        from unittest.mock import AsyncMock

        client = AsyncMock(spec=qdrant_client.AsyncQdrantClient)
        client.get_collection.side_effect = self._collection_missing_error()

        await qdrant_client.ensure_collection(
            client, name="codebus_demo", vector_size=8
        )

        client.create_collection.assert_awaited_once()
        kwargs = client.create_collection.await_args.kwargs
        assert kwargs.get("collection_name") == "codebus_demo"

    async def test_noop_when_collection_matches(self) -> None:
        from unittest.mock import AsyncMock

        client = AsyncMock(spec=qdrant_client.AsyncQdrantClient)
        client.get_collection.return_value = _StubCollectionInfo(
            size=8, distance="Cosine"
        )

        await qdrant_client.ensure_collection(
            client, name="codebus_demo", vector_size=8
        )

        client.create_collection.assert_not_awaited()
        client.delete_collection.assert_not_awaited()

    async def test_schema_mismatch_raises_without_destroying_data(self) -> None:
        from unittest.mock import AsyncMock

        client = AsyncMock(spec=qdrant_client.AsyncQdrantClient)
        # Existing collection has size 8 but caller asks for 16.
        client.get_collection.return_value = _StubCollectionInfo(
            size=8, distance="Cosine"
        )

        with pytest.raises(qdrant_client.QdrantCollectionSchemaError):
            await qdrant_client.ensure_collection(
                client, name="codebus_demo", vector_size=16
            )

        client.create_collection.assert_not_awaited()
        client.delete_collection.assert_not_awaited()

    async def test_distance_mismatch_raises(self) -> None:
        """Schema equality spans BOTH vector size and distance — a
        silent distance swap would corrupt kNN semantics."""
        from unittest.mock import AsyncMock

        client = AsyncMock(spec=qdrant_client.AsyncQdrantClient)
        client.get_collection.return_value = _StubCollectionInfo(
            size=8, distance="Euclid"
        )

        with pytest.raises(qdrant_client.QdrantCollectionSchemaError):
            await qdrant_client.ensure_collection(
                client, name="codebus_demo", vector_size=8, distance="Cosine"
            )

        client.create_collection.assert_not_awaited()

    async def test_no_payload_index_api_touched(self) -> None:
        """Design「`ensure_collection` 不做 payload index」— payload indices
        belong to Module 2's build pipeline, not this low-level helper.
        """
        from unittest.mock import AsyncMock

        client = AsyncMock(spec=qdrant_client.AsyncQdrantClient)
        client.get_collection.side_effect = self._collection_missing_error()

        await qdrant_client.ensure_collection(
            client, name="codebus_demo", vector_size=8
        )

        client.create_payload_index.assert_not_awaited()
