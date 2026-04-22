"""TDD red tests for `scanner.service.scan(on_progress=...)` — Section 10
of openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/folder-scanner/spec.md
  Requirement: Scanner progress callback hook
"""
from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine
from codebus_agent.scanner.models import ScannerProgressEvent, ScanResult
from codebus_agent.scanner.service import scan


def _ctx(root: Path) -> ToolContext:
    return ToolContext(
        workspace_root=root,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )


def _seed_workspace(root: Path, *, n: int = 5) -> None:
    """Create ``n`` small text files plus one with secrets so the
    sanitize phase has at least one hit to walk over.
    """
    for i in range(n):
        (root / f"f{i}.py").write_text(f"x = {i}\n", encoding="utf-8")
    # One file guaranteed to trigger the email rule so sanitize phase emits.
    (root / "contacts.txt").write_text(
        "alice@example.com\nbob@example.com\n", encoding="utf-8"
    )


async def test_scan_without_callback_preserves_sync_contract(tmp_path: Path) -> None:
    """Spec scenario "Synchronous call without callback preserves existing contract".

    No callback supplied → ``scan`` returns a usable ``ScanResult`` and
    behaves identically to the pre-change call shape.
    """
    _seed_workspace(tmp_path)
    result = await scan(str(tmp_path), _ctx(tmp_path))
    assert isinstance(result, ScanResult)
    # Same shape as before: deferred stubs intact, files populated.
    assert result.git is None
    assert result.is_monorepo is False
    assert len(result.files) >= 1


async def test_scan_emits_at_least_one_walking_and_sanitizing_event(
    tmp_path: Path,
) -> None:
    """Spec scenario "Callback receives at least one event per phase"."""
    _seed_workspace(tmp_path, n=3)
    seen: list[ScannerProgressEvent] = []

    async def collect(event: ScannerProgressEvent) -> None:
        seen.append(event)

    result = await scan(str(tmp_path), _ctx(tmp_path), on_progress=collect)
    assert isinstance(result, ScanResult)

    phases = {e.phase for e in seen}
    assert "walking" in phases, f"missing walking event in {phases!r}"
    assert "sanitizing" in phases, f"missing sanitizing event in {phases!r}"


async def test_callback_exception_propagates_and_does_not_return_partial_result(
    tmp_path: Path,
) -> None:
    """Spec scenario "Callback exception does not corrupt scan result"."""
    _seed_workspace(tmp_path)

    class CallbackBoom(RuntimeError):
        pass

    async def boom(event: ScannerProgressEvent) -> None:
        raise CallbackBoom("test-callback-failure")

    with pytest.raises(CallbackBoom):
        await scan(str(tmp_path), _ctx(tmp_path), on_progress=boom)


async def test_progress_event_invariants(tmp_path: Path) -> None:
    """Each ``ScannerProgressEvent`` MUST satisfy ``current >= 0`` and,
    when ``total`` is not None, ``current <= total``.
    """
    _seed_workspace(tmp_path, n=8)
    seen: list[ScannerProgressEvent] = []

    async def collect(event: ScannerProgressEvent) -> None:
        seen.append(event)

    await scan(str(tmp_path), _ctx(tmp_path), on_progress=collect)

    assert seen, "expected at least one progress event"
    for e in seen:
        assert e.current >= 0
        if e.total is not None:
            assert e.current <= e.total, (
                f"current {e.current} > total {e.total} on {e.phase!r}"
            )
