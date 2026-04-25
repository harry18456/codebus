"""`POST /scan` endpoint — synchronous (default) or async streaming
(`?stream=true`) workspace scan.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Workspace scan endpoint
  Requirement: Workspace type discriminator routing
  Requirement: Synchronous response without SSE progress events
和 `sidecar-runtime` delta Requirement: Workspace scan endpoint registration。
也對應 openspec/changes/sse-progress-skeleton/specs/folder-scanner/spec.md
  Requirement: POST /scan opt-in async streaming mode

關鍵約束（務必不鬆綁）：
  * Bearer + loopback 不鬆綁：router 只掛在 bearer middleware 之下，
    ``create_app`` 在 install bearer 之後才 include_router。
  * Discriminator day-1（D-002）：``workspace_type: "folder" | "topic"``
    是 Pydantic Literal；``topic`` 由 handler 回 501，``folder`` 走 pipeline。
  * SCANNER_WORKSPACE_INVALID：``workspace_root`` 不存在 / 非目錄 → 400。
  * 同步模式（無 query）只吐單一 JSON body，不 stream 任何 progress 事件。
  * ``?stream=true`` opt-in：建 task → 啟 background coroutine → 立即回
    ``{task_id}``；既有同步路徑程式碼路徑完全不動。
"""
from __future__ import annotations

import asyncio
import uuid
from pathlib import Path
from typing import Any, Literal

from fastapi import APIRouter, HTTPException, Request, status
from pydantic import BaseModel

from codebus_agent.api.tasks import (
    ERROR_CODES,
    TaskHandle,
    TaskRegistry,
    _run_background_task,
)
from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine
from codebus_agent.scanner.models import ScannerProgressEvent, ScanResult
from codebus_agent.scanner.service import scan

# Workspace-level sanitize_audit.jsonl — lives alongside other workspace audit
# artefacts under `<workspace_root>/.codebus/`.  Per D-025 this is a provisional
# location for M1; a future change will relocate workspace audit to
# `~/.codebus/workspaces/{id}/` once workspace_id plumbing lands.  Schema and
# JSONL contents do not depend on the enclosing directory.
_WORKSPACE_AUDIT_SUBDIR = ".codebus"
_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"

# Rules version recorded on every sanitize_audit line. Single source of truth
# is `codebus_agent.sanitizer.RULES_VERSION`; bumping that constant propagates
# here automatically (CLAUDE.md invariant #9 / `docs/sanitizer.md §六` /
# `docs/authorization.md §六`).
from codebus_agent.sanitizer import RULES_VERSION as _RULES_VERSION


router = APIRouter()


class ScanRequest(BaseModel):
    """POST /scan 請求 body schema。

    ``workspace_type`` 是 Literal —— 任何不在 {folder, topic} 的值會讓 Pydantic
    在請求進到 handler 前就丟 422，對齊 spec Scenario「Unknown discriminator
    rejected」。
    """

    workspace_type: Literal["folder", "topic"]
    workspace_root: str


def _scanner_event_to_wire(event: ScannerProgressEvent) -> dict[str, Any]:
    """Translate a ``ScannerProgressEvent`` into a wire ``progress`` event.

    Per `sse-progress-skeleton` spec "POST /scan opt-in async streaming mode":
    both internal phases (``walking`` / ``sanitizing``) collapse to the
    single wire phase ``"scanning"`` (Module 1 phase-name mapping —
    consumers don't need to know the scanner's internal pipeline split).
    """
    return {
        "type": "progress",
        "phase": "scanning",
        "current": event.current,
        "total": event.total,
        "current_file": event.current_file,
    }


def _build_scan_inputs(
    request: ScanRequest,
) -> tuple[Path, ToolContext, SanitizerAuditLogger, str]:
    """Validate the request and return the inputs required to call ``scan``.

    Pulled out of the endpoint body so the sync and ``?stream=true`` paths
    share the exact same setup — keeps the spec's "既有同步契約保留"
    invariant honest (no divergence between the two branches).
    """
    if request.workspace_type == "topic":
        # spec Scenario「Topic workspace returns 501」
        raise HTTPException(
            status_code=status.HTTP_501_NOT_IMPLEMENTED,
            detail="workspace_type='topic' not implemented in MVP",
        )

    workspace_root = Path(request.workspace_root)
    if not workspace_root.exists() or not workspace_root.is_dir():
        # spec Scenario「Nonexistent workspace root rejected」
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "SCANNER_WORKSPACE_INVALID",
                "message": f"workspace_root {request.workspace_root!r} does not exist "
                f"or is not a directory",
            },
        )

    # scanner-sanitizer-orchestration: a fresh engine per request is safe —
    # SanitizerEngine keeps no cross-call state (placeholder indices reset
    # per `sanitize` call, per `docs/decisions.md` D-015).
    ctx = ToolContext(
        workspace_root=workspace_root,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )
    audit_path = workspace_root / _WORKSPACE_AUDIT_SUBDIR / _SANITIZE_AUDIT_FILENAME
    audit_logger = SanitizerAuditLogger(audit_path)
    session_id = str(uuid.uuid4())
    return workspace_root, ctx, audit_logger, session_id


@router.post("/scan")
async def scan_endpoint(
    request: ScanRequest,
    http_request: Request,
    stream: bool = False,
) -> Any:
    """執行 workspace scan。

    Modes:
      * 預設（無 ``?stream=true``）：同步執行，回完整 ``ScanResult`` JSON，
        對應 `Synchronous response without SSE progress events`。
      * ``?stream=true``：建 task handle、spawn background coroutine 跑
        ``scan(..., on_progress=…)``，立即回 ``{"task_id": "scan_<hex8>"}``；
        訂閱者透過 ``GET /tasks/{id}/events`` 收 progress / done / error
        （`POST /scan opt-in async streaming mode`）。

    備註：bearer 驗證由 ``BearerAuthMiddleware`` 在 ASGI 層級處理；到達 handler
    的請求都已過 auth，無需在此重複檢查。
    """
    workspace_root, ctx, audit_logger, session_id = _build_scan_inputs(request)

    if not stream:
        # Sync path — unchanged behaviour. Return the full ScanResult.
        return await scan(
            request.workspace_root,
            ctx,
            sanitize_audit=audit_logger,
            rules_version=_RULES_VERSION,
            session_id=session_id,
        )

    # Stream path — opt-in async mode.
    registry: TaskRegistry = http_request.app.state.tasks
    handle = registry.create("scan")
    if handle is None:
        running = registry.current_running()
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={
                "code": "TASK_IN_FLIGHT",
                "running_task_id": running.id if running else None,
            },
        )

    async def _on_progress(event: ScannerProgressEvent) -> None:
        handle.emit(_scanner_event_to_wire(event))

    async def _coro_factory() -> dict[str, Any]:
        result = await scan(
            request.workspace_root,
            ctx,
            sanitize_audit=audit_logger,
            rules_version=_RULES_VERSION,
            session_id=session_id,
            on_progress=_on_progress,
        )
        # Hand the full ScanResult JSON to the task wrapper as the
        # terminal payload returned by `/tasks/{id}/result`.
        return result.model_dump(mode="json")

    asyncio.create_task(_run_background_task(handle, _coro_factory))
    return {"task_id": handle.id}


__all__ = [
    "router",
    "ScanRequest",
    "scan_endpoint",
    "_scanner_event_to_wire",
]
