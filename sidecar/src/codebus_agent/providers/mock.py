"""MockProvider + MockScript.

Backs SHALL clauses in
openspec/changes/m1-power-on/specs/llm-provider/spec.md
  Requirement: Mock provider returns Instructor-compatible output
    Scenario: Mock chat satisfies response_model
    Scenario: Mock script controls output
    Scenario: Mock embed returns deterministic vector

Design D-local-4 requires that `chat` walks the same Pydantic
parsing path real providers will use.  We generate a dict from each
field's annotation, then call `response_model.model_validate(...)` so
validators actually fire.  Callers that need exact outputs (fixtures,
golden-sample tests) push entries onto a `MockScript` FIFO.
"""
from __future__ import annotations

import hashlib
from collections import deque
from dataclasses import dataclass, field
from types import UnionType
from typing import Any, Literal, Union, get_args, get_origin

from pydantic import BaseModel

from .protocol import EmbedResponse, Message, Usage

_EMBED_MODEL_ID = "mock-embed-v1"
_CHAT_MODEL_ID = "mock-chat-v1"


@dataclass
class MockScript:
    """FIFO of pinned chat responses.

    Tests push expected outputs before invoking `MockProvider.chat`;
    each successful `chat` call consumes one entry.  When empty, the
    provider falls back to auto-generation.
    """

    _pending: deque[BaseModel] = field(default_factory=deque)

    def push(self, response: BaseModel) -> None:
        if not isinstance(response, BaseModel):
            raise TypeError(
                f"MockScript only accepts Pydantic BaseModel instances, got {type(response).__name__}"
            )
        self._pending.append(response)

    def pop(self) -> BaseModel | None:
        if not self._pending:
            return None
        return self._pending.popleft()

    @property
    def empty(self) -> bool:
        return not self._pending


class MockProvider:
    """Deterministic provider for M1 + unit tests.

    The object does not perform any network I/O — this is enforced by
    the registry guard and by the `block_outbound_sockets` fixture.
    """

    name = "mock"

    def __init__(
        self, script: MockScript | None = None, embedding_dim: int = 8
    ) -> None:
        if embedding_dim <= 0:
            raise ValueError(f"embedding_dim must be > 0, got {embedding_dim}")
        self.script = script or MockScript()
        self.embedding_dim = embedding_dim

    async def chat(
        self,
        messages: list[Message],
        *,
        response_model: type[BaseModel],
    ) -> BaseModel:
        pinned = self.script.pop()
        if pinned is not None:
            if not isinstance(pinned, response_model):
                raise TypeError(
                    f"MockScript entry type {type(pinned).__name__} does not match "
                    f"requested response_model {response_model.__name__}"
                )
            return pinned

        dummy = _generate_dummy(response_model)
        return response_model.model_validate(dummy)

    async def embed(self, texts: list[str]) -> EmbedResponse:
        vectors = [_deterministic_vector(t, self.embedding_dim) for t in texts]
        usage = Usage(
            call_type="embed",
            model=_EMBED_MODEL_ID,
            embed_tokens=sum(len(t) for t in texts),
            estimated=True,
        )
        return EmbedResponse(vectors=vectors, usage=usage)


def _deterministic_vector(text: str, dim: int) -> list[float]:
    """SHA256-based vector; same text always yields the same vector."""
    digest = hashlib.sha256(text.encode("utf-8")).digest()
    return [((digest[i % len(digest)]) - 128) / 128.0 for i in range(dim)]


def _generate_dummy(model_cls: type[BaseModel]) -> dict[str, Any]:
    data: dict[str, Any] = {}
    for name, info in model_cls.model_fields.items():
        if not info.is_required():
            continue
        data[name] = _dummy_for_annotation(info.annotation)
    return data


def _dummy_for_annotation(annotation: Any) -> Any:
    origin = get_origin(annotation)
    args = get_args(annotation)

    if origin is Union or origin is UnionType:
        if type(None) in args:
            return None
        return _dummy_for_annotation(args[0])

    if origin is Literal:
        return args[0]

    if origin is list:
        return []
    if origin is dict:
        return {}
    if origin is tuple:
        return ()
    if origin is set:
        return set()

    if isinstance(annotation, type):
        if issubclass(annotation, bool):
            return False
        if issubclass(annotation, int):
            return 0
        if issubclass(annotation, float):
            return 0.0
        if issubclass(annotation, str):
            return ""
        if issubclass(annotation, BaseModel):
            return _generate_dummy(annotation)

    return None
