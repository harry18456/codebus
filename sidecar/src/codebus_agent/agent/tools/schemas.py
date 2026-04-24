"""Tool-layer Pydantic schemas.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools
openspec/changes/explorer-tools-p1/specs/explorer-tools/spec.md
  Requirement: find_callers returns sanitized call-site FileMatches

``SearchHit`` is shared with the tool wire format returned by both KB and
grep fallback paths; ``DirEntry`` is the flat one-level entry shape
returned by ``list_dir``; ``FileMatch`` is the call-site shape returned
by ``find_callers`` (Explorer P1 differentiated tool). The agent-layer
``Content`` (defined in ``codebus_agent.agent.protocols``) is
re-exported here so callers can import everything tool-related from a
single module.
"""
from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field

from codebus_agent.agent.protocols import Content


__all__ = ["Content", "DirEntry", "FileMatch", "SearchHit"]


class SearchHit(BaseModel):
    """Primary-search result — abstract shape shared by KB / grep paths."""

    path: str
    snippet: str
    score: float = Field(ge=0, le=1)


class DirEntry(BaseModel):
    """One flat directory entry from ``FolderTools.list_dir``."""

    name: str
    kind: Literal["file", "dir"]
    size: int = Field(ge=0)


class FileMatch(BaseModel):
    """One call-site occurrence returned by ``FolderTools.find_callers``.

    ``path`` is relative to ``ctx.workspace_root`` (POSIX separators).
    ``line`` is 1-indexed. ``snippet`` is the occurrence's source line
    passed through Pass 1 sanitize and truncated at 200 characters.

    Intentionally minimal per `openspec/changes/explorer-tools-p1/design.md`:
    no ``column`` / ``end_line`` / ``ast_node`` metadata — the Agent falls
    back to ``read_file`` if it needs surrounding context.
    """

    path: str
    line: int = Field(ge=1)
    snippet: str
