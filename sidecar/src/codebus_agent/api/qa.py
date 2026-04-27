"""``POST /qa`` endpoint — Module 8 Q&A Agent under the task registry.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/sidecar-runtime/spec.md
  Requirement: Q&A task spawn endpoint
  Requirement: task_id format (qa kind)
  Requirement: Background task error containment (QA_FAILED)

Endpoint shape mirrors ``POST /explore`` and ``POST /generate``: same
``_run_background_task`` wrapping, same ``TaskHandleEmitter`` SSE
wiring, 409 ``TASK_IN_FLIGHT`` on the single-slot registry. The
endpoint stays thin — actual loop runs in
``codebus_agent.agent.qa.run_qa``.
"""
from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any

from fastapi import APIRouter, HTTPException, Request, status
from pydantic import BaseModel, ConfigDict, Field, field_validator

from codebus_agent.agent.emitter import TaskHandleEmitter
from codebus_agent.agent.qa import run_qa
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.station_id import _STATION_ID_RE
from codebus_agent.agent.tools.folder_tools import FolderTools
from codebus_agent.agent.tools.qa_tools import QATools
from codebus_agent.agent.types import ExplorerState, QAState
from codebus_agent.api._audit_paths import (
    _REASONING_LOG_FILENAME,
    _SANITIZE_AUDIT_FILENAME,
    _WORKSPACE_AUDIT_SUBDIR,
)
from codebus_agent.api.tasks import (
    TaskRegistry,
    _classify_exception,
    _run_background_task,
)
from codebus_agent.kb.knowledge_base import KnowledgeBase
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

logger = logging.getLogger(__name__)


router = APIRouter()


class QARequest(BaseModel):
    """Request body for ``POST /qa``."""

    model_config = ConfigDict(extra="forbid")

    workspace_root: str
    question: str = Field(min_length=1, max_length=4000)
    originating_station_id: str | None = None

    @field_validator("question", mode="after")
    @classmethod
    def _strip_question(cls, v: str) -> str:
        stripped = v.strip()
        if not stripped:
            raise ValueError("question MUST be non-empty after stripping whitespace")
        return stripped

    @field_validator("originating_station_id", mode="after")
    @classmethod
    def _validate_station_id(cls, v: str | None) -> str | None:
        if v is None:
            return None
        if not _STATION_ID_RE.fullmatch(v):
            raise ValueError(
                f"originating_station_id {v!r} must match {_STATION_ID_RE.pattern}"
            )
        return v


def _validate_workspace_root(raw: str) -> Path:
    """Resolve + assert the path exists and is a directory."""
    workspace_root = Path(raw)
    if not workspace_root.exists() or not workspace_root.is_dir():
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "QA_WORKSPACE_INVALID",
                "message": (
                    f"workspace_root {raw!r} does not exist or is not a directory"
                ),
            },
        )
    return workspace_root


def _require_qa_deps(state: Any) -> list[str]:
    """Return the list of missing dependency slot names; empty when all present.

    Spec Decision 5: all required slots MUST be populated before spawn.
    """
    required_slots = (
        "kb_provider",
        "kb_query_provider",
        "kb_growth_logger_factory",
        "llm_chat_provider",
        "llm_judge_provider",
    )
    missing: list[str] = []
    for slot in required_slots:
        if getattr(state, slot, None) is None:
            missing.append(slot)
    return missing


def _classify_qa_exception(exc: BaseException) -> str:
    """Force unmapped exceptions into ``QA_FAILED`` for /qa.

    Specific OpenAI / KB exceptions still flow through their typed
    branches in ``_classify_exception``; everything else gets the safe
    Q&A-task code so the wire never leaks ``repr(exc)`` shape.
    """
    code = _classify_exception(exc)
    if code == "INTERNAL_ERROR":
        return "QA_FAILED"
    return code


@router.post("/qa", status_code=status.HTTP_202_ACCEPTED)
async def qa_endpoint(
    request: QARequest, http_request: Request
) -> dict[str, str]:
    """Spawn a background Q&A run and return ``{"task_id": ...}``.

    Concurrency: 409 ``TASK_IN_FLIGHT`` when registry holds a running
    task. Errors raised by ``run_qa`` surface through the
    ``_run_background_task`` wrapper as sanitized SSE ``error`` events
    with ``code="QA_FAILED"``.
    """
    workspace_root = _validate_workspace_root(request.workspace_root)
    state_obj = http_request.app.state
    missing = _require_qa_deps(state_obj)
    if missing:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": "QA_NOT_CONFIGURED",
                "message": "Q&A dependencies not initialized on this sidecar",
                "missing": missing,
                # Inline detail string so consumers can grep without
                # parsing structured `missing` list.
                "detail": ", ".join(missing),
            },
        )

    registry: TaskRegistry = state_obj.tasks
    handle = registry.create("qa")
    if handle is None:
        running = registry.current_running()
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_IN_FLIGHT",
                "running_task_id": running.id if running else None,
            },
        )

    # Build per-task collaborators.
    kb_provider = state_obj.kb_query_provider(workspace_root)
    kb_growth_logger = state_obj.kb_growth_logger_factory(workspace_root)
    qa_chat_factory = getattr(state_obj, "llm_qa_provider", None) or state_obj.llm_chat_provider
    qa_provider = qa_chat_factory(workspace_root)

    emitter = TaskHandleEmitter(handle)
    qa_provider.set_emitter(emitter)
    kb_provider.set_emitter(emitter)

    # Construct KB façade tied to the workspace.
    kb_backend = state_obj.kb_backend
    kb_embedding_dim = state_obj.kb_embedding_dim
    if kb_backend is None or kb_embedding_dim is None:
        # Defensive — should be caught by `_require_qa_deps`, but in
        # fragmented test setups the backend might be unset while the
        # provider factories are set. Surface 503 rather than spawning.
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail={
                "code": "QA_NOT_CONFIGURED",
                "message": "kb_backend / kb_embedding_dim not initialized",
                "missing": ["kb_backend"],
                "detail": "kb_backend",
            },
        )
    kb = KnowledgeBase(
        backend=kb_backend,
        provider=kb_provider,
        usage_tracker=state_obj.kb_usage_tracker(workspace_root),
        workspace_root=str(workspace_root),
        embedding_dim=kb_embedding_dim,
    )

    sanitizer_engine = SanitizerEngine()
    audit_dir = workspace_root / _WORKSPACE_AUDIT_SUBDIR
    audit_dir.mkdir(parents=True, exist_ok=True)
    sanitizer_audit = SanitizerAuditLogger(audit_dir / _SANITIZE_AUDIT_FILENAME)

    qa_state = QAState(
        question=request.question,
        originating_station_id=request.originating_station_id,
        session_id=handle.id,
    )

    ctx = ToolContext(
        workspace_root=workspace_root,
        workspace_type="folder",
        session_id=handle.id,
        sanitizer=sanitizer_engine,
        kb=kb,
    )
    # Augment ctx with mutable attributes for the add_to_kb pipeline. We
    # use a duck-typed wrapper because ToolContext is a frozen Pydantic
    # model and cannot accept arbitrary mutable attributes.
    qa_ctx = _QACtxAdapter(
        ctx=ctx,
        kb=kb,
        sanitizer=sanitizer_engine,
        sanitizer_audit=sanitizer_audit,
        kb_growth_logger=kb_growth_logger,
        qa_state=qa_state,
        question=request.question,
        originating_station_id=request.originating_station_id,
        session_id=handle.id,
        workspace_root=workspace_root,
        workspace_type="folder",
        emitter=emitter,
    )

    # FolderTools wraps the explorer-style ToolContext for the five
    # reused read tools. Pass an ExplorerState shim because FolderTools
    # binds to it for `mark_station` (Q&A doesn't use mark_station, but
    # the constructor needs the state).
    explorer_state_shim = ExplorerState(
        task=request.question, budget_steps_left=0, budget_tokens_left=0
    )
    folder_tools = FolderTools(ctx=ctx, state=explorer_state_shim)
    qa_tools = QATools(folder_tools=folder_tools, ctx=qa_ctx)

    reasoning_logger = ReasoningLogger(
        audit_dir / _REASONING_LOG_FILENAME, mode="qa"
    )

    async def _coro_factory() -> dict[str, Any]:
        answer = await run_qa(
            question=request.question,
            state=qa_state,
            kb=kb,
            tools=qa_tools,
            provider=qa_provider,
            logger=reasoning_logger,
            emitter=emitter,
        )
        return answer.model_dump(mode="json")

    asyncio.create_task(
        _run_background_task(
            handle, _coro_factory, classify=_classify_qa_exception
        )
    )
    return {"task_id": handle.id}


class _QACtxAdapter:
    """Mutable façade carrying Q&A-specific dependencies + ToolContext fields.

    `ToolContext` is a frozen Pydantic model, so we can't attach
    `kb_growth_logger` etc. directly. This adapter exposes the same
    attribute surface tools expect (`workspace_root`, `workspace_type`,
    `session_id`, `sanitizer`, `kb`) plus the Q&A-specific extras
    (`sanitizer_audit`, `kb_growth_logger`, `qa_state`, `question`,
    `originating_station_id`).
    """

    def __init__(
        self,
        *,
        ctx: ToolContext,
        kb: Any,
        sanitizer: Any,
        sanitizer_audit: Any,
        kb_growth_logger: Any,
        qa_state: QAState,
        question: str,
        originating_station_id: str | None,
        session_id: str,
        workspace_root: Path,
        workspace_type: str,
        emitter: Any = None,
    ) -> None:
        self._ctx = ctx
        self.kb = kb
        self.sanitizer = sanitizer
        self.sanitizer_audit = sanitizer_audit
        self.kb_growth_logger = kb_growth_logger
        self.qa_state = qa_state
        self.question = question
        self.originating_station_id = originating_station_id
        self.session_id = session_id
        self.workspace_root = workspace_root
        self.workspace_type = workspace_type
        self.emitter = emitter


__all__ = ["QARequest", "qa_endpoint", "router"]
