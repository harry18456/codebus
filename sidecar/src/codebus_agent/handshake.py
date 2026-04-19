"""Stdout handshake line — how the Tauri parent learns our port + bearer.

Backs openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Handshake via stdout first line
  Design:       D-local-1 (parent reads first stdout line at startup)

The format is a single-line JSON object ending in ``\\n``.  Anything the
sidecar prints afterwards is logging — the parent MUST only read the
first line for discovery.
"""
from __future__ import annotations

import json
import sys
from typing import TextIO

_MIN_BEARER_LEN = 32


def build_line(port: int, bearer: str) -> str:
    """Return the handshake line without terminator.

    Validates inputs so malformed data cannot leave the sidecar in a
    state where the parent pings a bad endpoint.
    """
    if not isinstance(port, int) or port <= 0 or port > 65535:
        raise ValueError(f"port must be 1..65535, got {port!r}")
    if not isinstance(bearer, str) or len(bearer) < _MIN_BEARER_LEN:
        raise ValueError(
            f"bearer must be a string of length ≥ {_MIN_BEARER_LEN}, "
            f"got length {len(bearer) if isinstance(bearer, str) else 'non-string'}"
        )
    return json.dumps({"port": port, "bearer": bearer}, separators=(",", ":"))


def emit(port: int, bearer: str, stream: TextIO | None = None) -> None:
    """Write the handshake line + newline to ``stream`` and flush.

    Flush is essential: without it the parent may block reading stdout
    while our buffer sits in Python's IO layer.
    """
    target = stream if stream is not None else sys.stdout
    target.write(build_line(port, bearer) + "\n")
    target.flush()
