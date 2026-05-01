"""Sidecar config loaders.

D-033 B introduces a two-level provider pool schema (`llm.providers[]`
+ `llm.bindings`) with a backwards-compatible loader for the legacy
`llm.roles.<role>` shape — see `provider_pool.py` for the public API.
"""
from __future__ import annotations

from .provider_pool import (
    INVALID_PII_PROVIDER,
    INVALID_PROVIDER_BINDING,
    INVALID_PROVIDER_TYPE,
    ProviderPoolConfigError,
    ProviderPoolSnapshot,
    ProviderSpec,
    load_provider_pool,
)

__all__ = [
    "INVALID_PII_PROVIDER",
    "INVALID_PROVIDER_BINDING",
    "INVALID_PROVIDER_TYPE",
    "ProviderPoolConfigError",
    "ProviderPoolSnapshot",
    "ProviderSpec",
    "load_provider_pool",
]
