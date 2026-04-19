"""Red team fixture — backs SHALL clauses in
openspec/changes/m1-power-on/specs/tool-sandbox/spec.md
  Requirement: Red team fixture covers known attack vectors

Each attack must be rejected by ``ensure_in_workspace``; rejection means
a :class:`PathEscapeError` is raised and no path is returned.

Vectors (from spec.md):
  1. relative  ..  escape
  2. absolute path outside workspace
  3. symlink escape
  4. Windows junction escape
  5. UNC path
  6. \\?\ long-path prefix pointing outside
  7. case-only variants
  8. trailing-dot / trailing-space filename variants

Vectors that require elevated privileges on the host (symlink / junction
on Windows without developer-mode) emit a pytest.skip rather than a
silent pass, so the CI summary surfaces the missing coverage.
"""
from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import pytest

from codebus_agent.sandbox import PathEscapeError, ToolContext, ensure_in_workspace


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    ws = tmp_path / "workspace"
    ws.mkdir()
    (ws / "inside.txt").write_text("ok")
    (ws / "subdir").mkdir()
    return ws


@pytest.fixture
def ctx(workspace: Path) -> ToolContext:
    return ToolContext(workspace_root=workspace, workspace_type="folder")


@dataclass(frozen=True)
class Attack:
    name: str
    build: Callable[[Path, Path], str | None]


def _relative_dotdot(_ws: Path, _tmp: Path) -> str:
    return "../outside.txt"


def _absolute_outside(_ws: Path, tmp: Path) -> str:
    return str(tmp / "unrelated" / "secret.txt")


def _symlink_escape(ws: Path, tmp: Path) -> str | None:
    target = tmp / "outside_target"
    target.mkdir(exist_ok=True)
    (target / "leak").write_text("secret")
    link = ws / "link_out"
    try:
        os.symlink(target, link, target_is_directory=True)
    except (OSError, NotImplementedError):
        return None
    return "link_out/leak"


def _windows_junction_escape(ws: Path, tmp: Path) -> str | None:
    if sys.platform != "win32":
        return None
    target = tmp / "junction_target"
    target.mkdir(exist_ok=True)
    (target / "leak").write_text("secret")
    junction = ws / "jct_out"
    # mklink /J requires cmd.exe; no admin needed for junctions.
    result = subprocess.run(
        ["cmd", "/c", "mklink", "/J", str(junction), str(target)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return "jct_out/leak"


def _unc_path(_ws: Path, _tmp: Path) -> str | None:
    if sys.platform != "win32":
        return None
    return r"\\remote-host\share\secret.txt"


def _long_path_prefix_outside(_ws: Path, tmp: Path) -> str | None:
    if sys.platform != "win32":
        return None
    outside = tmp / "outside" / "secret.txt"
    return r"\\?\{}".format(outside)


def _case_only_escape(_ws: Path, _tmp: Path) -> str:
    # On a case-insensitive filesystem, a case-mutated .. still escapes.
    # Whatever the filesystem's behavior, the resolver must reject this.
    return "../OUTSIDE.txt"


def _trailing_dot_escape(_ws: Path, _tmp: Path) -> str:
    # Trailing dot on a traversal segment — on Windows the kernel strips
    # it, turning "..." into ".." and stripping trailing ".." dots into
    # an escape.  Must reject on every platform.
    return "..../outside.txt"


def _trailing_space_escape(_ws: Path, _tmp: Path) -> str:
    return ".. /outside.txt"


ATTACKS = [
    Attack("relative_parent_escape", _relative_dotdot),
    Attack("absolute_path_outside", _absolute_outside),
    Attack("symlink_escape", _symlink_escape),
    Attack("windows_junction_escape", _windows_junction_escape),
    Attack("windows_unc_path", _unc_path),
    Attack("windows_long_path_prefix_outside", _long_path_prefix_outside),
    Attack("case_only_variant", _case_only_escape),
    Attack("trailing_dot_variant", _trailing_dot_escape),
    Attack("trailing_space_variant", _trailing_space_escape),
]


def test_red_team_covers_all_spec_vectors() -> None:
    """Scenario: All attack vectors present in fixture.

    Scenario lists eight vector families; we have at least one Attack
    entry per family.  This test fails fast if someone removes a vector.
    """
    required = {
        "relative_parent_escape",
        "absolute_path_outside",
        "symlink_escape",
        "windows_junction_escape",
        "windows_unc_path",
        "windows_long_path_prefix_outside",
        "case_only_variant",
        # The spec lumps trailing-dot and trailing-space into one family;
        # we cover both variants separately.
    }
    names = {a.name for a in ATTACKS}
    missing = required - names
    assert not missing, f"red-team fixture missing vectors: {missing}"
    assert any(n.startswith("trailing_") for n in names), (
        "red-team fixture must cover trailing-dot / trailing-space variants"
    )


@pytest.mark.parametrize("attack", ATTACKS, ids=lambda a: a.name)
def test_attack_rejected(
    ctx: ToolContext,
    workspace: Path,
    tmp_path: Path,
    attack: Attack,
) -> None:
    """Scenario: Red team suite runs and passes."""
    attack_path = attack.build(workspace, tmp_path)
    if attack_path is None:
        pytest.skip(f"{attack.name}: not applicable on this platform / privilege level")
    with pytest.raises(PathEscapeError):
        ensure_in_workspace(attack_path, ctx)
