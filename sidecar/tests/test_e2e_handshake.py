"""End-to-end sidecar handshake test — backs SHALL clause
  Scenario: Parent reads handshake and succeeds ping

Spawns the sidecar in a subprocess, reads the stdout handshake line,
then issues GET /healthz with the supplied bearer.
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


@pytest.mark.slow
def test_parent_reads_handshake_and_succeeds_ping() -> None:
    repo_src = Path(__file__).resolve().parent.parent / "src"
    assert (repo_src / "codebus_agent" / "api" / "main.py").exists(), \
        "expected codebus_agent.api.main module to exist"

    proc = subprocess.Popen(
        [sys.executable, "-m", "codebus_agent.api.main"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={**os.environ, "PYTHONUNBUFFERED": "1"},
        cwd=str(repo_src.parent),
        text=True,
        bufsize=1,
    )
    try:
        deadline = time.monotonic() + SIDECAR_BOOT_TIMEOUT_S
        first_line = ""
        while time.monotonic() < deadline:
            line = proc.stdout.readline() if proc.stdout else ""
            if line:
                first_line = line.strip()
                break
            if proc.poll() is not None:
                stderr = proc.stderr.read() if proc.stderr else ""
                pytest.fail(f"sidecar exited early: rc={proc.returncode} stderr={stderr!r}")
        assert first_line, "no handshake line observed within boot timeout"

        payload = json.loads(first_line)
        port = payload["port"]
        bearer = payload["bearer"]
        assert isinstance(port, int) and 1 <= port <= 65535
        assert isinstance(bearer, str) and len(bearer) >= 32

        url = f"http://127.0.0.1:{port}/healthz"
        headers = {"Authorization": f"Bearer {bearer}"}

        last_err: Exception | None = None
        ping_deadline = time.monotonic() + HEALTHZ_TIMEOUT_S
        while time.monotonic() < ping_deadline:
            try:
                response = httpx.get(url, headers=headers, timeout=2.0)
                break
            except httpx.ConnectError as exc:
                last_err = exc
                time.sleep(0.1)
        else:
            pytest.fail(f"could not reach sidecar /healthz: {last_err!r}")

        assert response.status_code == 200
        # qdrant-lifecycle-bootstrap: /healthz now reflects live Qdrant
        # connectivity. CI may or may not have a Qdrant running, so the
        # e2e contract is "sidecar is reachable and answers", not "all
        # deps are ok". ``status`` will be ``degraded`` when Qdrant is
        # down — that is the design「degraded-but-alive」path, not a bug.
        assert response.json()["status"] in {"ok", "degraded"}
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5.0)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
