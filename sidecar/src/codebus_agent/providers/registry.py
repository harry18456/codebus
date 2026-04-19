"""Provider registry — M1 invariant guard.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: No outbound LLM traffic during M1
    Scenario: Only MockProvider registered

and openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Direct provider use forbidden
    Scenario: Skipping wrapper emits test failure

The registry accepts only `TrackedProvider` instances.  Combined with
`TrackedProvider.ALLOWED_INNER_TYPES` (= `{MockProvider}`), this gives
two independent checks that must both pass before any `chat` / `embed`
call is reachable from application code.
"""
from __future__ import annotations

from typing import ClassVar

from .protocol import LLMProvider
from .tracked import TrackedProvider


class ProviderRegistryError(RuntimeError):
    """Raised when a provider fails the M1 registration guard."""


class ProviderRegistry:
    ALLOWED_TYPES: ClassVar[frozenset[type]] = frozenset({TrackedProvider})

    def __init__(self) -> None:
        self._providers: dict[str, LLMProvider] = {}

    def register(self, provider: object) -> None:
        if type(provider) not in self.ALLOWED_TYPES:
            raise ProviderRegistryError(
                f"M1 invariant: only {{{', '.join(t.__name__ for t in self.ALLOWED_TYPES)}}} "
                f"may be registered; got {type(provider).__name__}. "
                "Wrap the inner provider in TrackedProvider before registering."
            )
        if not isinstance(provider, LLMProvider):
            raise ProviderRegistryError(
                f"{type(provider).__name__} does not satisfy the LLMProvider Protocol"
            )
        name = getattr(provider, "name", None)
        if not isinstance(name, str) or not name:
            raise ProviderRegistryError(
                "provider instance must expose a non-empty `name` attribute"
            )
        self._providers[name] = provider

    def get(self, name: str) -> LLMProvider:
        return self._providers[name]

    @property
    def names(self) -> list[str]:
        return list(self._providers.keys())
