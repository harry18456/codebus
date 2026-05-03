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


# ───── qdrant-auto-spawn §4 run() integration ─────────────────────


def test_run_spawns_qdrant_after_handshake_before_serve(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario: Spawn never blocks sidecar startup, plus ordering
    invariant from spec Requirement text:
      'after the sidecar's handshake JSON line has been emitted to
       stdout and BEFORE asyncio.run(_serve(...)) opens the FastAPI
       listener'

    Asserts that in the run() flow:
      1. handshake.emit() fires
      2. maybe_spawn_qdrant() fires
      3. register_cleanup() fires
      4. install_signal_cleanup() fires
      5. asyncio.run(_serve(...)) fires (last)
    """
    from unittest.mock import MagicMock, patch

    from codebus_agent.api import main as main_module

    call_order: list[str] = []

    def _record(name: str, *, returns: object = None) -> object:
        def _inner(*_args: object, **_kwargs: object) -> object:
            call_order.append(name)
            return returns

        return _inner

    fake_proc = MagicMock(name="qdrant_popen")

    monkeypatch.setattr(
        main_module.auth, "generate_token", _record("generate_token", returns="x" * 32)
    )
    fake_sock = MagicMock()
    fake_sock.close = MagicMock()
    monkeypatch.setattr(
        main_module.net,
        "bind_ephemeral_loopback",
        _record("bind", returns=(fake_sock, 12345)),
    )
    monkeypatch.setattr(
        main_module._kb_qdrant, "resolve_url", _record("resolve_url", returns=None)
    )
    monkeypatch.setattr(
        main_module, "create_app", _record("create_app", returns=MagicMock())
    )
    monkeypatch.setattr(
        main_module.handshake, "emit", _record("handshake_emit")
    )
    monkeypatch.setattr(
        main_module, "maybe_spawn_qdrant", _record("spawn_qdrant", returns=fake_proc)
    )
    monkeypatch.setattr(
        main_module, "register_cleanup", _record("register_cleanup", returns=lambda: None)
    )
    monkeypatch.setattr(
        main_module, "install_signal_cleanup", _record("install_signal_cleanup")
    )

    # Stub asyncio.run so the test does not actually spin up uvicorn.
    monkeypatch.setattr(
        main_module.asyncio, "run", _record("asyncio_run")
    )

    main_module.run(argv=[])

    handshake_idx = call_order.index("handshake_emit")
    spawn_idx = call_order.index("spawn_qdrant")
    register_idx = call_order.index("register_cleanup")
    signal_idx = call_order.index("install_signal_cleanup")
    serve_idx = call_order.index("asyncio_run")

    assert handshake_idx < spawn_idx, (
        f"handshake must emit before spawn; order={call_order}"
    )
    assert spawn_idx < serve_idx, (
        f"spawn must happen before asyncio.run(_serve); order={call_order}"
    )
    assert register_idx > spawn_idx and register_idx < serve_idx
    assert signal_idx > spawn_idx and signal_idx < serve_idx


def test_run_continues_when_spawn_returns_none(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario: Spawn never blocks sidecar startup.

    Even when maybe_spawn_qdrant returns None (binary missing /
    timeout), asyncio.run(_serve(...)) MUST still fire.
    """
    from unittest.mock import MagicMock, patch

    from codebus_agent.api import main as main_module

    serve_called = [False]

    def _serve_marker(*_a: object, **_kw: object) -> None:
        serve_called[0] = True

    fake_sock = MagicMock()
    monkeypatch.setattr(main_module.auth, "generate_token", lambda: "x" * 32)
    monkeypatch.setattr(
        main_module.net, "bind_ephemeral_loopback", lambda: (fake_sock, 12345)
    )
    monkeypatch.setattr(main_module._kb_qdrant, "resolve_url", lambda: None)
    monkeypatch.setattr(main_module, "create_app", lambda **_: MagicMock())
    monkeypatch.setattr(main_module.handshake, "emit", lambda **_: None)
    monkeypatch.setattr(
        main_module, "maybe_spawn_qdrant", lambda parent_pid=None: None
    )
    monkeypatch.setattr(
        main_module, "register_cleanup", lambda proc: lambda: None
    )
    monkeypatch.setattr(main_module, "install_signal_cleanup", lambda proc: None)
    monkeypatch.setattr(main_module.asyncio, "run", _serve_marker)

    main_module.run(argv=[])

    assert serve_called[0], "asyncio.run(_serve) must fire even when spawn returns None"


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
