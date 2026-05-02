"""uvicorn ``access_log`` invariant — backs Decision 2 of
``sidecar-sse-bearer-query-param-fallback``.

The SSE events endpoint accepts the bearer via ``?bearer=<token>`` query
parameter because browser ``EventSource`` cannot set headers. uvicorn's
access log records the full query string by default, so leaving
``access_log=True`` would leak the bearer to stdout / log files. This
test guards the invariant by asserting that ``_serve`` always passes
``access_log=False`` to ``uvicorn.Config``.
"""
from __future__ import annotations

import socket
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from codebus_agent.api.main import _serve


@pytest.fixture
def fake_sock() -> socket.socket:
    """A real loopback socket — `_serve` opens it, but we never reach
    `server.serve` because `uvicorn.Server` is mocked.
    """
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.bind(("127.0.0.1", 0))
    yield sock
    sock.close()


async def test_access_log_disabled(fake_sock: socket.socket) -> None:
    """`_serve` MUST pass `access_log=False` to `uvicorn.Config`.

    Mocks both `uvicorn.Config` and `uvicorn.Server` so we can inspect
    the kwargs without binding the loopback or starting an event loop.
    """
    fake_server = MagicMock()
    fake_server.serve = AsyncMock(return_value=None)

    with (
        patch("codebus_agent.api.main.uvicorn.Config") as config_cls,
        patch(
            "codebus_agent.api.main.uvicorn.Server", return_value=fake_server
        ),
        patch("codebus_agent.api.main.handshake.emit"),
    ):
        port = fake_sock.getsockname()[1]
        await _serve(
            app=MagicMock(),
            sock=fake_sock,
            port=port,
            bearer="test-bearer-token",
            parent_pid=None,
        )

    config_cls.assert_called_once()
    kwargs = config_cls.call_args.kwargs
    assert kwargs.get("access_log") is False, (
        "uvicorn.Config(access_log=...) must be False — see "
        "`sidecar-sse-bearer-query-param-fallback` Decision 2; "
        "flipping to True would leak the SSE `?bearer=` query "
        "parameter into the access log."
    )
