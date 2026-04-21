"""`POST /scan` endpoint — synchronous workspace scan.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Workspace scan endpoint
  Requirement: Workspace type discriminator routing
  Requirement: Synchronous response without SSE progress events
和 `sidecar-runtime` delta Requirement: Workspace scan endpoint registration。

關鍵約束（務必不鬆綁）：
  * Bearer + loopback 不鬆綁：router 只掛在 bearer middleware 之下，
    ``create_app`` 在 install bearer 之後才 include_router。
  * Discriminator day-1（D-002）：``workspace_type: "folder" | "topic"``
    是 Pydantic Literal；``topic`` 由 handler 回 501，``folder`` 走 pipeline。
  * SCANNER_WORKSPACE_INVALID：``workspace_root`` 不存在 / 非目錄 → 400。
  * SSE 禁用：這個 endpoint 只吐單一 JSON body，不 stream 任何 progress 事件。
"""
from __future__ import annotations

import uuid
from pathlib import Path
from typing import Literal

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel

from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine
from codebus_agent.scanner.models import ScanResult
from codebus_agent.scanner.service import scan

# Workspace-level sanitize_audit.jsonl — lives alongside other workspace audit
# artefacts under `<workspace_root>/.codebus/`.  Per D-025 this is a provisional
# location for M1; a future change will relocate workspace audit to
# `~/.codebus/workspaces/{id}/` once workspace_id plumbing lands.  Schema and
# JSONL contents do not depend on the enclosing directory.
_WORKSPACE_AUDIT_SUBDIR = ".codebus"
_SANITIZE_AUDIT_FILENAME = "sanitize_audit.jsonl"

# Rules version recorded on every sanitize_audit line; kept in sync with the
# built-in rule table bundled in `codebus_agent.sanitizer.rules`.  Bumping this
# string is mandatory whenever that rule table changes (see `docs/sanitizer.md
# §六` / `docs/authorization.md §六`).
_RULES_VERSION = "2026-04-20-1"


router = APIRouter()


class ScanRequest(BaseModel):
    """POST /scan 請求 body schema。

    ``workspace_type`` 是 Literal —— 任何不在 {folder, topic} 的值會讓 Pydantic
    在請求進到 handler 前就丟 422，對齊 spec Scenario「Unknown discriminator
    rejected」。
    """

    workspace_type: Literal["folder", "topic"]
    workspace_root: str


@router.post("/scan", response_model=ScanResult)
def scan_endpoint(request: ScanRequest) -> ScanResult:
    """同步執行 workspace scan，回傳 ``ScanResult``。

    流程：
      1. ``workspace_type == "topic"`` → 501 Not Implemented（MVP 未支援）
      2. ``workspace_root`` 不存在或非目錄 → 400 ``SCANNER_WORKSPACE_INVALID``
      3. 其他 → 建 ``ToolContext`` 並呼叫 ``service.scan`` 回傳結果

    備註：bearer 驗證由 ``BearerAuthMiddleware`` 在 ASGI 層級處理；到達 handler
    的請求都已過 auth，無需在此重複檢查。
    """
    if request.workspace_type == "topic":
        # spec Scenario「Topic workspace returns 501」—— detail 需明確指出未實作
        raise HTTPException(
            status_code=status.HTTP_501_NOT_IMPLEMENTED,
            detail="workspace_type='topic' not implemented in MVP",
        )

    # folder branch — 驗 workspace_root 存在且是目錄
    workspace_root = Path(request.workspace_root)
    if not workspace_root.exists() or not workspace_root.is_dir():
        # spec Scenario「Nonexistent workspace root rejected」——
        # detail 以 dict 傳 code + message，方便前端機器判讀
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
    # per `sanitize` call, per `docs/decisions.md` D-015).  Future M2+
    # wiring may relocate construction to app.state if rule config turns
    # out to be expensive to load; schema doesn't change either way.
    ctx = ToolContext(
        workspace_root=workspace_root,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )

    # SanitizerAuditLogger creates the `.codebus/` subdir on first write;
    # a fresh uuid4 tags this scan's audit lines so forensic readers can
    # trace a full /scan invocation across the seven-layer audit tree.
    audit_path = workspace_root / _WORKSPACE_AUDIT_SUBDIR / _SANITIZE_AUDIT_FILENAME
    audit_logger = SanitizerAuditLogger(audit_path)
    session_id = str(uuid.uuid4())

    return scan(
        request.workspace_root,
        ctx,
        sanitize_audit=audit_logger,
        rules_version=_RULES_VERSION,
        session_id=session_id,
    )


__all__ = ["router", "ScanRequest", "scan_endpoint"]
