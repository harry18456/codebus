"""Folder-mode Explorer tools package.

Re-exports the concrete ``FolderTools`` implementation and the shared
schemas so callers can import everything tool-related from one module.

Schema inventory:

- ``SearchHit`` — ``search`` / KB result shape (explorer-tools-p0)
- ``DirEntry`` — ``list_dir`` one-level entry (explorer-tools-p0)
- ``FileMatch`` — ``find_callers`` call-site shape (explorer-tools-p1)
"""
from __future__ import annotations

from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.tools.schemas import (
    Content,
    DirEntry,
    FileMatch,
    SearchHit,
)

__all__ = [
    "Content",
    "DirEntry",
    "FileMatch",
    "FolderTools",
    "SearchHit",
]
