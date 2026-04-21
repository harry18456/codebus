"""TDD red tests for `POST /scan` — Task 7.1.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Workspace scan endpoint
  Requirement: Workspace type discriminator routing
  Requirement: Synchronous response without SSE progress events
and the `sidecar-runtime` delta Requirement: Workspace scan endpoint registration.

測試負責鎖住以下契約：
  * 200 folder：正常工作區同步回 JSON ScanResult
  * 501 topic：workspace_type='topic' 在 skeleton 階段回 501 Not Implemented
  * 422 unknown：workspace_type 不在 {folder, topic} → Pydantic 422
  * 401 missing bearer：無 Authorization 標頭直接 401，不執行 traversal
  * 400 SCANNER_WORKSPACE_INVALID：workspace_root 路徑不存在
  * Content-Type: application/json 單 body（不得 text/event-stream）
"""
from __future__ import annotations

import secrets
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def bearer() -> str:
    """每個 test 獨立 token —— 與 `tests/test_healthz.py` 的慣例一致。"""
    return secrets.token_urlsafe(32)


@pytest.fixture
def client(bearer: str) -> TestClient:
    app = create_app(bearer_token=bearer)
    return TestClient(app)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


# ---------------------------------------------------------------------------
# 1. 200 folder — happy path
# ---------------------------------------------------------------------------


def test_scan_folder_returns_200_with_scan_result(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """合法 folder workspace → HTTP 200，body 為可 parse 的 ScanResult JSON。"""
    (tmp_path / "a.py").write_bytes(b"x = 1\n")

    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
    )

    assert resp.status_code == 200
    body = resp.json()
    assert body["workspace_root"] == str(tmp_path.resolve(strict=False))
    assert "files" in body and "symlinks" in body
    assert "content_summary" in body
    assert "stats" in body
    # Deferred stubs 一併驗證（spec Requirement "Deferred subsystem schema preservation"）
    assert body["git"] is None
    assert body["is_monorepo"] is False
    assert body["monorepo_type"] is None
    assert body["sub_packages"] == []


def test_scan_folder_content_type_is_application_json(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """同步 JSON 單 body —— Content-Type 不得是 SSE。"""
    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
    )

    assert resp.status_code == 200
    assert resp.headers["content-type"].startswith("application/json")
    # SSE header 不得出現
    assert "text/event-stream" not in resp.headers.get("content-type", "")


def test_scan_folder_timestamps_present_and_ordered(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """scan_started_at / scan_completed_at 都 ISO-8601 且 started <= completed。"""
    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
    )

    assert resp.status_code == 200
    body = resp.json()
    started = body["scan_started_at"]
    completed = body["scan_completed_at"]
    assert isinstance(started, str) and isinstance(completed, str)
    # ISO-8601 字串可直接字串序比（同 UTC、同格式）
    assert started <= completed


# ---------------------------------------------------------------------------
# 2. 501 topic
# ---------------------------------------------------------------------------


def test_scan_topic_returns_501_not_implemented(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """workspace_type='topic' → 501，detail 指出為 MVP 未實作分支。"""
    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "topic", "workspace_root": str(tmp_path)},
    )

    assert resp.status_code == 501
    body = resp.json()
    # detail 包含 topic 字樣即可；spec Scenario 範例是
    # "workspace_type='topic' not implemented in MVP"
    assert "topic" in body.get("detail", "").lower()


# ---------------------------------------------------------------------------
# 3. 422 unknown discriminator
# ---------------------------------------------------------------------------


def test_scan_unknown_workspace_type_returns_422(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """workspace_type 不在 {folder, topic} → Pydantic 422，不執行 traversal。"""
    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "file", "workspace_root": str(tmp_path)},
    )

    assert resp.status_code == 422


def test_scan_missing_workspace_type_returns_422(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """沒帶 workspace_type 欄位一樣 422。"""
    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_root": str(tmp_path)},
    )
    assert resp.status_code == 422


def test_scan_missing_workspace_root_returns_422(
    client: TestClient, bearer: str
) -> None:
    """沒帶 workspace_root 欄位一樣 422。"""
    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder"},
    )
    assert resp.status_code == 422


# ---------------------------------------------------------------------------
# 4. 401 missing bearer
# ---------------------------------------------------------------------------


def test_scan_without_bearer_returns_401(
    client: TestClient, tmp_path: Path
) -> None:
    """沒帶 Authorization 標頭 → 401，且 body 絕不含 ScanResult。"""
    resp = client.post(
        "/scan",
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
    )
    assert resp.status_code == 401
    # 401 body 應是 {"detail": "unauthorized"}，絕不包含 files / workspace_root
    body = resp.json()
    assert "files" not in body
    assert "workspace_root" not in body


def test_scan_with_wrong_bearer_returns_401(
    client: TestClient, tmp_path: Path
) -> None:
    """Authorization 含非預期 token → 401，同樣不執行 traversal。"""
    resp = client.post(
        "/scan",
        headers={"Authorization": "Bearer wrong-token"},
        json={"workspace_type": "folder", "workspace_root": str(tmp_path)},
    )
    assert resp.status_code == 401


# ---------------------------------------------------------------------------
# 5. 400 SCANNER_WORKSPACE_INVALID
# ---------------------------------------------------------------------------


def test_scan_nonexistent_workspace_root_returns_400(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """workspace_root 路徑不存在 → 400，error code = SCANNER_WORKSPACE_INVALID。"""
    nonexistent = tmp_path / "does-not-exist"
    assert not nonexistent.exists()

    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder", "workspace_root": str(nonexistent)},
    )

    assert resp.status_code == 400
    body = resp.json()
    # error code 放在 detail 裡（FastAPI 慣例）；允許 detail 是 dict 或 str
    detail = body.get("detail")
    if isinstance(detail, dict):
        assert detail.get("code") == "SCANNER_WORKSPACE_INVALID"
    else:
        # str detail 至少要包含 code 名稱
        assert "SCANNER_WORKSPACE_INVALID" in str(detail)


def test_scan_workspace_root_that_is_a_file_returns_400(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """workspace_root 指向檔案（不是目錄）也應被判為 invalid。"""
    f = tmp_path / "single-file"
    f.write_bytes(b"not a workspace")

    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder", "workspace_root": str(f)},
    )

    assert resp.status_code == 400
    body = resp.json()
    detail = body.get("detail")
    code_found = (
        (isinstance(detail, dict) and detail.get("code") == "SCANNER_WORKSPACE_INVALID")
        or (not isinstance(detail, dict) and "SCANNER_WORKSPACE_INVALID" in str(detail))
    )
    assert code_found
