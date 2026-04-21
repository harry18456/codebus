"""Chunker: token-window slicing + FileEntry strategy dispatch.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: Token-window chunker respects line boundaries
  Requirement: Chunk strategy dispatch by FileEntry kind and language
"""
from __future__ import annotations

from functools import lru_cache

import tiktoken

from codebus_agent.kb.payload import ChunkDraft
from codebus_agent.scanner.models import FileEntry

_TOKEN_ENCODING = "cl100k_base"

_DOC_LANGUAGES: frozenset[str] = frozenset({"markdown", "rst", "asciidoc", "plaintext"})


@lru_cache(maxsize=1)
def _encoding() -> tiktoken.Encoding:
    """Cache the tiktoken encoding so each `chunk_text` call avoids the
    fresh-construction overhead (encoding load is the slow part)."""
    return tiktoken.get_encoding(_TOKEN_ENCODING)


def _line_start(prefix: str) -> int:
    """1-based line number for the character that comes *after* `prefix`."""
    return prefix.count("\n") + 1


def _line_end(combined: str) -> int:
    """1-based line number containing the final character of `combined`."""
    if not combined:
        return 1
    newlines = combined.count("\n")
    return newlines if combined.endswith("\n") else newlines + 1


def chunk_text(
    text: str,
    *,
    chunk_size: int = 600,
    overlap: int = 60,
) -> list[ChunkDraft]:
    """Token-window chunker that respects source line boundaries.

    The chunker measures windows in tiktoken tokens (encoding
    ``cl100k_base``); when a natural window boundary lands mid-line the
    slice is backtracked to the nearest preceding newline so the
    emitted chunk ends on a complete line. The final chunk is exempt
    from this requirement (it carries whatever tail remains).

    A line that exceeds ``chunk_size`` on its own (no embedded newline
    in the window) is emitted as-is rather than crashed — see design
    "line-boundary backtrack helper ... 遇到整塊無換行的極端狀況放行
    不強切".
    """
    if chunk_size <= 0:
        raise ValueError(f"chunk_size must be > 0, got {chunk_size}")
    if overlap < 0 or overlap >= chunk_size:
        raise ValueError(
            f"overlap must satisfy 0 <= overlap < chunk_size; "
            f"got chunk_size={chunk_size}, overlap={overlap}"
        )
    if not text:
        return []

    enc = _encoding()
    tokens = enc.encode(text)
    if not tokens:
        return []

    if len(tokens) <= chunk_size:
        return [
            ChunkDraft(
                text=text,
                line_start=1,
                line_end=_line_end(text),
                token_count=len(tokens),
            )
        ]

    drafts: list[ChunkDraft] = []
    start = 0
    step = chunk_size - overlap
    total_tokens = len(tokens)
    while start < total_tokens:
        end = min(start + chunk_size, total_tokens)
        is_last = end == total_tokens
        prefix = enc.decode(tokens[:start])
        chunk_str = enc.decode(tokens[start:end])

        if not is_last and not chunk_str.endswith("\n"):
            last_nl = chunk_str.rfind("\n")
            if last_nl >= 0:
                chunk_str = chunk_str[: last_nl + 1]
                # Re-anchor `end` so the next iteration's overlap is
                # computed against the actual emitted boundary, not the
                # un-backtracked window.
                end = len(enc.encode(prefix + chunk_str))

        combined = prefix + chunk_str
        drafts.append(
            ChunkDraft(
                text=chunk_str,
                line_start=_line_start(prefix),
                line_end=_line_end(combined),
                token_count=len(enc.encode(chunk_str)),
            )
        )

        if is_last:
            break

        new_start = end - overlap
        # Defensive: never regress, never stall on degenerate single-line input.
        if new_start <= start:
            new_start = start + max(1, step)
        start = new_start

    return drafts


def dispatch_for_file_entry(file_entry: FileEntry) -> list[ChunkDraft]:
    """Strategy dispatch by ``FileEntry.kind`` and ``language``.

    Per spec ``Chunk strategy dispatch by FileEntry kind and language``:
      - ``binary`` / ``lockfile`` / ``generated``: skeleton (one empty payload)
      - ``oversized``: chunk only the preview, mark each ``ChunkDraft``
        with the ``"preview"`` flag
      - ``text`` + doc language: heading-first split, then token window
      - ``text`` + any other language: pure token-window strategy
    """
    kind = file_entry.kind
    if kind in {"binary", "lockfile", "generated"}:
        return _skeleton_strategy(file_entry)
    if kind == "oversized":
        return _oversized_strategy(file_entry)
    if kind == "text":
        if file_entry.language in _DOC_LANGUAGES:
            return _doc_strategy(file_entry)
        return _code_strategy(file_entry)

    raise ValueError(
        f"chunker.dispatch_for_file_entry: unsupported kind {kind!r} "
        f"on file_path={file_entry.path!r}"
    )


def _skeleton_strategy(file_entry: FileEntry) -> list[ChunkDraft]:
    return [
        ChunkDraft(
            text="",
            line_start=1,
            line_end=1,
            token_count=0,
            chunk_index=0,
            chunk_total=1,
            flags=["skeleton"],
        )
    ]


def _oversized_strategy(file_entry: FileEntry) -> list[ChunkDraft]:
    preview = file_entry.oversized_preview or ""
    drafts = chunk_text(preview)
    if not drafts:
        return []
    for d in drafts:
        d.flags.append("preview")
    return drafts


def _code_strategy(file_entry: FileEntry) -> list[ChunkDraft]:
    return chunk_text(file_entry.content or "")


def _doc_strategy(file_entry: FileEntry) -> list[ChunkDraft]:
    """Heading-first split for prose, then token-window any oversize segment.

    ``##`` (and deeper) headings define segment boundaries; the prelude
    before the first heading is kept as its own segment so a doc with
    no leading heading still produces useful chunks. Any segment whose
    token count exceeds ``chunk_size`` is fed back through ``chunk_text``
    so the line-boundary + overlap invariants still hold.
    """
    text = file_entry.content or ""
    if not text:
        return []

    enc = _encoding()
    chunk_size = 600

    segments = _split_on_headings(text)
    drafts: list[ChunkDraft] = []
    char_offset = 0
    for seg_text in segments:
        seg_token_count = len(enc.encode(seg_text))
        line_start_in_file = text[:char_offset].count("\n") + 1
        if seg_token_count <= chunk_size:
            drafts.append(
                ChunkDraft(
                    text=seg_text,
                    line_start=line_start_in_file,
                    line_end=line_start_in_file
                    + seg_text.count("\n")
                    - (1 if seg_text.endswith("\n") and seg_text.count("\n") > 0 else 0),
                    token_count=seg_token_count,
                )
            )
        else:
            sub = chunk_text(seg_text)
            for s in sub:
                # Shift line numbers from segment-local to file-global.
                s.line_start += line_start_in_file - 1
                s.line_end += line_start_in_file - 1
                drafts.append(s)
        char_offset += len(seg_text)
    return drafts


def _split_on_headings(text: str) -> list[str]:
    """Split markdown-ish text on ``##``-or-deeper headings.

    Boundary rule: a line that starts with two-or-more ``#`` followed by
    a space marks the start of a new segment. The prelude (everything
    before the first such line) becomes the first segment when non-empty.
    """
    segments: list[str] = []
    current: list[str] = []
    for line in text.splitlines(keepends=True):
        stripped = line.lstrip()
        is_heading = stripped.startswith("##") and (
            len(stripped) > 2 and (stripped[2] == " " or stripped[2] == "#")
        )
        if is_heading and current:
            segments.append("".join(current))
            current = [line]
        else:
            current.append(line)
    if current:
        segments.append("".join(current))
    return segments


__all__ = [
    "chunk_text",
    "dispatch_for_file_entry",
]
