"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/qdrant-client/spec.md
  Requirement: Local Qdrant launch recipe
    Scenario: Script emits actionable error when binary is missing
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[3]
SH_SCRIPT = REPO_ROOT / "sidecar" / "scripts" / "start-qdrant.sh"
PS_SCRIPT = REPO_ROOT / "sidecar" / "scripts" / "start-qdrant.ps1"

DOWNLOAD_HINT = "github.com/qdrant/qdrant/releases"


def _run_sh(env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    bash = shutil.which("bash")
    assert bash is not None, "bash required on PATH"
    return subprocess.run(
        [bash, str(SH_SCRIPT)],
        env=env,
        capture_output=True,
        text=True,
        timeout=10,
    )


def _run_ps(env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    pwsh = shutil.which("pwsh") or shutil.which("powershell")
    assert pwsh is not None, "PowerShell required on PATH"
    return subprocess.run(
        [pwsh, "-NoProfile", "-File", str(PS_SCRIPT)],
        env=env,
        capture_output=True,
        text=True,
        timeout=10,
    )


@pytest.fixture
def missing_bin_env(tmp_path: Path) -> dict[str, str]:
    env = {**os.environ}
    # Point at a path that definitely does not exist AND override HOME so
    # the default resolution also misses.
    env["CODEBUS_QDRANT_BIN"] = str(tmp_path / "nope" / "qdrant")
    env["HOME"] = str(tmp_path / "home")
    env["USERPROFILE"] = str(tmp_path / "home")
    return env


@pytest.mark.skipif(shutil.which("bash") is None, reason="bash not available")
def test_sh_exits_nonzero_when_binary_missing(missing_bin_env: dict[str, str]) -> None:
    result = _run_sh(missing_bin_env)
    assert result.returncode != 0, result.stdout + result.stderr
    combined = result.stderr + result.stdout
    assert DOWNLOAD_HINT in combined, f"missing download hint: {combined!r}"


@pytest.mark.skipif(
    sys.platform != "win32" and shutil.which("pwsh") is None,
    reason="PowerShell not available on this platform",
)
def test_ps_exits_nonzero_when_binary_missing(missing_bin_env: dict[str, str]) -> None:
    result = _run_ps(missing_bin_env)
    assert result.returncode != 0, result.stdout + result.stderr
    combined = result.stderr + result.stdout
    assert DOWNLOAD_HINT in combined, f"missing download hint: {combined!r}"


def test_compose_fallback_defines_qdrant_service() -> None:
    """Spec: Docker Compose remains available as a fallback."""
    compose = REPO_ROOT / "sidecar" / "docker-compose.qdrant.yml"
    body = compose.read_text(encoding="utf-8")
    assert "qdrant:" in body, "compose must declare a qdrant service"
    assert "/qdrant/storage" in body or "./kb" in body, (
        "compose must bind-mount ./kb for persistence"
    )
    assert "6333:6333" in body, "qdrant HTTP port must be exposed"
