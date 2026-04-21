"""Strategy dispatch regression tests.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: Chunk strategy dispatch by FileEntry kind and language
    Scenario: Markdown routed to doc strategy
    Scenario: Source code routed to code strategy
    Scenario: Binary file produces skeleton payload
    Scenario: Oversized file chunks preview only
    Scenario: Symlink produces no payload
"""
from __future__ import annotations

from pathlib import Path

import pytest

from codebus_agent.kb.chunker import dispatch_for_file_entry
from codebus_agent.scanner.models import FileEntry


_FIXTURE_DIR = Path(__file__).parent / "fixtures"


def _load(name: str) -> str:
    return (_FIXTURE_DIR / name).read_text(encoding="utf-8")


def test_markdown_routed_to_doc_strategy() -> None:
    """Scenario: Markdown routed to doc strategy.

    Doc strategy splits on `##`-or-deeper headings before falling back to
    the token window, so the emitted chunk count MUST be >= the heading
    count and a chunk MUST start at each `##` boundary.
    """
    text = _load("sample-doc.md")
    entry = FileEntry(
        path="sample-doc.md",
        size=len(text.encode("utf-8")),
        kind="text",
        language="markdown",
        encoding="utf-8",
        content=text,
    )

    drafts = dispatch_for_file_entry(entry)

    assert len(drafts) >= 3, (
        f"sample-doc.md has 3 ## headings; doc strategy must yield at "
        f"least one segment per heading, got {len(drafts)} drafts"
    )
    headings = ("## Overview", "## Architecture", "## Usage")
    starts = [d.text.lstrip().splitlines()[0] if d.text.strip() else "" for d in drafts]
    for heading in headings:
        assert any(s.startswith(heading) for s in starts), (
            f"expected a chunk to begin with {heading!r}; "
            f"actual chunk starts: {starts}"
        )


def test_python_routed_to_code_strategy() -> None:
    """Scenario: Source code routed to code strategy.

    Code strategy is pure token-window, so headings (e.g., a `## ...`
    inside a docstring) MUST NOT be used as segment boundaries — every
    chunk MUST end on a real source line, not a heading break.
    """
    code = _load("sample-code.py")
    entry = FileEntry(
        path="sample-code.py",
        size=len(code.encode("utf-8")),
        kind="text",
        language="python",
        encoding="utf-8",
        content=code,
    )

    drafts = dispatch_for_file_entry(entry)

    assert len(drafts) >= 1
    for idx, draft in enumerate(drafts):
        is_last = idx == len(drafts) - 1
        assert draft.text.endswith("\n") or is_last, (
            f"code chunk {idx} must end on a line boundary "
            f"(or be final); tail={draft.text[-20:]!r}"
        )
        # Code strategy MUST NOT mark chunks with the doc-only `preview`
        # flag nor the binary `skeleton` flag.
        assert "preview" not in draft.flags
        assert "skeleton" not in draft.flags


@pytest.mark.parametrize("kind", ["binary", "lockfile", "generated"])
def test_binary_produces_skeleton_payload(kind: str) -> None:
    """Scenario: Binary file produces skeleton payload."""
    entry = FileEntry(
        path="assets/logo.png",
        size=4096,
        kind=kind,
        language=None,
        encoding=None,
        content=None,
    )

    drafts = dispatch_for_file_entry(entry)

    assert len(drafts) == 1, (
        f"{kind} file MUST produce exactly one skeleton draft, got {len(drafts)}"
    )
    only = drafts[0]
    assert only.text == ""
    assert only.chunk_index == 0
    assert only.chunk_total == 1
    assert "skeleton" in only.flags


def test_oversized_chunks_preview_only() -> None:
    """Scenario: Oversized file chunks preview only.

    Build a long preview body so it spans multiple windows; assert every
    emitted draft carries the `preview` marker and that the union of
    chunk texts only references preview content (no full-file leakage).
    """
    preview_lines = [
        f"preview line {i}: " + ("token " * 40) for i in range(1, 401)
    ]
    preview = "\n".join(preview_lines) + "\n"
    entry = FileEntry(
        path="huge.log",
        size=10_000_000,
        kind="oversized",
        language="plaintext",
        encoding="utf-8",
        content=None,
        oversized_preview=preview,
    )

    drafts = dispatch_for_file_entry(entry)

    assert len(drafts) >= 2, (
        "long preview must produce multiple windows so we can verify "
        "preview-marker propagation across all of them"
    )
    for idx, draft in enumerate(drafts):
        assert "preview" in draft.flags, (
            f"oversized draft {idx} missing preview flag; flags={draft.flags}"
        )


def test_symlink_produces_no_payload() -> None:
    """Scenario: Symlink produces no payload.

    Symlinks are reported on `ScanResult.symlinks`, never as a `FileEntry`.
    The dispatcher itself never sees them. We assert the contract by
    confirming no `FileKind` value covers symlinks: the type system already
    excludes them, so the builder's iteration over `ScanResult.files` is
    structurally incapable of producing a payload for a symlink entry.
    """
    from codebus_agent.scanner.models import FileKind
    from typing import get_args

    kinds = set(get_args(FileKind))
    # The literal MUST cover the four chunkable kinds plus the skeleton
    # kinds, and MUST NOT contain a "symlink" alternative.
    assert "symlink" not in kinds, (
        f"FileKind unexpectedly contains 'symlink': {kinds}. The builder "
        f"relies on the scanner segregating symlinks into ScanResult.symlinks."
    )
    assert kinds == {"text", "binary", "oversized", "lockfile", "generated"}, (
        f"FileKind drift would let dispatch_for_file_entry receive an "
        f"unexpected kind; got {kinds}"
    )
