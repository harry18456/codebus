"""`POST /explore` endpoint — spawns Module 4 Explorer under the task registry.

Backs openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: POST /explore endpoint spawns Explorer under task registry

The endpoint is thin by design: validate the request, reserve a task
slot, wire the Explorer's dependencies (`ToolContext` / `FolderTools` /
`LLMJudge` / `ReasoningLogger` / `TaskHandleEmitter`), and hand the
blocking coroutine to ``_run_background_task`` so failures collapse
into the existing sanitized `error` event contract.

Authorization middleware for the workspace root is a follow-up change;
for MVP the endpoint runs an ``ensure_in_workspace``-style check
(``Path.exists() + is_dir()``) and refuses anything that doesn't
resolve to an existing directory.
"""
from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any

from fastapi import APIRouter, HTTPException, Request, status
from pydantic import BaseModel, ConfigDict, Field

from codebus_agent.agent.budget import AggregatedTokenProbe
from codebus_agent.agent.context_vars import current_phase_var, current_session_var
from codebus_agent.agent.coverage import LLMCoverageChecker
from codebus_agent.agent.emitter import TaskHandleEmitter
from codebus_agent.agent.explorer import run_explorer
from codebus_agent.agent.judge import LLMJudge
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.types import ExplorerState
from codebus_agent.api.tasks import TaskRegistry, _run_background_task
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine

# `audit-path-unification`: shared workspace-audit constants live in
# `api/_audit_paths.py` (leaf module to break circular import between
# `api/__init__.py` and this caller). ReasoningLogger does not auto-mkdir
# (per `agent-core` spec), so this caller MUST `mkdir(parents=True,
# exist_ok=True)` the `<workspace>/.codebus/` parent before constructing.
from codebus_agent.api._audit_paths import (
    _REASONING_LOG_FILENAME,
    _WORKSPACE_AUDIT_SUBDIR,
)


logger = logging.getLogger(__name__)

router = APIRouter()


class ExploreRequest(BaseModel):
    """Request body for ``POST /explore``."""

    model_config = ConfigDict(extra="forbid")

    workspace_root: str
    task: str = Field(min_length=1)
    budget_steps: int = Field(default=10, ge=0, le=200)
    budget_tokens: int = Field(default=50_000, ge=0)


def _validate_workspace_root(raw: str) -> Path:
    """Resolve + assert the path exists and is a directory.

    Spec scenario `Missing workspace root rejected` permits either 400
    or 404; we pick 400 to mirror `SCANNER_WORKSPACE_INVALID`.
    """
    workspace_root = Path(raw)
    if not workspace_root.exists() or not workspace_root.is_dir():
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "EXPLORE_WORKSPACE_INVALID",
                "message": (
                    f"workspace_root {raw!r} does not exist or is not a directory"
                ),
            },
        )
    return workspace_root


def _require_explore_deps(request: Request) -> tuple[Any, Any, Any]:
    """Pull reasoning + judge + coverage provider factories from ``app.state``.

    Mirrors ``_require_kb_deps`` — surface a 503 ``EXPLORE_NOT_CONFIGURED``
    when any factory is not wired (the only time this happens today is
    when ``CODEBUS_OPENAI_API_KEY`` is absent, per ``wire_kb_dependencies``).

    `coverage-gap-recurse` added `llm_coverage_provider` alongside the
    reasoning / judge slots per design Decision 7. All three are
    treated identically in the 503 path so the error message surfaces
    every missing slot name.
    """
    state = request.app.state
    reasoning_factory = getattr(state, "llm_reasoning_provider", None)
    judge_factory = getattr(state, "llm_judge_provider", None)
    coverage_factory = getattr(state, "llm_coverage_provider", None)
    if (
        reasoning_factory is None
        or judge_factory is None
        or coverage_factory is None
    ):
        missing = [
            name
            for name, val in (
                ("llm_reasoning_provider", reasoning_factory),
                ("llm_judge_provider", judge_factory),
                ("llm_coverage_provider", coverage_factory),
            )
            if val is None
        ]
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": "EXPLORE_NOT_CONFIGURED",
                "message": "explore dependencies not initialized on this sidecar",
                "missing": missing,
            },
        )
    return reasoning_factory, judge_factory, coverage_factory


@router.post("/explore", status_code=status.HTTP_202_ACCEPTED)
async def explore_endpoint(
    request: ExploreRequest, http_request: Request
) -> dict[str, str]:
    """Spawn a background Explorer run and return ``{"task_id": ...}``.

    Concurrency: 409 ``TASK_IN_FLIGHT`` when the registry already holds a
    running task. Errors raised by ``run_explorer`` surface through the
    ``_run_background_task`` wrapper as sanitized SSE ``error`` events.
    """
    workspace_root = _validate_workspace_root(request.workspace_root)
    (
        reasoning_factory,
        judge_factory,
        coverage_factory,
    ) = _require_explore_deps(http_request)

    registry: TaskRegistry = http_request.app.state.tasks
    handle = registry.create("explore")
    if handle is None:
        running = registry.current_running()
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_IN_FLIGHT",
                "running_task_id": running.id if running else None,
            },
        )

    reasoning_provider = reasoning_factory(workspace_root)
    judge = LLMJudge(judge_factory, workspace_root)
    coverage = LLMCoverageChecker(coverage_factory, workspace_root)
    emitter = TaskHandleEmitter(handle)
    # Factories build their TrackedProviders + inner LLMCallLoggers with
    # emitter=None because they're constructed at workspace-scope time,
    # before any per-task handle exists. Wire the per-task emitter now so
    # `usage_delta` / `llm_call` events land on the same SSE channel as
    # Explorer loop's own `agent_thought` / `agent_action_result` /
    # `judge_verdict` / `progress` / `coverage_gaps` emits.
    reasoning_provider.set_emitter(emitter)
    judge.set_emitter(emitter)
    coverage.set_emitter(emitter)
    # `context-compression-token-budget`: aggregate session token totals
    # across reasoning / judge / coverage providers so `run_explorer`'s
    # `_should_stop` can enforce `state.budget_tokens_left`.
    token_probe = AggregatedTokenProbe(
        [reasoning_provider, judge.provider, coverage.provider]
    )
    state_obj = ExplorerState(
        task=request.task,
        budget_steps_left=request.budget_steps,
        budget_tokens_left=request.budget_tokens,
    )
    ctx = ToolContext(
        workspace_root=workspace_root,
        workspace_type="folder",
        session_id=handle.id,
        sanitizer=SanitizerEngine(),
    )
    tools = FolderTools(ctx=ctx, state=state_obj)
    # ReasoningLogger does not auto-mkdir its parent (per `agent-core`
    # spec `Path stays under workspace and caller mkdirs .codebus parent`);
    # caller MUST ensure `<workspace>/.codebus/` exists before construction.
    audit_dir = workspace_root / _WORKSPACE_AUDIT_SUBDIR
    audit_dir.mkdir(parents=True, exist_ok=True)
    reasoning_logger = ReasoningLogger(audit_dir / _REASONING_LOG_FILENAME)

    async def _coro_factory() -> dict[str, Any]:
        # Scope phase / session for the duration of the run so downstream
        # TrackedProvider emits pick up `phase="explore"` + `session_id=<task>`.
        phase_token = current_phase_var.set("explore")
        session_token = current_session_var.set(handle.id)
        try:
            result = await run_explorer(
                state=state_obj,
                provider=reasoning_provider,
                tools=tools,
                judge=judge,
                coverage=coverage,
                logger=reasoning_logger,
                emitter=emitter,
                token_probe=token_probe,
            )
        finally:
            current_phase_var.reset(phase_token)
            current_session_var.reset(session_token)
        return result.model_dump(mode="json")

    asyncio.create_task(_run_background_task(handle, _coro_factory))
    return {"task_id": handle.id}


__all__ = ["router", "ExploreRequest", "explore_endpoint"]
