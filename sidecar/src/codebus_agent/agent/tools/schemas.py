"""Tool-layer Pydantic schemas.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools

``SearchHit`` is shared with the tool wire format returned by both KB and
grep fallback paths; ``DirEntry`` is the flat one-level entry shape
returned by ``list_dir``. The agent-layer ``Content`` (defined in
``codebus_agent.agent.protocols``) is re-exported here so callers can
import everything tool-related from a single module.
"""
from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field

from codebus_agent.agent.protocols import Content


__all__ = ["Content", "DirEntry", "SearchHit"]


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
