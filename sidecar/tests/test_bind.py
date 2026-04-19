"""Ephemeral loopback bind tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: FastAPI sidecar binds ephemeral loopback port
"""
from __future__ import annotations

import socket

from codebus_agent.net import bind_ephemeral_loopback


def test_successive_binds_give_distinct_ports() -> None:
    """Scenario: Random port chosen at startup."""
    sock_a, port_a = bind_ephemeral_loopback()
    sock_b, port_b = bind_ephemeral_loopback()
    try:
        assert port_a != port_b, "two consecutive binds must produce distinct ports"
        assert 1024 <= port_a <= 65535
        assert 1024 <= port_b <= 65535
    finally:
        sock_a.close()
        sock_b.close()


def test_bind_is_strictly_loopback() -> None:
    """Scenario: Not reachable from non-loopback interfaces.

    We cannot portably assert the kernel-level unreachability from a
    non-loopback interface inside a unit test (requires multiple NICs).
    The operative invariant is that the bound address is strictly
    127.0.0.1 — binding to 0.0.0.0 would expose the port on every
    interface, which is exactly what this requirement forbids.
    """
    sock, _ = bind_ephemeral_loopback()
    try:
        bound_host, _ = sock.getsockname()
        assert bound_host == "127.0.0.1", (
            f"sidecar must bind strictly to loopback, got {bound_host!r}"
        )
        # Must be AF_INET + SOCK_STREAM (TCP); anything else breaks uvicorn wiring.
        assert sock.family == socket.AF_INET
        assert sock.type == socket.SOCK_STREAM
    finally:
        sock.close()
