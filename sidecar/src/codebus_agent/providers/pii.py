"""PIIProvider Protocol + concrete detectors for the Sanitizer pipeline.

Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/pii-provider/spec.md
  Requirement: PIIProvider Protocol exposes detect-shaped interface
  Requirement: RuleBasedPIIProvider wraps existing default_rules
  Requirement: MockPIIProvider supports test scripting
  Requirement: TrackedProvider gates PII inner classes via PII_ALLOWED_INNER_TYPES
  Requirement: TrackedProvider auto-bypasses Pass 2 for PII inner
  Requirement: Future LLM-based PII providers extend allowlist additively

D-033 Change A: this module is the single home for the third Provider
abstraction (LLM / Embedding / PII). PIIProvider deliberately lives in
``providers/`` rather than ``sanitizer/`` because (a) it shares the
allowlist + TrackedProvider gating pattern with LLMProvider /
EmbeddingProvider, and (b) future LLM-based PII providers will share
audit infrastructure with the LLM lane (per `usage-tracking` spec
``AuditRole enumerates legal role values`` Requirement).

The Protocol is detect-only: placeholder rendering and audit emission
remain ``SanitizerEngine``'s job (D-033 Decision 1). This lets the
Engine remain the single owner of the placeholder format
``<REDACTED:kind#N>`` (D-015 invariant) regardless of which PIIProvider
backend is injected — rule-based, future LLM-based, or hybrid.

``RuleBasedPIIProvider`` and ``MockPIIProvider`` ship in this change.
``LocalLLMPIIProvider`` / ``OpenAIPIIDetectionProvider`` are explicit
non-goals for D-033 Change A (the spec ``Future LLM-based PII
providers extend allowlist additively`` Requirement records the
extension contract for whichever change introduces them).
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Protocol, runtime_checkable

from ..sanitizer.rules import Rule, default_rules

if TYPE_CHECKING:
    from ..sanitizer.config import SanitizerConfig


@dataclass(frozen=True)
class PIISpan:
    """Single PII hit within an input text.

    Five-field shape mirrors ``codebus_agent.sanitizer.rules.RuleMatch``
    so the existing rule-table output can be re-emitted as ``PIISpan``
    without semantic translation. ``rule_id`` and ``kind`` are stable
    identifiers used by ``SanitizerEngine`` for audit emission and
    placeholder kind selection.
    """

    rule_id: str
    kind: str
    start: int
    end: int
    value: str


@runtime_checkable
class PIIProvider(Protocol):
    """Structural contract every PII detection backend satisfies.

    Implementations return zero or more :class:`PIISpan` describing
    where PII appears in the input text. The Provider does NOT apply
    placeholders, write audit entries, or otherwise mutate state — the
    Engine consumes the spans and renders the redaction pipeline.

    The method is declared ``async`` regardless of implementation
    strategy: rule-based providers (e.g., :class:`RuleBasedPIIProvider`)
    perform pure-CPU regex scanning with no real ``await`` suspension
    point, while future LLM-based providers will issue audited LLM
    calls. Both must conform to the same async signature so the
    Engine's ``await pii_provider.detect(text)`` site stays uniform
    (D-033 Decision 3).
    """

    async def detect(self, text: str) -> list[PIISpan]:
        """Return PII spans found in ``text`` (left-to-right order)."""
        ...


class RuleBasedPIIProvider:
    """Default PIIProvider — wraps the existing built-in rule table.

    Backs `RuleBasedPIIProvider wraps existing default_rules` Requirement.
    The class is a thin packaging layer: ``default_rules()`` (the
    sanitizer-safety-chain rule table) is the single source of truth for
    rule patterns / kinds / rule_ids; this Provider exposes them through
    the new :class:`PIIProvider` Protocol without altering any pattern.

    ``config`` is accepted to satisfy the constructor contract declared
    by the spec but is currently unused — allowlist application stays
    on :class:`SanitizerEngine` (Decision 1: Engine owns placeholder +
    audit; Provider only detects). The parameter exists so future
    allowlist-aware fast-path optimisations stay additive.
    """

    def __init__(
        self,
        rules: list[Rule] | None = None,
        *,
        config: "SanitizerConfig | None" = None,
    ) -> None:
        self._rules: list[Rule] = list(rules) if rules is not None else default_rules()
        self._config = config

    async def detect(self, text: str) -> list[PIISpan]:
        """Run every rule, collect matches, return spans sorted by start.

        Overlap resolution is intentionally NOT performed here — that
        responsibility lives on :class:`SanitizerEngine` so multiple
        providers (rule + future LLM) can be unioned cleanly. Each rule
        match is converted to :class:`PIISpan` (same five fields as
        ``RuleMatch`` so the conversion is field-by-field).
        """
        spans: list[PIISpan] = []
        for rule in self._rules:
            for match in rule.find(text):
                spans.append(
                    PIISpan(
                        rule_id=match.rule_id,
                        kind=match.kind,
                        start=match.start,
                        end=match.end,
                        value=match.value,
                    )
                )
        spans.sort(key=lambda span: span.start)
        return spans


class MockPIIProvider:
    """Test-only PIIProvider — script-driven, mirrors :class:`MockProvider`.

    Backs `MockPIIProvider supports test scripting` Requirement. Each
    ``detect`` call records the input text in :attr:`calls` and returns
    one entry from the constructor-supplied ``script`` list. Once the
    script is exhausted (or if no script was provided) ``detect``
    returns an empty list.

    The mock deliberately does NOT inspect the input text — its return
    value is determined solely by ``script``, so unit tests can assert
    Engine behaviour against arbitrary detection sequences without
    depending on real rule patterns.
    """

    def __init__(self, script: list[list[PIISpan]] | None = None) -> None:
        self._script: list[list[PIISpan]] | None = (
            list(script) if script is not None else None
        )
        self._cursor: int = 0
        self.calls: list[str] = []

    async def detect(self, text: str) -> list[PIISpan]:
        self.calls.append(text)
        if self._script is None or self._cursor >= len(self._script):
            return []
        result = self._script[self._cursor]
        self._cursor += 1
        return list(result)
