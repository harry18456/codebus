"""ensure_in_workspace tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/tool-sandbox/spec.md
  Requirement: ensure_in_workspace blocks path escape
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

from codebus_agent.sandbox import PathEscapeError, ToolContext, ensure_in_workspace


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    ws = tmp_path / "workspace"
    ws.mkdir()
    (ws / "inside.txt").write_text("hello")
    (ws / "subdir").mkdir()
    (ws / "subdir" / "deep.txt").write_text("deep")
    return ws


@pytest.fixture
def ctx(workspace: Path) -> ToolContext:
    return ToolContext(workspace_root=workspace, workspace_type="folder")


def test_in_scope_relative_path_accepted(ctx: ToolContext, workspace: Path) -> None:
    """Scenario: In-scope path accepted.

    The returned path MUST be absolute and MUST sit under the workspace.
    """
    resolved = ensure_in_workspace("inside.txt", ctx)
    assert resolved.is_absolute()
    assert resolved == (workspace / "inside.txt").resolve()


def test_in_scope_nested_path_accepted(ctx: ToolContext, workspace: Path) -> None:
    resolved = ensure_in_workspace("subdir/deep.txt", ctx)
    assert resolved == (workspace / "subdir" / "deep.txt").resolve()


def test_parent_directory_escape_rejected(ctx: ToolContext) -> None:
    """Scenario: Parent-directory escape rejected."""
    with pytest.raises(PathEscapeError):
        ensure_in_workspace("../outside.txt", ctx)


def test_deep_parent_escape_rejected(ctx: ToolContext) -> None:
    with pytest.raises(PathEscapeError):
        ensure_in_workspace("subdir/../../outside.txt", ctx)


@pytest.mark.skipif(sys.platform == "win32", reason="symlinks require admin / developer mode on Windows")
def test_symlink_escape_rejected(ctx: ToolContext, workspace: Path, tmp_path: Path) -> None:
    """Scenario: Symlink escape rejected."""
    secret_dir = tmp_path / "outside"
    secret_dir.mkdir()
    (secret_dir / "secret").write_text("leaked")
    link = workspace / "escape_link"
    os.symlink(secret_dir, link)

    with pytest.raises(PathEscapeError):
        ensure_in_workspace("escape_link/secret", ctx)


def test_absolute_path_outside_rejected(ctx: ToolContext, tmp_path: Path) -> None:
    outside = tmp_path / "sibling" / "file.txt"
    with pytest.raises(PathEscapeError):
        ensure_in_workspace(str(outside), ctx)


@pytest.mark.skipif(sys.platform != "win32", reason="UNC path semantics are Windows-only")
def test_windows_unc_path_rejected(ctx: ToolContext) -> None:
    """Scenario: Windows UNC path rejected."""
    with pytest.raises(PathEscapeError):
        ensure_in_workspace(r"\\remoteserver\share\file.txt", ctx)


@pytest.mark.skipif(sys.platform != "win32", reason="long-path prefix is Windows-only")
def test_windows_long_path_prefix_pointing_inside_accepted(
    ctx: ToolContext, workspace: Path,
) -> None:
    """Scenario: Windows long-path prefix normalized.

    The ``\\\\?\\`` prefix pointing INTO the workspace MUST be accepted
    after normalization.
    """
    # Use the already-existing file so resolve succeeds even with strict=False
    prefixed = r"\\?\{}".format(workspace / "inside.txt")
    resolved = ensure_in_workspace(prefixed, ctx)
    assert resolved.exists()
    # Resolved must be within workspace (normcase-insensitive on Windows)
    assert os.path.normcase(str(resolved)).startswith(os.path.normcase(str(workspace)))


def test_absolute_path_inside_accepted(ctx: ToolContext, workspace: Path) -> None:
    """An absolute path pointing into the workspace is acceptable."""
    abs_inside = workspace / "subdir" / "deep.txt"
    resolved = ensure_in_workspace(str(abs_inside), ctx)
    assert resolved == abs_inside.resolve()


def test_nonexistent_in_scope_path_accepted(ctx: ToolContext, workspace: Path) -> None:
    """Tools may want to create files — resolve(strict=False) must succeed
    for nonexistent paths that would sit inside the workspace."""
    resolved = ensure_in_workspace("future_file.txt", ctx)
    assert resolved == (workspace / "future_file.txt").resolve()
