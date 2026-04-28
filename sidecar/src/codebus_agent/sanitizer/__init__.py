"""Sanitizer package — three-pass redaction pipeline.

Backs SHALL clauses in
openspec/changes/sanitizer-safety-chain/specs/sanitizer/spec.md

Pass 1: Scanner → KB
Pass 2: LLMProvider dispatch pre-flight (TrackedProvider)
Pass 3: Q&A add_to_kb (slot reserved; wired in a later change)

Placeholder format `<REDACTED:kind#index>` is one-way — no reverse
mapping is retained, per D-015.
"""
from __future__ import annotations

from .audit import SanitizerAuditLogger
from .config import RULES_VERSION, PatternAllowlistEntry, SanitizerConfig
from .engine import (
    AuditEntry,
    FileSource,
    MessageSource,
    SanitizedResult,
    SanitizeSource,
    SanitizerEngine,
    SanitizerError,
)

__all__ = [
    "AuditEntry",
    "FileSource",
    "MessageSource",
    "PatternAllowlistEntry",
    "RULES_VERSION",
    "SanitizedResult",
    "SanitizeSource",
    "SanitizerAuditLogger",
    "SanitizerConfig",
    "SanitizerEngine",
    "SanitizerError",
    "make_default_engine",
]


def make_default_engine(
    config: SanitizerConfig | None = None,
) -> SanitizerEngine:
    """Build a :class:`SanitizerEngine` with the default rule-based PII detector.

    Backs spec MODIFIED `Built-in rule set covers Secret, PII,
    internal-identifier kinds` Requirement (post-D-033). Callers that
    previously instantiated ``SanitizerEngine()`` with no PII provider
    can switch to ``make_default_engine()`` to preserve the M1
    behaviour without depending on the (now deprecated) implicit
    rule-table loading inside the Engine.

    The lazy import of :class:`RuleBasedPIIProvider` keeps the
    ``sanitizer`` package free of runtime dependencies on
    ``providers``; the Engine itself stays Type-Check-only coupled to
    ``providers.pii`` per the import-graph rationale in
    ``codebus_agent.sanitizer.engine``.
    """
    from ..providers.pii import RuleBasedPIIProvider

    return SanitizerEngine(
        pii_provider=RuleBasedPIIProvider(),
        config=config,
    )
