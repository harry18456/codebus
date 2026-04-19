"""Shared fixtures for provider tests.

Implements the M1 invariant `No outbound LLM traffic during M1`
(openspec/changes/m1-power-on/specs/llm-provider/spec.md) by
intercepting socket.socket.connect at test time.  Tests opt in by
requesting the `block_outbound_sockets` fixture.
"""
from __future__ import annotations

import socket
from collections.abc import Iterator
from typing import Any

import pytest

_LOOPBACK_HOSTS = {"127.0.0.1", "::1", "localhost", ""}


@pytest.fixture
def block_outbound_sockets(monkeypatch: pytest.MonkeyPatch) -> Iterator[list[Any]]:
    """Record + block any non-loopback socket.connect.

    Returns a list that tests can assert is empty to prove no outbound
    traffic left the process during the test body.
    """
    blocked: list[Any] = []
    original_connect = socket.socket.connect

    def guarded_connect(self: socket.socket, address: Any) -> None:
        host = address[0] if isinstance(address, tuple) else str(address)
        if host in _LOOPBACK_HOSTS:
            original_connect(self, address)
            return
        blocked.append(address)
        raise RuntimeError(
            f"M1 invariant breach: outbound socket connect to {address!r} blocked"
        )

    monkeypatch.setattr(socket.socket, "connect", guarded_connect)
    yield blocked
