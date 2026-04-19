"""CLI self-check tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/app-packaging/spec.md
  Requirement: Packaged binary health check
    Scenario: Healthz flag succeeds after build
    Scenario: Healthz reports degraded if optional dependency missing

Source-mode tests exercise ``python -m codebus_agent.api.main --healthz``
so the CLI contract is covered even before PyInstaller has produced a
binary.  Binary-mode tests run the packaged artifact from
``sidecar/dist/`` and are ``skipif``'d when it does not exist — so CI
can still go green on platforms where the build has not happened yet.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Iterator

import pytest

_REPO_ROOT = Path(__file__).resolve().parents[1]
_BINARY_CANDIDATES = [
    _REPO_ROOT / "dist" / "codebus-sidecar.exe",
    _REPO_ROOT / "dist" / "codebus-sidecar",
]


def _find_binary() -> Path | None:
    for candidate in _BINARY_CANDIDATES:
        if candidate.exists():
            return candidate
    return None


def _run_source_healthz(env_extra: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    if env_extra:
        env.update(env_extra)
    return subprocess.run(
        [sys.executable, "-m", "codebus_agent.api.main", "--healthz"],
        capture_output=True,
        text=True,
        env=env,
        timeout=30,
        check=False,
    )


def _parse_single_json_line(stdout: str) -> dict[str, object]:
    lines = [line for line in stdout.splitlines() if line.strip()]
    assert len(lines) == 1, f"expected one JSON line, got {len(lines)}: {stdout!r}"
    return json.loads(lines[0])


class _ReadyzHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 — stdlib override
        if self.path.rstrip("/") == "/readyz":
            self.send_response(200)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(404)
        self.end_headers()

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002
        return  # silence request log


@contextmanager
def _stub_qdrant_ready() -> Iterator[str]:
    server = HTTPServer(("127.0.0.1", 0), _ReadyzHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        host, port = server.server_address
        yield f"http://{host}:{port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def test_source_healthz_ok_when_qdrant_reachable() -> None:
    """Scenario: Healthz flag succeeds after build.

    A local ``http.server`` stub answers ``/readyz`` with 200 so the
    packaged self-check observes a healthy Qdrant without requiring a
    real vector DB in CI.
    """
    with _stub_qdrant_ready() as stub_url:
        result = _run_source_healthz(env_extra={"CODEBUS_QDRANT_URL": stub_url})

    assert result.returncode == 0, f"stderr: {result.stderr!r}"
    report = _parse_single_json_line(result.stdout)
    assert report["status"] == "ok"
    assert report["dependencies"]["qdrant"]["ok"] is True  # type: ignore[index]


def test_source_healthz_degraded_when_qdrant_unreachable() -> None:
    """Scenario: Healthz reports degraded if optional dependency missing.

    Port 1 on loopback is an unprivileged-reserved port that nothing
    listens on — the connect attempt fails fast without relying on
    resolver behaviour.
    """
    result = _run_source_healthz(env_extra={"CODEBUS_QDRANT_URL": "http://127.0.0.1:1"})
    assert result.returncode == 0, f"stderr: {result.stderr!r}"
    report = _parse_single_json_line(result.stdout)
    assert report["status"] == "degraded"
    assert "qdrant" in report["dependencies"]  # type: ignore[operator]
    qdrant = report["dependencies"]["qdrant"]  # type: ignore[index]
    assert qdrant["ok"] is False


def test_source_healthz_returns_zero_on_either_status() -> None:
    """Exit code MUST be 0 regardless of dependency outcome (CI uses the
    ``status`` field, not the exit code, to tell 'binary crashed' apart
    from 'Qdrant not running yet')."""
    result = _run_source_healthz(env_extra={"CODEBUS_QDRANT_URL": "http://127.0.0.1:1"})
    assert result.returncode == 0
    report = _parse_single_json_line(result.stdout)
    assert report["status"] in {"ok", "degraded"}


@pytest.mark.skipif(_find_binary() is None, reason="packaged binary not built yet")
def test_binary_healthz_ok_when_qdrant_reachable() -> None:
    """Scenario: Healthz flag succeeds after build (packaged artifact)."""
    binary = _find_binary()
    assert binary is not None
    with _stub_qdrant_ready() as stub_url:
        env = os.environ.copy()
        env["CODEBUS_QDRANT_URL"] = stub_url
        result = subprocess.run(
            [str(binary), "--healthz"],
            capture_output=True,
            text=True,
            env=env,
            timeout=30,
            check=False,
        )
    assert result.returncode == 0, f"stderr: {result.stderr!r}"
    report = _parse_single_json_line(result.stdout)
    assert report["status"] == "ok"


@pytest.mark.skipif(_find_binary() is None, reason="packaged binary not built yet")
def test_binary_healthz_degraded_when_qdrant_unreachable() -> None:
    """Scenario: Healthz reports degraded if optional dependency missing.

    Runs the PyInstaller artifact directly — this is the canonical
    contract the M1 packaging spec promises.
    """
    binary = _find_binary()
    assert binary is not None  # for type-checkers; skipif guards runtime
    env = os.environ.copy()
    env["CODEBUS_QDRANT_URL"] = "http://127.0.0.1:1"
    result = subprocess.run(
        [str(binary), "--healthz"],
        capture_output=True,
        text=True,
        env=env,
        timeout=30,
        check=False,
    )
    assert result.returncode == 0, f"stderr: {result.stderr!r}"
    report = _parse_single_json_line(result.stdout)
    assert report["status"] == "degraded"
    assert "qdrant" in report["dependencies"]  # type: ignore[operator]
