"""Tool sandbox — ToolContext schema + path-escape guard.

Backs openspec/changes/m1-power-on/specs/tool-sandbox/spec.md
  Requirement: ToolContext carries workspace type discriminator
  Requirement: ensure_in_workspace blocks path escape

The M1 ToolContext is the stripped-down skeleton — just the fields we
actually use here plus the discriminator (D-002 day-1 invariant).
Future fields (kb, sanitizer, audit_log, usage_tracker, ...) will be
added in subsequent milestones; adding them later is schema-compatible
because every future field is either required-with-default or optional.
"""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from typing import Literal

from pydantic import BaseModel, ConfigDict, field_validator


class PathEscapeError(ValueError):
    """Raised when a requested path resolves outside the workspace root."""


class ToolContext(BaseModel):
    """Authoritative per-run context handed to every sandboxed tool.

    ``frozen=True`` guarantees tools cannot silently relocate the
    workspace mid-run by mutating the context.  Per D-002 the
    ``workspace_type`` discriminator MUST be present day 1.
    """

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True)

    workspace_root: Path
    workspace_type: Literal["folder", "topic"]
    workspace_id: str = ""
    session_id: str = ""

    @field_validator("workspace_root", mode="after")
    @classmethod
    def _resolve_root(cls, v: Path) -> Path:
        return v.resolve(strict=False)


_LONG_PATH_PREFIX = "\\\\?\\"
_LONG_PATH_UNC_PREFIX = "\\\\?\\UNC\\"


def _strip_long_path_prefix(s: str) -> str:
    """Strip the Windows ``\\\\?\\`` / ``\\\\?\\UNC\\`` prefix so we can
    compare prefixed and non-prefixed paths structurally.

    ``Path.resolve`` does NOT normalise this prefix away — without
    stripping, a long-path-prefixed in-workspace path would fail the
    ``startswith`` check against the bare workspace root.
    """
    if s.startswith(_LONG_PATH_UNC_PREFIX):
        return "\\\\" + s[len(_LONG_PATH_UNC_PREFIX):]
    if s.startswith(_LONG_PATH_PREFIX):
        return s[len(_LONG_PATH_PREFIX):]
    return s


_WIN_SEP_RE = re.compile(r"[\\/]")


def _strip_trailing_dots_spaces_per_component(requested: str) -> str:
    """Strip trailing dots and spaces from each path component.

    Windows kernel does this at filesystem-call time — ``CreateFile``
    on ``foo.txt.`` opens ``foo.txt`` — but Python's ``pathlib`` does
    not replicate the behavior at ``resolve(strict=False)`` time.
    Without this preprocessing, an attack like ``.. /secret`` slips
    through comparison as a literal ``.. `` component even though the
    real filesystem would treat it as ``..`` and escape the workspace.
    """
    if sys.platform != "win32":
        return requested

    def _strip_component(comp: str) -> str:
        # Bare traversal operators survive unchanged.
        if comp in ("", ".", ".."):
            return comp
        stripped = comp.rstrip(". ")
        if stripped:
            return stripped
        # Pure dots/spaces component (e.g. "..." or ".. ") — Windows kernel
        # collapses trailing dots+spaces, so "..." and ".. " both behave like
        # ".." at filesystem-call time.  Canonicalize here so the resolver
        # sees the traversal and rejects the escape.
        dots = comp.count(".")
        if dots >= 2:
            return ".."
        if dots == 1:
            return "."
        return comp

    # Preserve the original separators by splitting/joining via regex.
    tokens: list[str] = []
    pos = 0
    for m in _WIN_SEP_RE.finditer(requested):
        tokens.append(_strip_component(requested[pos:m.start()]))
        tokens.append(m.group(0))
        pos = m.end()
    tokens.append(_strip_component(requested[pos:]))
    return "".join(tokens)


def _normalize(p: Path) -> str:
    """Return a normcase+normpath string for case-insensitive comparison.

    On Windows ``normcase`` lowercases ASCII and flips forward slashes to
    backslashes, which covers the case-only attack variants.  On POSIX it
    is a no-op, so behavior on Unix is unchanged.
    """
    return os.path.normcase(os.path.normpath(_strip_long_path_prefix(str(p))))


def _is_within(candidate: Path, root: Path) -> bool:
    c = _normalize(candidate)
    r = _normalize(root)
    if c == r:
        return True
    return c.startswith(r + os.sep)


def ensure_in_workspace(requested: str | os.PathLike[str], ctx: ToolContext) -> Path:
    """Resolve ``requested`` and assert it is inside ``ctx.workspace_root``.

    Per D-local-3 we ``resolve(strict=False)`` first — this follows
    symlinks (closing the symlink-escape vector) and normalizes Windows
    long-path prefixes (``\\\\?\\``) and UNC paths.  Trailing dots /
    spaces collapse through ``normpath`` at compare-time.

    Returns the resolved absolute Path on success; raises
    :class:`PathEscapeError` otherwise.  Never returns a path outside
    the workspace.
    """
    root = ctx.workspace_root  # already resolved by validator
    cleaned = _strip_trailing_dots_spaces_per_component(str(requested))
    p = Path(cleaned)
    candidate = p if p.is_absolute() else (root / p)
    resolved = candidate.resolve(strict=False)

    if not _is_within(resolved, root):
        raise PathEscapeError(
            f"Path {str(requested)!r} resolves to {resolved} which is outside "
            f"workspace {root}"
        )
    # Strip the \\?\ prefix from the returned path so downstream tool
    # code gets the canonical in-workspace form regardless of how the
    # caller spelled the input.
    stripped = _strip_long_path_prefix(str(resolved))
    return Path(stripped) if stripped != str(resolved) else resolved
