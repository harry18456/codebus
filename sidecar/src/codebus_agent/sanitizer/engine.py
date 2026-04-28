"""SanitizerEngine — pure `sanitize(text, source)` with fail-closed guard.

Backs SHALL clauses in
openspec/changes/sanitizer-safety-chain/specs/sanitizer/spec.md
  Requirement: SanitizerEngine exposes pure `sanitize` interface
  Requirement: Placeholder format is `<REDACTED:kind#index>`
  Requirement: Placeholder index scope is single sanitize call
  Requirement: Allowlist hits still audited but not redacted
    (allowlist pipeline wiring lands in a follow-up task; engine
     accepts optional pre-computed allowlist decisions per-match.)

Per Decisions "Placeholder index — 單檔 scope、session-less、in-memory"
and "Fail-closed 失敗處理":
- Index scope is one `sanitize()` call. No state survives the return.
- Any unrecoverable rule error is wrapped in `SanitizerError` with the
  offending source in the message and the original exception chained
  via `__cause__`.
- No method on this class accepts a placeholder and returns its
  pre-sanitize value; the `(kind, value) → index` map lives only on
  the stack.
"""
from __future__ import annotations

import fnmatch
from dataclasses import dataclass, field
from pathlib import PurePosixPath
from typing import TYPE_CHECKING, Any

from .config import SanitizerConfig

if TYPE_CHECKING:
    # D-033 Decision 4 — Engine consumes ``PIIProvider`` (provided by
    # callers via the ``providers`` package) instead of holding rule
    # instances directly. The TYPE_CHECKING guard avoids a circular
    # import: the providers package imports the rule table at runtime,
    # so ``sanitizer.engine`` MUST NOT import from ``providers`` at
    # runtime.
    from ..providers.pii import PIIProvider, PIISpan


class SanitizerError(RuntimeError):
    """Raised when sanitization fails unrecoverably (fail-closed)."""


@dataclass(frozen=True)
class FileSource:
    """Pass 1 (scanner) / Pass 3 (Q&A add_to_kb) source tag.

    ``pass_`` is an optional lifecycle label.  When empty (the sanitizer-
    safety-chain default) the audit log emits ``source`` as the legacy
    ``"file:<path>"`` string, preserving the archived schema.  When set
    (scanner passes ``"scanner"`` per
    ``openspec/changes/scanner-sanitizer-orchestration``) the audit log
    emits ``source`` as a structured ``{"pass": ..., "path": ...}``
    object so downstream tooling can key on the pass label directly.
    """

    path: str
    pass_: str = ""


@dataclass(frozen=True)
class MessageSource:
    message_id: str


SanitizeSource = FileSource | MessageSource


@dataclass(frozen=True)
class AuditEntry:
    """Single Pass-N hit record.

    ``source`` carries either the legacy string form (``"file:<path>"``
    / ``"message:<id>"``) or the structured dict form introduced by
    scanner-sanitizer-orchestration (``{"pass": "scanner", "path": ...}``).
    JSON serialization handles both transparently.
    """

    rule_id: str
    kind: str
    placeholder_index: int
    source: "str | dict[str, Any]"
    extra: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class SanitizedResult:
    text: str
    entries: list[AuditEntry]


class SanitizerEngine:
    """Deterministic, stateless-across-calls sanitizer.

    Span discovery is delegated to the injected ``PIIProvider``;
    callers typically pass ``RuleBasedPIIProvider()`` (which wraps the
    built-in rule table) but tests and future LLM-based PII flows can
    inject other implementations.
    ``config`` is optional — when provided, its allowlists are consulted
    to mark matched spans as ``extra.allowlisted = true`` (and skip the
    textual replacement for those spans).
    """

    def __init__(
        self,
        pii_provider: "PIIProvider | None" = None,
        *,
        config: SanitizerConfig | None = None,
    ) -> None:
        # D-033 Decision 4: Engine no longer owns the rule table; it
        # consumes a ``PIIProvider`` (typically ``RuleBasedPIIProvider``)
        # and delegates span discovery via ``await pii_provider.detect(text)``.
        # ``pii_provider=None`` lazily resolves to the default
        # ``RuleBasedPIIProvider()`` so ``SanitizerEngine()`` continues to
        # construct the default rule-based pipeline; lazy import avoids the
        # circular ``sanitizer ↔ providers`` package dependency.
        if pii_provider is None:
            from ..providers.pii import RuleBasedPIIProvider

            pii_provider = RuleBasedPIIProvider()
        self._pii_provider = pii_provider
        self._config = config

    async def sanitize(self, text: str, source: SanitizeSource) -> SanitizedResult:
        formatted_source = _format_source(source)
        source_label = _format_source_label(source)

        try:
            spans = await self._pii_provider.detect(text)
        except BaseException as exc:
            raise SanitizerError(
                f"sanitize failed on {source_label}"
            ) from exc

        spans = _resolve_overlaps(spans)

        path_hit, filename_hit = self._path_allowlist_hit(source)
        pattern_allowlist = self._compiled_pattern_allowlist()

        # Walk left-to-right and emit placeholders at each unmasked span.
        out_parts: list[str] = []
        cursor = 0
        per_kind_next_index: dict[str, int] = {}
        per_kind_value_index: dict[tuple[str, str], int] = {}
        entries: list[AuditEntry] = []

        for span in spans:
            pattern_allowed = _pattern_allowlist_hit(span.value, pattern_allowlist)
            allowlisted = path_hit or filename_hit or pattern_allowed

            key = (span.kind, span.value)
            if key in per_kind_value_index:
                index = per_kind_value_index[key]
                first_seen = False
            else:
                index = per_kind_next_index.get(span.kind, 0) + 1
                per_kind_next_index[span.kind] = index
                per_kind_value_index[key] = index
                first_seen = True

            if allowlisted:
                # Leave the original text in place; still consume input span.
                out_parts.append(text[cursor : span.end])
            else:
                out_parts.append(text[cursor : span.start])
                out_parts.append(f"<REDACTED:{span.kind}#{index}>")
            cursor = span.end

            if first_seen:
                extra: dict[str, Any] = {}
                if allowlisted:
                    extra["allowlisted"] = True
                entries.append(
                    AuditEntry(
                        rule_id=span.rule_id,
                        kind=span.kind,
                        placeholder_index=index,
                        source=formatted_source,
                        extra=extra,
                    )
                )

        out_parts.append(text[cursor:])
        return SanitizedResult(text="".join(out_parts), entries=entries)

    def _path_allowlist_hit(
        self, source: SanitizeSource
    ) -> tuple[bool, bool]:
        """Return (path_allowlist_hit, filename_allowlist_hit)."""
        if self._config is None or not isinstance(source, FileSource):
            return (False, False)
        normalized = source.path.replace("\\", "/")
        path_hit = any(
            fnmatch.fnmatch(normalized, pat)
            for pat in self._config.path_allowlist
        )
        filename = PurePosixPath(normalized).name
        filename_hit = any(
            fnmatch.fnmatch(filename, pat)
            for pat in self._config.filename_allowlist
        )
        return (path_hit, filename_hit)

    def _compiled_pattern_allowlist(self) -> list[Any]:
        if self._config is None:
            return []
        import re

        compiled: list[Any] = []
        for entry in self._config.pattern_allowlist:
            try:
                compiled.append(re.compile(entry.pattern))
            except re.error:
                # Skip invalid patterns per `docs/sanitizer.md §六`:
                # "使用者 pattern regex 編譯錯 → UI 錯誤提示 → 該規則不載入"
                continue
        return compiled


def _format_source(source: SanitizeSource) -> str | dict[str, Any]:
    """Serialize ``source`` for the audit entry payload.

    FileSource with a non-empty ``pass_`` yields a structured
    ``{"pass": ..., "path": ...}`` dict — this is the format scanner
    Pass 1 writes so the Trust-Layer inspector can filter on
    ``source.pass``.  All other callers fall back to the legacy
    ``"file:<path>"`` / ``"message:<id>"`` string so the archived
    sanitizer-safety-chain audit schema stays intact.
    """
    if isinstance(source, FileSource):
        if source.pass_:
            return {"pass": source.pass_, "path": source.path}
        return f"file:{source.path}"
    if isinstance(source, MessageSource):
        return f"message:{source.message_id}"
    raise TypeError(f"unknown SanitizeSource type: {type(source).__name__}")


def _format_source_label(source: SanitizeSource) -> str:
    """Human-readable source label for ``SanitizerError`` messages.

    The SanitizerEngine promises fail-closed errors identify the source
    — scanner-safety-chain's red tests assert ``"file:src/app.py"``
    appears in the error text regardless of whether the audit payload
    is a string or a dict.
    """
    if isinstance(source, FileSource):
        return f"file:{source.path}"
    if isinstance(source, MessageSource):
        return f"message:{source.message_id}"
    raise TypeError(f"unknown SanitizeSource type: {type(source).__name__}")


def _resolve_overlaps(spans: "list[PIISpan]") -> "list[PIISpan]":
    """Sort by (start, -length) and greedily drop overlaps.

    Ties go to the longer span; a later span that starts before the
    previous one's end is discarded so we never emit two placeholders
    for the same substring. Operates structurally on the ``start`` /
    ``end`` fields shared by :class:`PIISpan` and the legacy
    ``RuleMatch`` shape.
    """
    if not spans:
        return []
    spans.sort(key=lambda s: (s.start, -(s.end - s.start)))
    out: list = []
    cursor = -1
    for span in spans:
        if span.start < cursor:
            continue
        out.append(span)
        cursor = span.end
    return out


def _pattern_allowlist_hit(value: str, compiled: list[Any]) -> bool:
    return any(p.search(value) for p in compiled)
