"""Handshake-line tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Handshake via stdout first line
"""
from __future__ import annotations

import io
import json
import secrets

import pytest

from codebus_agent.handshake import build_line, emit


def test_handshake_line_is_valid_json_with_port_and_bearer() -> None:
    """Scenario: Handshake line format.

    First stdout line MUST be valid JSON with an integer ``port`` and a
    ``bearer`` string of at least 32 characters.
    """
    bearer = secrets.token_urlsafe(32)
    line = build_line(port=51734, bearer=bearer)
    payload = json.loads(line)

    assert isinstance(payload["port"], int)
    assert payload["port"] == 51734
    assert isinstance(payload["bearer"], str)
    assert len(payload["bearer"]) >= 32


def test_handshake_emit_writes_single_terminated_line() -> None:
    """emit must write exactly one line ending in '\\n' and flush the
    stream — a parent process reading one line (not the whole buffer)
    needs the newline terminator and the flush to arrive in order."""
    bearer = secrets.token_urlsafe(32)
    buf = io.StringIO()
    emit(port=40001, bearer=bearer, stream=buf)
    contents = buf.getvalue()

    assert contents.endswith("\n")
    assert contents.count("\n") == 1


@pytest.mark.parametrize("bad_port", [0, -1, 65536, 70000])
def test_build_line_rejects_invalid_ports(bad_port: int) -> None:
    """Audit lens: a 0 or >65535 port slipping in would make the parent
    ping a nonsense address.  Catch it at the handshake boundary."""
    bearer = secrets.token_urlsafe(32)
    with pytest.raises(ValueError):
        build_line(port=bad_port, bearer=bearer)


def test_build_line_rejects_short_bearer() -> None:
    """The spec floor is 32 characters; shorter must be refused so we
    cannot accidentally ship a weak token."""
    with pytest.raises(ValueError):
        build_line(port=40001, bearer="too-short")
