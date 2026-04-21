"""Schema tests for Scanner data models.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Deferred subsystem schema preservation

TDD red: these tests assert that the `ScanResult` schema is locked in
with stable stub defaults for subsystems this skeleton does not implement
(Sanitizer / Git / Monorepo). Once models.py lands they must pass without
modification — the stubs are the contract downstream consumers rely on.
"""
from __future__ import annotations

from datetime import datetime, timezone

import pytest
from pydantic import ValidationError

from codebus_agent.scanner.models import (
    ContentTypeSummary,
    FileEntry,
    GitMeta,
    ScanResult,
    ScanStats,
    Symlink,
)


# ---------------------------------------------------------------------------
# FileEntry
# ---------------------------------------------------------------------------


def _text_entry(**overrides: object) -> FileEntry:
    base: dict[str, object] = {
        "path": "src/main.py",
        "size": 128,
        "kind": "text",
        "language": "python",
        "language_confidence": "extension",
        "encoding": "utf-8",
        "content": "print('hi')\n",
    }
    base.update(overrides)
    return FileEntry(**base)  # type: ignore[arg-type]


class TestFileEntry:
    def test_text_entry_sanitize_stats_defaults_empty(self) -> None:
        entry = _text_entry()
        assert entry.sanitize_stats == {}

    def test_text_entry_roundtrip(self) -> None:
        entry = _text_entry()
        dumped = entry.model_dump()
        assert dumped["path"] == "src/main.py"
        assert dumped["size"] == 128
        assert dumped["kind"] == "text"
        assert dumped["encoding"] == "utf-8"
        assert dumped["content"] == "print('hi')\n"
        assert dumped["sanitize_stats"] == {}
        restored = FileEntry(**dumped)
        assert restored == entry

    def test_binary_entry_has_null_content_and_encoding(self) -> None:
        entry = FileEntry(
            path="logo.png",
            size=4096,
            kind="binary",
            language=None,
            language_confidence="unknown",
            encoding=None,
            content=None,
        )
        assert entry.content is None
        assert entry.encoding is None
        assert entry.oversized_preview is None

    def test_oversized_entry_supports_preview(self) -> None:
        entry = FileEntry(
            path="data/large.txt",
            size=1024 * 1024,
            kind="oversized",
            language=None,
            language_confidence="unknown",
            encoding=None,
            content=None,
            oversized_preview="head line 1\nhead line 2\n",
        )
        assert entry.oversized_preview == "head line 1\nhead line 2\n"

    def test_invalid_kind_rejected(self) -> None:
        with pytest.raises(ValidationError):
            _text_entry(kind="mystery")  # type: ignore[arg-type]

    def test_invalid_language_confidence_rejected(self) -> None:
        with pytest.raises(ValidationError):
            _text_entry(language_confidence="heuristic")  # type: ignore[arg-type]


# ---------------------------------------------------------------------------
# Symlink
# ---------------------------------------------------------------------------


class TestSymlink:
    def test_inside_workspace(self) -> None:
        sl = Symlink(path="link.py", target="src/real.py", resolved_in_workspace=True)
        assert sl.resolved_in_workspace is True

    def test_outside_workspace(self) -> None:
        sl = Symlink(path="escape", target="/etc/passwd", resolved_in_workspace=False)
        assert sl.resolved_in_workspace is False

    def test_missing_required_field_rejected(self) -> None:
        with pytest.raises(ValidationError):
            Symlink(path="link.py", target="src/real.py")  # type: ignore[call-arg]


# ---------------------------------------------------------------------------
# ScanStats
# ---------------------------------------------------------------------------


class TestScanStats:
    def test_all_counters_present(self) -> None:
        stats = ScanStats(
            total_files_walked=10,
            total_files_included=8,
            total_bytes_read=2048,
            duration_seconds=0.42,
            quarantined_count=0,
            skipped_count=2,
        )
        dumped = stats.model_dump()
        assert set(dumped) == {
            "total_files_walked",
            "total_files_included",
            "total_bytes_read",
            "duration_seconds",
            "quarantined_count",
            "skipped_count",
        }
        assert dumped["duration_seconds"] == pytest.approx(0.42)


# ---------------------------------------------------------------------------
# ContentTypeSummary
# ---------------------------------------------------------------------------


class TestContentTypeSummary:
    def test_dominant_category_literal(self) -> None:
        summary = ContentTypeSummary(
            total_files=10,
            kind_counts={"text": 10},
            language_counts={"python": 10},
            category_counts={"code": 10, "docs": 0, "config": 0, "test": 0, "other": 0},
            dominant_category="code",
            dominant_languages=["python"],
            has_tests=False,
            has_docs=False,
            is_monorepo=False,
        )
        assert summary.dominant_category == "code"

    def test_invalid_dominant_category_rejected(self) -> None:
        with pytest.raises(ValidationError):
            ContentTypeSummary(
                total_files=1,
                kind_counts={"text": 1},
                language_counts={"python": 1},
                category_counts={"code": 1, "docs": 0, "config": 0, "test": 0, "other": 0},
                dominant_category="unknown",  # type: ignore[arg-type]
                dominant_languages=["python"],
                has_tests=False,
                has_docs=False,
                is_monorepo=False,
            )


# ---------------------------------------------------------------------------
# GitMeta — schema locked in even though skeleton never populates it
# ---------------------------------------------------------------------------


class TestGitMeta:
    def test_round_trip(self) -> None:
        meta = GitMeta(
            head="abc123",
            branch="main",
            remote_url=None,
            recent_commits=[],
            file_activity={},
            blame={},
        )
        dumped = meta.model_dump()
        assert dumped["head"] == "abc123"
        assert dumped["recent_commits"] == []
        assert dumped["blame"] == {}


# ---------------------------------------------------------------------------
# ScanResult — deferred-subsystem stub defaults
# ---------------------------------------------------------------------------


def _minimal_summary(total: int = 0) -> ContentTypeSummary:
    return ContentTypeSummary(
        total_files=total,
        kind_counts={},
        language_counts={},
        category_counts={"code": 0, "docs": 0, "config": 0, "test": 0, "other": 0},
        dominant_category="mixed",
        dominant_languages=[],
        has_tests=False,
        has_docs=False,
        is_monorepo=False,
    )


def _minimal_stats() -> ScanStats:
    return ScanStats(
        total_files_walked=0,
        total_files_included=0,
        total_bytes_read=0,
        duration_seconds=0.0,
        quarantined_count=0,
        skipped_count=0,
    )


class TestScanResultDeferredDefaults:
    def test_git_defaults_to_none(self) -> None:
        started = datetime.now(timezone.utc)
        result = ScanResult(
            workspace_root="C:/tmp/ws",
            scan_started_at=started,
            scan_completed_at=started,
            files=[],
            symlinks=[],
            content_summary=_minimal_summary(),
            stats=_minimal_stats(),
            warnings=[],
        )
        assert result.git is None

    def test_monorepo_fields_default_inactive(self) -> None:
        started = datetime.now(timezone.utc)
        result = ScanResult(
            workspace_root="C:/tmp/ws",
            scan_started_at=started,
            scan_completed_at=started,
            files=[],
            symlinks=[],
            content_summary=_minimal_summary(),
            stats=_minimal_stats(),
            warnings=[],
        )
        assert result.is_monorepo is False
        assert result.monorepo_type is None
        assert result.sub_packages == []

    def test_files_default_sanitize_stats_empty(self) -> None:
        started = datetime.now(timezone.utc)
        result = ScanResult(
            workspace_root="C:/tmp/ws",
            scan_started_at=started,
            scan_completed_at=started,
            files=[_text_entry(), _text_entry(path="src/util.py")],
            symlinks=[],
            content_summary=_minimal_summary(total=2),
            stats=_minimal_stats(),
            warnings=[],
        )
        assert all(f.sanitize_stats == {} for f in result.files)

    def test_roundtrip_preserves_deferred_defaults(self) -> None:
        started = datetime.now(timezone.utc)
        result = ScanResult(
            workspace_root="C:/tmp/ws",
            scan_started_at=started,
            scan_completed_at=started,
            files=[_text_entry()],
            symlinks=[Symlink(path="link", target="target", resolved_in_workspace=True)],
            content_summary=_minimal_summary(total=1),
            stats=_minimal_stats(),
            warnings=["something happened"],
        )
        dumped = result.model_dump(mode="json")
        # Deferred defaults survive JSON-mode dump.
        assert dumped["git"] is None
        assert dumped["is_monorepo"] is False
        assert dumped["monorepo_type"] is None
        assert dumped["sub_packages"] == []
        assert dumped["files"][0]["sanitize_stats"] == {}

        restored = ScanResult(**dumped)
        assert restored.git is None
        assert restored.is_monorepo is False
        assert restored.monorepo_type is None
        assert restored.sub_packages == []
