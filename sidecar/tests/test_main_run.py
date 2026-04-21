"""Sidecar entry-point tests — backs SHALL clauses in
openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
  Requirement: Sidecar entry point wires Qdrant URL into app factory
  Requirement: Sidecar startup remains available when Qdrant is unreachable

These exercise the full ``python -m codebus_agent.api.main`` spawn path
and assert that:
  - ``CODEBUS_QDRANT_URL`` is threaded through to the runtime probe,
  - the default URL (``http://127.0.0.1:6333``) is used when env is unset,
  - an unreachable Qdrant does NOT block startup (design「degraded-but-
    alive」) — handshake still lands inside the 3s budget and
    ``/healthz`` returns 200 with ``status=degraded``.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

import httpx
import pytest

SIDECAR_BOOT_TIMEOUT_S = 15.0
HEALTHZ_TIMEOUT_S = 5.0
DEGRADED_HANDSHAKE_BUDGET_S = 3.0


def _repo_src() -> Path:
    return Path(__file__).resolve().parent.parent / "src"


def _spawn_sidecar(env: dict[str, str]) -> subprocess.Popen[str]:
    repo_src = _repo_src()
    return subprocess.Popen(
        [sys.executable, "-m", "codebus_agent.api.main"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        cwd=str(repo_src.parent),
        text=True,
        bufsize=1,
    )


def _read_handshake(
    proc: subprocess.Popen[str], deadline_s: float
) -> tuple[dict[str, object], float]:
    """Return (payload, elapsed_seconds). Fails the test if the handshake
    doesn't arrive inside ``deadline_s``."""
    start = time.monotonic()
    deadline = start + deadline_s
    first_line = ""
    while time.monotonic() < deadline:
        line = proc.stdout.readline() if proc.stdout else ""
        if line:
            first_line = line.strip()
            break
        if proc.poll() is not None:
            stderr = proc.stderr.read() if proc.stderr else ""
            pytest.fail(f"sidecar exited early: rc={proc.returncode} stderr={stderr!r}")
    if not first_line:
        pytest.fail("no handshake line observed before deadline")
    return json.loads(first_line), time.monotonic() - start


def _fetch_healthz(port: int, bearer: str) -> httpx.Response:
    url = f"http://127.0.0.1:{port}/healthz"
    headers = {"Authorization": f"Bearer {bearer}"}
    last_err: Exception | None = None
    deadline = time.monotonic() + HEALTHZ_TIMEOUT_S
    while time.monotonic() < deadline:
        try:
            return httpx.get(url, headers=headers, timeout=2.0)
        except httpx.ConnectError as exc:
            last_err = exc
            time.sleep(0.1)
    pytest.fail(f"could not reach sidecar /healthz: {last_err!r}")


def _terminate(proc: subprocess.Popen[str]) -> None:
    proc.terminate()
    try:
        proc.wait(timeout=5.0)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()


@pytest.mark.slow
def test_env_qdrant_url_threads_through_to_runtime_probe() -> None:
    """Scenario: CLI / runtime share the same resolver.

    When ``CODEBUS_QDRANT_URL`` is set, the runtime ``/healthz`` must
    surface that host in ``dependencies.qdrant.detail`` — proving the
    entry point delegated to ``kb.qdrant_client.resolve_url``.
    """
    sentinel_host = "custom.invalid"
    sentinel_port = 7000
    env = {
        **os.environ,
        "PYTHONUNBUFFERED": "1",
        "CODEBUS_QDRANT_URL": f"http://{sentinel_host}:{sentinel_port}",
    }
    proc = _spawn_sidecar(env)
    try:
        payload, _elapsed = _read_handshake(proc, SIDECAR_BOOT_TIMEOUT_S)
        response = _fetch_healthz(port=payload["port"], bearer=payload["bearer"])  # type: ignore[arg-type]
        assert response.status_code == 200
        body = response.json()
        qdrant = body["dependencies"]["qdrant"]
        assert sentinel_host in str(qdrant["detail"])
        assert str(sentinel_port) in str(qdrant["detail"])
    finally:
        _terminate(proc)


@pytest.mark.slow
def test_default_qdrant_url_used_when_env_unset() -> None:
    """Scenario: Default URL when nothing configured.

    Without ``CODEBUS_QDRANT_URL``, the runtime probe must target
    ``127.0.0.1:6333`` per ``resolve_url`` default.
    """
    env = {k: v for k, v in os.environ.items() if k != "CODEBUS_QDRANT_URL"}
    env["PYTHONUNBUFFERED"] = "1"
    proc = _spawn_sidecar(env)
    try:
        payload, _elapsed = _read_handshake(proc, SIDECAR_BOOT_TIMEOUT_S)
        response = _fetch_healthz(port=payload["port"], bearer=payload["bearer"])  # type: ignore[arg-type]
        assert response.status_code == 200
        body = response.json()
        qdrant = body["dependencies"]["qdrant"]
        assert "127.0.0.1:6333" in str(qdrant["detail"])
    finally:
        _terminate(proc)


@pytest.mark.slow
def test_startup_remains_available_when_qdrant_unreachable() -> None:
    """Scenario: Sidecar starts and /healthz reports degraded.

    Design「Startup policy：degraded-but-alive」— an unreachable Qdrant
    must NOT block the handshake (budget: 3 s from spawn→stdout) and
    ``/healthz`` must answer 200 with ``status=degraded``.
    """
    env = {
        **os.environ,
        "PYTHONUNBUFFERED": "1",
        "CODEBUS_QDRANT_URL": "http://127.0.0.1:1",
    }
    proc = _spawn_sidecar(env)
    try:
        payload, elapsed = _read_handshake(proc, SIDECAR_BOOT_TIMEOUT_S)
        assert elapsed < DEGRADED_HANDSHAKE_BUDGET_S, (
            f"handshake took {elapsed:.2f}s — startup must not block on Qdrant"
        )
        response = _fetch_healthz(port=payload["port"], bearer=payload["bearer"])  # type: ignore[arg-type]
        assert response.status_code == 200
        body = response.json()
        assert body["status"] == "degraded"
        assert body["dependencies"]["qdrant"]["ok"] is False
    finally:
        _terminate(proc)
