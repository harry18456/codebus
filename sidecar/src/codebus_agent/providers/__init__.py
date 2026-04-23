"""LLM provider abstraction — `docs/llm-provider.md`, D-003.

M1 scope (`openspec/changes/m1-power-on/specs/llm-provider/spec.md`
 + `openspec/changes/m1-power-on/specs/usage-tracking/spec.md`):
  - Protocol with `chat(messages, response_model)` and `embed(texts)`
  - `MockProvider` exercising the real Pydantic parsing path (D-local-4)
  - `UsageTracker` → `token_usage.jsonl` (D-021)
  - `LLMCallLogger` → `llm_calls.jsonl` (D-022)
  - `TrackedProvider` wrapping every provider, enforced by registry guard
"""
from __future__ import annotations

from .llm_call_logger import LLMCallLogger
from .mock import MockProvider, MockScript
from .openai_embedding import (
    OPENAI_EMBEDDING_DIM,
    OPENAI_EMBEDDING_MODEL,
    OpenAIAuthError,
    OpenAIEmbeddingProvider,
    OpenAIRateLimitError,
)
from .protocol import (
    EmbedResponse,
    LLMProvider,
    Message,
    ProviderRole,
    RoleConfig,
    Usage,
)
from .registry import ProviderRegistry, ProviderRegistryError
from .tracked import TrackedProvider
from .usage_tracker import UsageTracker

__all__ = [
    "EmbedResponse",
    "LLMCallLogger",
    "LLMProvider",
    "Message",
    "MockProvider",
    "MockScript",
    "OPENAI_EMBEDDING_DIM",
    "OPENAI_EMBEDDING_MODEL",
    "OpenAIAuthError",
    "OpenAIEmbeddingProvider",
    "OpenAIRateLimitError",
    "ProviderRegistry",
    "ProviderRegistryError",
    "ProviderRole",
    "RoleConfig",
    "TrackedProvider",
    "Usage",
    "UsageTracker",
]
