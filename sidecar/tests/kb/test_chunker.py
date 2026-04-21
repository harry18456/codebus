"""Token-window chunker regression tests.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: Token-window chunker respects line boundaries
    Scenario: Chunk boundaries land on newline
    Scenario: Overlap preserves continuity
    Scenario: Short text produces single chunk
    Scenario: Empty text produces empty list
"""
from __future__ import annotations

import tiktoken

from codebus_agent.kb.chunker import chunk_text


_ENCODING = tiktoken.get_encoding("cl100k_base")


def _multi_line_long_text(target_tokens: int) -> str:
    """Build deterministic multi-line text whose tiktoken length exceeds target."""
    lines: list[str] = []
    counter = 0
    while True:
        counter += 1
        # Mix narrative and code-like text so the tokenizer produces
        # realistic distribution; line widths vary so the natural
        # token-window boundary lands mid-line plenty of times.
        lines.append(
            f"line {counter}: lorem ipsum dolor sit amet consectetur "
            f"adipiscing elit sed do eiusmod tempor incididunt ut labore "
            f"et dolore magna aliqua ut enim ad minim veniam quis nostrud {counter * 7}"
        )
        if counter % 5 == 0:
            lines.append(
                f"def f_{counter}(x: int) -> int: return x * {counter} + {counter % 13}"
            )
        joined = "\n".join(lines) + "\n"
        if len(_ENCODING.encode(joined)) >= target_tokens:
            return joined


def test_chunk_text_lands_on_line_boundary() -> None:
    """Scenario: Chunk boundaries land on newline."""
    text = _multi_line_long_text(target_tokens=2000)
    chunks = chunk_text(text, chunk_size=600, overlap=60)

    assert len(chunks) >= 2
    for idx, chunk in enumerate(chunks):
        is_last = idx == len(chunks) - 1
        assert chunk.text.endswith("\n") or is_last, (
            f"chunk {idx} must end on a line boundary "
            f"(or be the final chunk); got tail={chunk.text[-20:]!r}"
        )


def test_chunk_text_overlap_preserves_continuity() -> None:
    """Scenario: Overlap preserves continuity."""
    text = _multi_line_long_text(target_tokens=1500)
    chunks = chunk_text(text, chunk_size=600, overlap=60)
    assert len(chunks) >= 2

    for i in range(len(chunks) - 1):
        prev_tokens = _ENCODING.encode(chunks[i].text)
        next_tokens = _ENCODING.encode(chunks[i + 1].text)
        # Find the longest k such that prev's suffix (length k) equals
        # next's prefix (length k). The overlap parameter requires k >= 60.
        max_k = min(len(prev_tokens), len(next_tokens))
        overlap_len = 0
        for k in range(min(200, max_k), 0, -1):
            if prev_tokens[-k:] == next_tokens[:k]:
                overlap_len = k
                break
        assert overlap_len >= 60, (
            f"chunks {i} and {i + 1} share only {overlap_len} boundary tokens; "
            f"spec requires >= 60"
        )


def test_chunk_text_short_returns_single_chunk() -> None:
    """Scenario: Short text produces single chunk."""
    text = "alpha\nbeta\ngamma\n"
    chunks = chunk_text(text, chunk_size=600, overlap=60)

    assert len(chunks) == 1
    only = chunks[0]
    assert only.line_start == 1
    assert only.line_end == 3
    assert only.text == text


def test_chunk_text_empty_returns_empty() -> None:
    """Scenario: Empty text produces empty list."""
    assert chunk_text("", chunk_size=600, overlap=60) == []
    # No accidental TypeError / ValueError when caller hands in whitespace either.
    assert chunk_text("   \n  \n", chunk_size=600, overlap=60) != []
