"""Provider registry — role-aware dispatch + wrapping invariant.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: No outbound LLM traffic during M1
    Scenario: Only MockProvider registered

and openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Direct provider use forbidden
    Scenario: Skipping wrapper emits test failure

and openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: Registry dispatches provider by role
    Scenario: Registry returns role-specific provider
    Scenario: Registry raises on missing role
  Requirement: Registry enforces TrackedProvider wrapping per role
    Scenario: Unwrapped provider in any role raises
    Scenario: Wrapped providers in every role succeed

Design llm-role-routing §3: the guard fires at instantiation only —
`get(role)` is a raw dict lookup so the hot path stays O(1) without
re-validating on every LLM call.

Combined with `TrackedProvider.ALLOWED_INNER_TYPES` (= `{MockProvider}`),
two independent checks must both pass before any `chat` / `embed` call
is reachable from application code.
"""
from __future__ import annotations

from typing import ClassVar

from .protocol import LLMProvider, ProviderRole
from .tracked import TrackedProvider


class ProviderRegistryError(ValueError):
    """Raised when a provider fails the registration guard.

    Inherits from `ValueError` so the llm-role-routing spec clause
    (`SHALL raise a ValueError ...`) is satisfied while preserving the
    M1-era `ProviderRegistryError` symbol for callers that caught it.
    """


class ProviderRegistry:
    ALLOWED_TYPES: ClassVar[frozenset[type]] = frozenset({TrackedProvider})

    def __init__(self, providers: dict[ProviderRole, LLMProvider]) -> None:
        for role, provider in providers.items():
            if not isinstance(role, ProviderRole):
                raise ProviderRegistryError(
                    f"registry keys must be ProviderRole members; "
                    f"got {role!r} of type {type(role).__name__}"
                )
            if type(provider) not in self.ALLOWED_TYPES:
                allowed = ", ".join(t.__name__ for t in self.ALLOWED_TYPES)
                raise ProviderRegistryError(
                    f"role {role.value!r}: only {{{allowed}}} may be registered; "
                    f"got {type(provider).__name__}. "
                    "Wrap the inner provider in TrackedProvider before registering."
                )
        self._providers: dict[ProviderRole, LLMProvider] = dict(providers)

    def get(self, role: ProviderRole) -> LLMProvider:
        try:
            return self._providers[role]
        except KeyError:
            raise KeyError(
                f"no provider registered for role {role.value!r}; "
                f"known roles: {[r.value for r in self._providers]}"
            ) from None

    @property
    def roles(self) -> list[ProviderRole]:
        return list(self._providers.keys())
