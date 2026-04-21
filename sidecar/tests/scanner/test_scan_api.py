"""TDD red tests for `POST /scan` — Task 7.1 + scanner-sanitizer-orchestration Task 4.2.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Workspace scan endpoint
  Requirement: Workspace type discriminator routing
  Requirement: Synchronous response without SSE progress events
and the `sidecar-runtime` delta Requirement: Workspace scan endpoint registration.

Also backs openspec/changes/scanner-sanitizer-orchestration/specs/folder-scanner/spec.md
  Requirement: Pass 1 sanitizer orchestration for text FileEntries
  Requirement: Sanitize audit logging during scan

測試負責鎖住以下契約：
  * 200 folder：正常工作區同步回 JSON ScanResult
  * 501 topic：workspace_type='topic' 在 skeleton 階段回 501 Not Implemented
  * 422 unknown：workspace_type 不在 {folder, topic} → Pydantic 422
  * 401 missing bearer：無 Authorization 標頭直接 401，不執行 traversal
  * 400 SCANNER_WORKSPACE_INVALID：workspace_root 路徑不存在
  * Content-Type: application/json 單 body（不得 text/event-stream）
  * Pass 1 sanitizer 整合：with-secrets fixture 會讓 FileEntry.content 帶
    `<REDACTED:...>` placeholder、sanitize_stats 非空、sanitize_audit.jsonl
    實際落盤且 source.pass == "scanner"。
"""
from __future__ import annotations

import json
import secrets
import shutil
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app

_FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "with-secrets"


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


# ---------------------------------------------------------------------------
# 6. Pass 1 sanitizer integration（scanner-sanitizer-orchestration Task 4.2）
# ---------------------------------------------------------------------------


def test_scan_with_secrets_fixture_sanitizes_content_and_writes_audit(
    client: TestClient, bearer: str, tmp_path: Path
) -> None:
    """with-secrets fixture → FileEntry.content 含 `<REDACTED:...>` placeholder,
    sanitize_stats 至少一個非零 kind, sanitize_audit.jsonl 實際落盤且含
    source.pass == "scanner"。

    對應 spec Scenario: Text file containing an email is scrubbed in content
    and counted in stats + Scenario: Sanitize audit line written for each hit.
    """
    # Copy fixture into a tmp workspace so the endpoint's audit writes land in
    # an isolated location (not under the repo source tree).
    ws = tmp_path / "with-secrets"
    shutil.copytree(_FIXTURE_ROOT, ws)

    resp = client.post(
        "/scan",
        headers=_auth(bearer),
        json={"workspace_type": "folder", "workspace_root": str(ws)},
    )
    assert resp.status_code == 200

    body = resp.json()
    files_by_path = {e["path"]: e for e in body["files"]}

    # contacts.txt → two emails redacted; raw emails gone from content.
    contacts = files_by_path.get("contacts.txt")
    assert contacts is not None, f"contacts.txt missing from scan result: {list(files_by_path)!r}"
    assert contacts["kind"] == "text"
    assert "<REDACTED:email#1>" in contacts["content"]
    assert "alice@example.com" not in contacts["content"]
    assert "bob@example.com" not in contacts["content"]
    assert contacts["sanitize_stats"].get("email", 0) >= 1

    # config.py → detect-secrets rule path (AWS dummy creds).
    config_py = files_by_path.get("config.py")
    assert config_py is not None
    assert config_py["kind"] == "text"
    assert "AKIAIOSFODNN7EXAMPLE" not in config_py["content"]
    assert "<REDACTED:" in config_py["content"]
    assert config_py["sanitize_stats"].get("secret", 0) >= 1

    # README.md → clean; sanitize_stats stays {}.
    readme = files_by_path.get("README.md")
    assert readme is not None
    assert readme["sanitize_stats"] == {}

    # quarantined_count is 0 (no engine crash expected on this fixture).
    assert body["stats"]["quarantined_count"] == 0

    # sanitize_audit.jsonl landed under <ws>/.codebus/ and includes scanner-
    # scoped lines whose source is the structured {"pass": ..., "path": ...}.
    audit_path = ws / ".codebus" / "sanitize_audit.jsonl"
    assert audit_path.exists(), f"expected {audit_path} to be written by /scan"

    lines = [
        json.loads(line)
        for line in audit_path.read_text(encoding="utf-8").splitlines()
        if line
    ]
    assert len(lines) >= 3, (
        f"expected at least 3 audit lines (2 emails + >=1 secret), got {len(lines)}: {lines!r}"
    )
    scanner_lines = [
        ln for ln in lines
        if isinstance(ln.get("source"), dict) and ln["source"].get("pass") == "scanner"
    ]
    assert len(scanner_lines) == len(lines), (
        "all Pass 1 audit lines MUST carry source.pass == 'scanner'; "
        f"got {lines!r}"
    )
    # Every scanner line's source.path MUST be one of the fixture's relative
    # paths (posix slash form regardless of host OS).
    audited_paths = {ln["source"]["path"] for ln in scanner_lines}
    assert audited_paths <= {"contacts.txt", "config.py"}, (
        f"unexpected paths in audit log: {audited_paths!r}"
    )
