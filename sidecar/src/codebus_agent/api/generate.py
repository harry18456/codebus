"""``POST /generate`` endpoint — Module 5 Generator under the task registry.

Backs Requirements in
`openspec/changes/module-5-generator-p0/specs/sidecar-runtime/spec.md`:
  - ``task_id format`` (``generate`` kind matches ``^generate_[0-9a-f]{8}$``)
  - ``Background task error containment`` (``GENERATE_FAILED`` error code)

Endpoint shape mirrors ``POST /explore`` (same ``_run_background_task``
wrapping + ``TaskHandleEmitter`` SSE wiring + 409 ``TASK_IN_FLIGHT`` on
the single-slot registry). The endpoint stays thin — actual generation
lives in ``codebus_agent.generator.runner.run_generator``.
"""
from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any

from fastapi import APIRouter, HTTPException, Request, status
from pydantic import BaseModel, ConfigDict, Field

from codebus_agent.agent.emitter import TaskHandleEmitter
from codebus_agent.agent.types import ExplorerState, Station
from codebus_agent.api.tasks import (
    TaskRegistry,
    _classify_exception,
    _run_background_task,
)
from codebus_agent.generator.runner import run_generator
from codebus_agent.generator.types import GeneratorOptions

logger = logging.getLogger(__name__)


router = APIRouter()


class GenerateRequest(BaseModel):
    """Request body for ``POST /generate``."""

    model_config = ConfigDict(extra="forbid")

    workspace_root: str
    task: str = Field(min_length=1)
    stations: list[Station] = Field(default_factory=list)
    options: GeneratorOptions = Field(default_factory=GeneratorOptions)


def _validate_workspace_root(raw: str) -> Path:
    """Resolve + assert the path exists and is a directory.

    Mirrors ``api/explore.py::_validate_workspace_root`` so the error
    code surface stays consistent across endpoints.
    """
    workspace_root = Path(raw)
    if not workspace_root.exists() or not workspace_root.is_dir():
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "GENERATE_WORKSPACE_INVALID",
                "message": (
                    f"workspace_root {raw!r} does not exist or is not a directory"
                ),
            },
        )
    return workspace_root


def _require_generate_factory(request: Request):
    """Pull the chat-ish factory the generator needs from ``app.state``.

    Spec wording uses ``llm_chat_provider`` as the parameter name, but
    the production wiring layer (``api/__init__.py::wire_kb_dependencies``)
    binds an ``llm_generate_provider`` slot tagged ``module="generate"``
    so ``token_usage.jsonl`` lines split cleanly from the chat / judge /
    coverage lanes.
    """
    factory = getattr(request.app.state, "llm_generate_provider", None)
    if factory is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": "GENERATE_NOT_CONFIGURED",
                "message": "generate dependencies not initialized on this sidecar",
                "missing": ["llm_generate_provider"],
            },
        )
    return factory


def _classify_generate_exception(exc: BaseException) -> str:
    """Force unmapped exceptions into ``GENERATE_FAILED`` for /generate.

    Specific OpenAI / KB exceptions still flow through their typed
    branches in ``_classify_exception`` (auth / rate-limit / context-
    length), but everything else gets the safe generate-task code so
    the wire never leaks ``repr(exc)`` shape. Spec scenario
    ``Generate task exception surfaces as safe error event``.
    """
    code = _classify_exception(exc)
    if code == "INTERNAL_ERROR":
        return "GENERATE_FAILED"
    return code


@router.post("/generate", status_code=status.HTTP_202_ACCEPTED)
async def generate_endpoint(
    request: GenerateRequest, http_request: Request
) -> dict[str, str]:
    """Spawn a background generator run and return ``{"task_id": ...}``.

    Concurrency: 409 ``TASK_IN_FLIGHT`` when the registry already holds
    a running task (any kind). Errors raised by ``run_generator``
    surface through ``_run_background_task`` as a sanitized SSE
    ``error`` event with ``code="GENERATE_FAILED"``.
    """
    workspace_root = _validate_workspace_root(request.workspace_root)
    factory = _require_generate_factory(http_request)

    registry: TaskRegistry = http_request.app.state.tasks
    handle = registry.create("generate")
    if handle is None:
        running = registry.current_running()
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_IN_FLIGHT",
                "running_task_id": running.id if running else None,
            },
        )

    emitter = TaskHandleEmitter(handle)
    state = ExplorerState(
        task=request.task,
        stations=list(request.stations),
        budget_steps_left=0,
        budget_tokens_left=0,
    )

    async def _coro_factory() -> dict[str, Any]:
        result = await run_generator(
            state=state,
            workspace_root=workspace_root,
            task_id=handle.id,
            llm_chat_provider=factory,
            options=request.options,
            emitter=emitter,
        )
        return result.model_dump(mode="json")

    asyncio.create_task(
        _run_background_task(
            handle, _coro_factory, classify=_classify_generate_exception
        )
    )
    return {"task_id": handle.id}


__all__ = ["GenerateRequest", "generate_endpoint", "router"]
