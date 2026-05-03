"""Parent-process watchdog tests — backs SHALL clause in
openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Parent-process watchdog
"""
from __future__ import annotations

import asyncio
import json
import os
import signal
import socket
import subprocess
import sys
import time

import pytest

from codebus_agent.watchdog import supervise_parent, watch_parent

WATCHDOG_LIMIT_S = 5.0


async def test_watch_parent_returns_when_pid_dies() -> None:
    """Unit-level: the watchdog coroutine resolves once pid_exists(pid)
    returns False.  Uses a short-lived subprocess as a stand-in parent.
    """
    helper = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(1)"],
    )
    helper_pid = helper.pid
    helper.wait()
    assert helper.poll() is not None, "helper must already have exited"

    # With the pid already gone, watch_parent must return almost
    # immediately — certainly within the 5 s spec budget.
    start = time.monotonic()
    await asyncio.wait_for(
        watch_parent(parent_pid=helper_pid, poll_interval_s=0.1),
        timeout=2.0,
    )
    elapsed = time.monotonic() - start
    assert elapsed < 2.0


async def test_supervise_parent_invokes_on_exit_before_os_exit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario: Tauri parent exit triggers child termination via watchdog.

    `supervise_parent` MUST call the on_exit hook before invoking
    `os._exit(0)` so the Qdrant child is terminated before the
    sidecar process disappears (Decision 3: 三層 cleanup).
    """
    helper = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(0.2)"],
    )
    helper_pid = helper.pid
    helper.wait()

    call_order: list[str] = []

    def _on_exit() -> None:
        call_order.append("on_exit")

    fake_exit_called = []

    def _fake_os_exit(code: int) -> None:
        # Record AFTER any prior on_exit appended its marker so the
        # ordering assertion below is meaningful.
        call_order.append(f"os_exit({code})")
        fake_exit_called.append(code)

    monkeypatch.setattr("os._exit", _fake_os_exit)

    await supervise_parent(
        parent_pid=helper_pid,
        on_exit=_on_exit,
        poll_interval_s=0.05,
    )

    assert fake_exit_called == [0], "supervise_parent must call os._exit(0)"
    assert call_order == ["on_exit", "os_exit(0)"], (
        f"on_exit must run before os._exit; got {call_order}"
    )


async def test_supervise_parent_swallows_on_exit_exception(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """on_exit raising MUST NOT block os._exit — best-effort cleanup."""
    helper = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(0.2)"],
    )
    helper_pid = helper.pid
    helper.wait()

    def _on_exit_boom() -> None:
        raise RuntimeError("cleanup failed")

    fake_exit_called = []
    monkeypatch.setattr("os._exit", lambda code: fake_exit_called.append(code))

    await supervise_parent(
        parent_pid=helper_pid,
        on_exit=_on_exit_boom,
        poll_interval_s=0.05,
    )

    assert fake_exit_called == [0], "os._exit must still fire even if on_exit raised"


async def test_watch_parent_stays_running_while_parent_alive() -> None:
    """While the parent is alive, the watchdog must NOT return early."""
    helper = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(30)"],
    )
    try:
        with pytest.raises(asyncio.TimeoutError):
            await asyncio.wait_for(
                watch_parent(parent_pid=helper.pid, poll_interval_s=0.1),
                timeout=0.5,
            )
    finally:
        helper.terminate()
        helper.wait(timeout=5)


@pytest.mark.slow
def test_sidecar_self_terminates_and_releases_port_within_5s() -> None:
    """Scenario: Parent exits unexpectedly.

    Full-stack E2E:
      1. Spawn a long-lived helper process as the "parent"
      2. Spawn sidecar with --parent-pid=<helper.pid>
      3. Kill the helper
      4. Assert the sidecar exits within 5 s AND the port is reusable
    """
    import httpx

    helper = subprocess.Popen(
        [sys.executable, "-c", "import sys, time; sys.stdout.write('ready\\n'); sys.stdout.flush(); time.sleep(300)"],
        stdout=subprocess.PIPE,
        text=True,
        env={**os.environ, "PYTHONUNBUFFERED": "1"},
    )
    assert helper.stdout is not None
    ready = helper.stdout.readline().strip()
    assert ready == "ready"

    sidecar = subprocess.Popen(
        [sys.executable, "-m", "codebus_agent.api.main", f"--parent-pid={helper.pid}"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={**os.environ, "PYTHONUNBUFFERED": "1"},
        text=True,
        bufsize=1,
    )
    port: int | None = None
    try:
        assert sidecar.stdout is not None
        handshake_line = sidecar.stdout.readline().strip()
        payload = json.loads(handshake_line)
        port = payload["port"]
        bearer = payload["bearer"]

        # sanity: sidecar is actually up
        response = httpx.get(
            f"http://127.0.0.1:{port}/healthz",
            headers={"Authorization": f"Bearer {bearer}"},
            timeout=2.0,
        )
        assert response.status_code == 200

        # kill the parent, start the clock
        if sys.platform == "win32":
            helper.kill()
        else:
            helper.send_signal(signal.SIGKILL)
        helper.wait(timeout=5.0)

        start = time.monotonic()
        sidecar.wait(timeout=WATCHDOG_LIMIT_S + 1.0)
        elapsed = time.monotonic() - start
        assert elapsed <= WATCHDOG_LIMIT_S, (
            f"sidecar did not self-terminate within {WATCHDOG_LIMIT_S} s "
            f"after parent exit (took {elapsed:.2f} s)"
        )

        # port must be rebindable — otherwise watchdog leaked a socket
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.bind(("127.0.0.1", port))
        finally:
            s.close()
    finally:
        if helper.poll() is None:
            helper.kill()
            helper.wait()
        if sidecar.poll() is None:
            sidecar.kill()
            sidecar.wait()
