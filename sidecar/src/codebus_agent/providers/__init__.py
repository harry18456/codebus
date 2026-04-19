"""LLM provider abstraction — `docs/llm-provider.md`, D-003.

M1 scope (`openspec/changes/m1-power-on/specs/llm-provider/spec.md`):
  - Protocol with `chat(messages, response_model)` and `embed(texts)`
  - `MockProvider` that exercises the real Instructor / Pydantic parsing path
  - Registry guard rejecting any non-Mock provider (no outbound LLM traffic)
"""
from __future__ import annotations

from .mock import MockProvider, MockScript
from .protocol import EmbedResponse, LLMProvider, Message, Usage
from .registry import ProviderRegistry, ProviderRegistryError

__all__ = [
    "EmbedResponse",
    "LLMProvider",
    "Message",
    "MockProvider",
    "MockScript",
    "ProviderRegistry",
    "ProviderRegistryError",
    "Usage",
]
