"""Provider registry — M1 invariant guard.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: No outbound LLM traffic during M1
    Scenario: Only MockProvider registered

The registry is the single point where a `LLMProvider` instance can
be made discoverable to the rest of the sidecar.  During M1 the
allow-list contains only `MockProvider`, so any attempt to register
an OpenAI / Anthropic / Gemini / Ollama adapter raises before an
outbound HTTP client is ever constructed.
"""
from __future__ import annotations

from typing import ClassVar

from .mock import MockProvider
from .protocol import LLMProvider


class ProviderRegistryError(RuntimeError):
    """Raised when a provider fails the M1 registration guard."""


class ProviderRegistry:
    ALLOWED_TYPES: ClassVar[frozenset[type]] = frozenset({MockProvider})

    def __init__(self) -> None:
        self._providers: dict[str, LLMProvider] = {}

    def register(self, provider: object) -> None:
        if type(provider) not in self.ALLOWED_TYPES:
            raise ProviderRegistryError(
                f"M1 invariant: only {{{', '.join(t.__name__ for t in self.ALLOWED_TYPES)}}} "
                f"may be registered; got {type(provider).__name__}. "
                "No outbound LLM traffic is permitted during M1."
            )
        if not isinstance(provider, LLMProvider):
            raise ProviderRegistryError(
                f"{type(provider).__name__} does not satisfy the LLMProvider Protocol"
            )
        name = getattr(provider, "name", None)
        if not isinstance(name, str) or not name:
            raise ProviderRegistryError(
                f"provider instance must expose a non-empty `name` attribute"
            )
        self._providers[name] = provider

    def get(self, name: str) -> LLMProvider:
        return self._providers[name]

    @property
    def names(self) -> list[str]:
        return list(self._providers.keys())
