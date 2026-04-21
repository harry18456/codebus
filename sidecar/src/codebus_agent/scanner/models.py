"""Pydantic v2 models for the `folder-scanner` capability.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Deferred subsystem schema preservation

Schema locked in up front (aligned with `docs/module-1-scanner.md` §十一) so
follow-up changes (sanitizer wiring / git metadata / monorepo) stay additive.
During this skeleton change every `sanitize_stats` MUST emit as `{}`, `git`
MUST emit as `None`, and `is_monorepo` / `monorepo_type` / `sub_packages`
MUST default to `False` / `None` / `[]`.
"""
from __future__ import annotations

from datetime import datetime
from typing import Literal

from pydantic import BaseModel, Field

FileKind = Literal["text", "binary", "oversized", "lockfile", "generated"]
LanguageConfidence = Literal["extension", "shebang", "unknown"]
DominantCategory = Literal["code", "docs", "config", "mixed"]


class FileEntry(BaseModel):
    """Single file observed by the scanner.

    For `binary`, `lockfile`, `generated` entries `content` / `encoding`
    MUST be `None` (spec Requirement "File classification by extension
    and content sniffing").
    """

    path: str
    size: int
    kind: FileKind
    language: str | None = None
    language_confidence: LanguageConfidence = "unknown"
    encoding: str | None = None
    content: str | None = None
    oversized_preview: str | None = None
    sanitize_stats: dict[str, int] = Field(default_factory=dict)


class Symlink(BaseModel):
    """Symbolic link observed by the scanner.

    The scanner MUST NOT follow symlinks; this entry is the *only* record
    of the link (spec Requirement "Symlink handling without following").
    """

    path: str
    target: str
    resolved_in_workspace: bool


class GitMeta(BaseModel):
    """Git-derived metadata — deferred to a follow-up change.

    Defined now so downstream consumers can be written against the final
    contract; skeleton scans always leave `ScanResult.git = None`.
    """

    head: str
    branch: str
    remote_url: str | None = None
    recent_commits: list[dict] = Field(default_factory=list)
    file_activity: dict[str, dict] = Field(default_factory=dict)
    blame: dict[str, list[dict]] = Field(default_factory=dict)


class ContentTypeSummary(BaseModel):
    """Repo-level overview used by Explorer Agent at startup."""

    total_files: int
    kind_counts: dict[str, int]
    language_counts: dict[str, int]
    category_counts: dict[str, int]
    dominant_category: DominantCategory
    dominant_languages: list[str]
    has_tests: bool
    has_docs: bool
    is_monorepo: bool


class ScanStats(BaseModel):
    """Aggregated counters for a single scan run."""

    total_files_walked: int
    total_files_included: int
    total_bytes_read: int
    duration_seconds: float
    quarantined_count: int
    skipped_count: int


class ScanResult(BaseModel):
    """Top-level scan result consumed by Module 2 KB Builder and Module 4 Explorer.

    Deferred subsystems keep stable defaults:
      - `git` — filled in by the git-metadata change; skeleton leaves it `None`.
      - `is_monorepo` / `monorepo_type` / `sub_packages` — filled in by the
        monorepo-detection change; skeleton leaves them `False` / `None` / `[]`.
      - Each `FileEntry.sanitize_stats` — filled in by the sanitizer-wiring
        change; skeleton leaves it `{}`.
    """

    workspace_root: str
    scan_started_at: datetime
    scan_completed_at: datetime
    files: list[FileEntry]
    symlinks: list[Symlink]
    is_monorepo: bool = False
    monorepo_type: str | None = None
    sub_packages: list[dict] = Field(default_factory=list)
    git: GitMeta | None = None
    content_summary: ContentTypeSummary
    stats: ScanStats
    warnings: list[str] = Field(default_factory=list)


__all__ = [
    "ContentTypeSummary",
    "DominantCategory",
    "FileEntry",
    "FileKind",
    "GitMeta",
    "LanguageConfidence",
    "ScanResult",
    "ScanStats",
    "Symlink",
]
