"""Network utilities — ephemeral loopback binding.

Backs openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: FastAPI sidecar binds ephemeral loopback port
  Design:       D-local-1 (sidecar port = OS-assigned ephemeral)
"""
from __future__ import annotations

import socket

_LOOPBACK_HOST = "127.0.0.1"


def bind_ephemeral_loopback() -> tuple[socket.socket, int]:
    """Bind a TCP socket to 127.0.0.1 on an OS-chosen ephemeral port.

    Returns the (still-open) socket and the port number.  The caller owns
    the socket and MUST either hand it to the server stack (uvicorn) or
    close it.  Binding here — rather than letting uvicorn pick the port
    internally — is what lets us emit the handshake line before the HTTP
    server is fully up.
    """
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 0)
    sock.bind((_LOOPBACK_HOST, 0))
    _, port = sock.getsockname()
    return sock, port
