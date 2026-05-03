"""TDD red tests for `qdrant_supervisor`.

Backs SHALL clauses in
``openspec/changes/qdrant-auto-spawn/specs/qdrant-client/spec.md``
  Requirement: Sidecar-managed Qdrant child process (5 scenarios)
And ``specs/sidecar-runtime/spec.md``
  Requirement: Qdrant child process supervision lifecycle (5 scenarios)
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import Any
from unittest.mock import MagicMock, patch

import pytest

from codebus_agent.qdrant_supervisor import (
    cleanup_qdrant_child,
    install_signal_cleanup,
    maybe_spawn_qdrant,
    _resolve_binary_path,
    _resolve_storage_paths,
)


def _make_popen_mock(*, alive: bool = True, exit_code: int = 0) -> MagicMock:
    """Build a Popen mock whose .poll() reflects alive/dead state."""
    proc = MagicMock()
    proc.pid = 99999
    proc.returncode = None if alive else exit_code
    proc.poll = MagicMock(side_effect=lambda: None if proc.returncode is None else proc.returncode)

    def _terminate() -> None:
        proc.returncode = 0  # graceful exit

    proc.terminate = MagicMock(side_effect=_terminate)
    proc.kill = MagicMock(side_effect=_terminate)
    proc.wait = MagicMock(return_value=0)
    return proc


# ───── §2 supervisor happy / degraded path ─────────────────────────


class _FakeProbe:
    """Probe stub. Each entry: (delay_s, status_or_exception)."""

    def __init__(self, responses: list[Any]) -> None:
        self._responses = list(responses)

    def __call__(self, url: str, timeout: float) -> Any:
        if not self._responses:
            raise RuntimeError("probe exhausted")
        result = self._responses.pop(0)
        if isinstance(result, Exception):
            raise result
        resp = MagicMock()
        resp.status = result
        resp.__enter__ = lambda *_: resp
        resp.__exit__ = lambda *_: None
        return resp


def test_spawn_skipped_when_qdrant_already_reachable(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Spec scenario: Spawn skipped when Qdrant already reachable."""
    fake_bin = tmp_path / "qdrant.exe"
    fake_bin.write_bytes(b"placeholder")
    monkeypatch.setenv("CODEBUS_QDRANT_BIN", str(fake_bin))

    probe = _FakeProbe(responses=[200])
    popen = MagicMock()

    with patch("urllib.request.urlopen", probe), patch("subprocess.Popen", popen):
        result = maybe_spawn_qdrant(parent_pid=None)

    assert result is None
    assert popen.call_count == 0, "must NOT spawn when 6333 already serves 2xx"


def test_spawn_happens_when_port_unreachable(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Spec scenario: Spawn happens when port 6333 is unreachable."""
    fake_bin = tmp_path / "qdrant.exe"
    fake_bin.write_bytes(b"placeholder")
    storage = tmp_path / "kb"
    monkeypatch.setenv("CODEBUS_QDRANT_BIN", str(fake_bin))
    monkeypatch.setenv("CODEBUS_QDRANT_STORAGE", str(storage))

    # Initial probe fails (unreachable), then 1 retry fails, then 2xx.
    probe = _FakeProbe(responses=[
        ConnectionRefusedError("not running"),
        ConnectionRefusedError("still booting"),
        200,
    ])
    proc = _make_popen_mock(alive=True)
    popen = MagicMock(return_value=proc)

    with patch("urllib.request.urlopen", probe), patch("subprocess.Popen", popen):
        result = maybe_spawn_qdrant(parent_pid=None)

    assert result is proc, "must return the spawned Popen handle"
    assert popen.call_count == 1, "spawn called exactly once"

    call_args = popen.call_args
    cmd = call_args.args[0]
    assert cmd[0] == str(fake_bin)
    env = call_args.kwargs.get("env") or {}
    assert env["QDRANT__STORAGE__STORAGE_PATH"] == str(storage)
    assert env["QDRANT__STORAGE__SNAPSHOTS_PATH"] == str(storage / "snapshots")


def test_binary_not_found_degrades_to_fallback(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture) -> None:
    """Spec scenario: Binary not found degrades to fallback."""
    monkeypatch.setenv("CODEBUS_QDRANT_BIN", str(tmp_path / "nonexistent" / "qdrant.exe"))
    monkeypatch.setenv("HOME", str(tmp_path / "fake_home"))
    monkeypatch.setenv("USERPROFILE", str(tmp_path / "fake_home"))  # Windows

    probe = _FakeProbe(responses=[ConnectionRefusedError("not running")])
    popen = MagicMock()

    with caplog.at_level("WARNING"):
        with patch("urllib.request.urlopen", probe), patch("subprocess.Popen", popen):
            result = maybe_spawn_qdrant(parent_pid=None)

    assert result is None
    assert popen.call_count == 0, "must NOT attempt spawn without binary"
    warnings = [r for r in caplog.records if r.levelname == "WARNING"]
    assert any("qdrant" in r.getMessage().lower() for r in warnings), (
        "must emit a warning naming the resolution issue"
    )


def test_spawn_timeout_terminates_orphaned_child(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Spec scenario: Spawn timeout terminates orphaned child."""
    fake_bin = tmp_path / "qdrant.exe"
    fake_bin.write_bytes(b"placeholder")
    monkeypatch.setenv("CODEBUS_QDRANT_BIN", str(fake_bin))

    # Probe always fails (poll exhausted).
    probe = _FakeProbe(responses=[ConnectionRefusedError("never ready")] * 100)
    proc = _make_popen_mock(alive=True)
    popen = MagicMock(return_value=proc)

    # Compress poll budget for the test so it doesn't take 10 real seconds.
    with (
        patch("urllib.request.urlopen", probe),
        patch("subprocess.Popen", popen),
        patch("codebus_agent.qdrant_supervisor._POLL_BUDGET_S", 0.5),
        patch("codebus_agent.qdrant_supervisor._POLL_INTERVAL_S", 0.05),
    ):
        result = maybe_spawn_qdrant(parent_pid=None)

    assert result is None
    assert proc.terminate.call_count >= 1, "must terminate orphaned child after timeout"


# ───── §2.2 binary path resolution ─────────────────────────────────


def test_resolve_binary_env_override_wins(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Spec text: 'resolved in the following order: (1) CODEBUS_QDRANT_BIN'."""
    override = tmp_path / "custom-qdrant.exe"
    override.write_bytes(b"placeholder")
    monkeypatch.setenv("CODEBUS_QDRANT_BIN", str(override))

    resolved = _resolve_binary_path()
    assert resolved == override


def test_resolve_binary_falls_back_to_home_path(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Spec text: '(2) ~/.codebus/bin/qdrant{.exe}'."""
    fake_home = tmp_path / "home"
    fake_bin_dir = fake_home / ".codebus" / "bin"
    fake_bin_dir.mkdir(parents=True)
    suffix = ".exe" if os.name == "nt" else ""
    fake_bin = fake_bin_dir / f"qdrant{suffix}"
    fake_bin.write_bytes(b"placeholder")
    monkeypatch.setenv("HOME", str(fake_home))
    monkeypatch.setenv("USERPROFILE", str(fake_home))
    monkeypatch.delenv("CODEBUS_QDRANT_BIN", raising=False)

    resolved = _resolve_binary_path()
    assert resolved == fake_bin


def test_resolve_binary_returns_none_when_missing(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Both override + fallback paths missing → None (caller handles degraded)."""
    monkeypatch.delenv("CODEBUS_QDRANT_BIN", raising=False)
    monkeypatch.setenv("HOME", str(tmp_path / "empty_home"))
    monkeypatch.setenv("USERPROFILE", str(tmp_path / "empty_home"))

    resolved = _resolve_binary_path()
    assert resolved is None


# ───── §2.4 storage env alignment ──────────────────────────────────


def test_storage_env_default_matches_dev_tool(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Spec scenario: Storage env vars match dev tool resolution.

    `start-qdrant.ps1` defaults: $HOME/.codebus/kb (storage),
    $storage/snapshots (snapshots). When CODEBUS_QDRANT_STORAGE is unset
    the supervisor MUST resolve the same paths.
    """
    fake_home = tmp_path / "home"
    monkeypatch.setenv("HOME", str(fake_home))
    monkeypatch.setenv("USERPROFILE", str(fake_home))
    monkeypatch.delenv("CODEBUS_QDRANT_STORAGE", raising=False)

    storage, snapshots = _resolve_storage_paths()
    assert storage == fake_home / ".codebus" / "kb"
    assert snapshots == storage / "snapshots"


def test_storage_env_override_wins(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """When CODEBUS_QDRANT_STORAGE is set, both supervisor and
    dev tool MUST honor it for storage path."""
    custom = tmp_path / "custom_kb"
    monkeypatch.setenv("CODEBUS_QDRANT_STORAGE", str(custom))

    storage, snapshots = _resolve_storage_paths()
    assert storage == custom
    assert snapshots == custom / "snapshots"


# ───── §3 cleanup paths ────────────────────────────────────────────


def test_cleanup_terminates_live_child() -> None:
    """Spec Requirement: cleanup calls terminate() then wait(5)."""
    proc = _make_popen_mock(alive=True)
    cleanup_qdrant_child(proc)
    assert proc.terminate.call_count == 1
    proc.wait.assert_called_once()


def test_cleanup_kills_after_timeout() -> None:
    """terminate timeout → kill()."""
    import subprocess as _sp

    proc = _make_popen_mock(alive=True)
    proc.wait = MagicMock(side_effect=[_sp.TimeoutExpired(cmd="qdrant", timeout=5), 0])

    cleanup_qdrant_child(proc)
    assert proc.terminate.call_count == 1
    assert proc.kill.call_count == 1
    assert proc.wait.call_count == 2


def test_cleanup_idempotent_on_dead_child() -> None:
    """Spec scenario: Cleanup is idempotent under multiple exit paths."""
    proc = _make_popen_mock(alive=False, exit_code=0)
    cleanup_qdrant_child(proc)
    cleanup_qdrant_child(proc)  # second invocation must no-op
    assert proc.terminate.call_count == 0, "dead child must not be terminated"
    assert proc.kill.call_count == 0


# ───── §3.5 signal handler installation ───────────────────────────


def test_install_signal_cleanup_registers_sigterm_on_posix() -> None:
    """Spec text: 'POSIX SIGTERM ... SHALL trigger the same cleanup'.

    The handler must end up calling cleanup_qdrant_child(proc) when
    the signal fires. Test invokes the registered handler directly
    rather than delivering a real signal (delivering SIGTERM to the
    pytest process would kill the test runner).
    """
    import signal as _signal

    proc = _make_popen_mock(alive=True)
    captured: dict[int, object] = {}

    def fake_signal(sig: int, handler: object) -> object:
        captured[sig] = handler
        return _signal.SIG_DFL

    with patch("signal.signal", side_effect=fake_signal):
        install_signal_cleanup(proc)

    if os.name == "nt":
        # Windows path uses SIGBREAK (CTRL_BREAK_EVENT) since SIGTERM
        # delivery semantics differ from POSIX.
        sig = _signal.SIGBREAK  # type: ignore[attr-defined]
    else:
        sig = _signal.SIGTERM
    assert sig in captured, f"expected handler registered for {sig}"

    handler = captured[sig]
    assert callable(handler), "handler must be invokable"

    # Invoking the handler MUST call cleanup_qdrant_child on the proc.
    # The handler also re-raises the default disposition; we patch the
    # final raise so the test process is not killed.
    with patch("signal.signal", side_effect=fake_signal), patch("os.kill"):
        handler(sig, None)

    assert proc.terminate.call_count == 1, (
        "signal handler must call cleanup_qdrant_child (which terminates the child)"
    )


def test_install_signal_cleanup_noop_when_proc_is_none() -> None:
    """If maybe_spawn_qdrant returned None, install_signal_cleanup
    MUST NOT register a handler — there is nothing to clean."""
    captured: dict[int, object] = {}

    def fake_signal(sig: int, handler: object) -> object:
        captured[sig] = handler
        return None

    with patch("signal.signal", side_effect=fake_signal):
        install_signal_cleanup(None)

    assert captured == {}, "no signal handlers should be registered for None"
