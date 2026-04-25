"""Markdown validator — D-029 component rules.

Backs Requirement
``Markdown validator enforces D-029 component rules``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Returns a ``ValidationResult`` with all violations collected — the
validator never raises on bad markdown; it lists every issue so the
retry path can feed the union back into the next prompt as a
correction hint.
"""
from __future__ import annotations

import re
from pathlib import Path
from typing import Literal

from .types import ValidationResult

__all__ = ["validate_station_markdown"]


_BODY_LIMIT_CHARS: int = 800
_CODE_BLOCK_LINE_LIMIT: int = 30
_QUIZ_VALID_CORRECT: frozenset[str] = frozenset({"a", "b", "c", "d"})

_FRONTMATTER_RE = re.compile(r"^---\n.*?\n---\n", re.DOTALL)
_CHECKPOINT_TAG_RE = re.compile(
    r"<Checkpoint\b([^>]*)>", re.IGNORECASE
)
_QUIZ_TAG_RE = re.compile(r"<Quiz\b([^>]*)>", re.IGNORECASE)
_CODEREF_TAG_RE = re.compile(r"<CodeRef\b([^>]*?)/?>", re.IGNORECASE)
_ATTR_RE = re.compile(
    r"""(\w+)\s*=\s*(?:"([^"]*)"|'([^']*)'|(\S+))""",
    re.IGNORECASE,
)
_CODE_FENCE_RE = re.compile(r"^```", re.MULTILINE)


def validate_station_markdown(
    md: str,
    *,
    station_idx: int,
    mode: Literal["interactive", "plain"],
    workspace_root: Path,
) -> ValidationResult:
    """Run all D-029 component rules over ``md`` and collect issues.

    ``station_idx`` is the 1-based station position; reserved for
    future per-station rule variants. ``workspace_root`` anchors
    ``<CodeRef file=...>`` validation: any ``file`` that resolves
    outside the workspace lands in ``coderef_escape`` issues.

    ``mode`` controls component-specific gating: in ``"plain"`` mode
    the validator MUST NOT emit ``missing_checkpoint`` /
    ``too_many_quizzes`` / ``quiz_*`` / ``coderef_*`` issues — those
    components are absent by spec §六. Length and code-block-line
    rules apply in both modes.
    """
    issues: list[str] = []
    parsed: dict[str, list[str]] = {"required_checks": []}

    body = _strip_frontmatter(md)

    # Length rule — body excludes the YAML frontmatter (spec §五.5).
    if len(body) > _BODY_LIMIT_CHARS:
        issues.append("too_long")

    # Code-block line-count rule — fenced ``` blocks only (the spec
    # examples use triple-backtick fences; HTML-tagged blocks aren't
    # in scope for D-029 P0).
    for block in _iter_code_blocks(body):
        if block.count("\n") + 1 > _CODE_BLOCK_LINE_LIMIT:
            issues.append("code_block_too_long")
            break

    # Component scan — Checkpoint, Quiz, CodeRef
    checkpoints: list[str] = [
        _attr(attrs).get("id", "")
        for attrs in (m.group(1) for m in _CHECKPOINT_TAG_RE.finditer(body))
    ]
    quizzes: list[dict[str, str]] = [
        _attr(m.group(1)) for m in _QUIZ_TAG_RE.finditer(body)
    ]
    coderefs: list[dict[str, str]] = [
        _attr(m.group(1)) for m in _CODEREF_TAG_RE.finditer(body)
    ]

    if mode == "interactive":
        if not checkpoints:
            issues.append("missing_checkpoint")

        if len(quizzes) > 1:
            issues.append("too_many_quizzes")

        for quiz in quizzes:
            correct = quiz.get("correct", "")
            if correct not in _QUIZ_VALID_CORRECT:
                issues.append(f"quiz_bad_correct: {correct}")

        for ref in coderefs:
            file_path = ref.get("file", "")
            if file_path and _is_outside_workspace(file_path, workspace_root):
                issues.append(f"coderef_escape: {file_path}")

    # `parsed.required_checks` aggregates ids visible to downstream
    # frontmatter rendering — populated for both modes so plain mode
    # task lists can still surface stable check ids when present.
    parsed["required_checks"] = [c for c in checkpoints if c]
    if quizzes:
        for q in quizzes:
            qid = q.get("id", "")
            if qid:
                parsed["required_checks"].append(qid)

    return ValidationResult(issues=issues, parsed=parsed)


def _strip_frontmatter(md: str) -> str:
    return _FRONTMATTER_RE.sub("", md, count=1)


def _attr(attr_chunk: str) -> dict[str, str]:
    """Parse a tag's attribute list ``key="value"`` pairs.

    Tolerant: unquoted values fall through to the third group; missing
    quotes still produce a usable string. The parser intentionally
    stays simple — content is LLM-generated structured markdown, not
    arbitrary HTML.
    """
    out: dict[str, str] = {}
    for match in _ATTR_RE.finditer(attr_chunk):
        name = match.group(1).lower()
        value = match.group(2) or match.group(3) or match.group(4) or ""
        out[name] = value
    return out


def _iter_code_blocks(body: str) -> list[str]:
    """Return the inner text of every fenced ``` code block.

    Pairs fences left-to-right. Unmatched trailing fences are ignored.
    """
    fences = list(_CODE_FENCE_RE.finditer(body))
    if len(fences) < 2:
        return []
    blocks: list[str] = []
    for i in range(0, len(fences) - 1, 2):
        start = fences[i].end()
        end = fences[i + 1].start()
        if start < end:
            blocks.append(body[start:end])
    return blocks


def _is_outside_workspace(file_attr: str, workspace_root: Path) -> bool:
    """Return True when ``file_attr`` does not resolve under ``workspace_root``.

    Accepts paths whether or not the workspace root currently exists
    on disk (validator runs against an in-memory string, not against
    the live filesystem). Resolution uses ``Path.resolve(strict=False)``
    so ``..`` segments still normalize.
    """
    candidate = (workspace_root / file_attr).resolve()
    root = workspace_root.resolve()
    try:
        candidate.relative_to(root)
    except ValueError:
        return True
    return False
