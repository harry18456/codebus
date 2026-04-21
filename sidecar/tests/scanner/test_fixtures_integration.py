"""Integration tests driving `scan()` against the canned fixtures in
`tests/scanner/fixtures/`.

Backs openspec/changes/scanner-skeleton/tasks.md Task 8.1.

每個 fixture 目錄對應一組高階不變式；這些測試是 leaf-module 測試的補強，
專門驗「把一整個 workspace 丟給 scanner，會不會得到 spec 要求的形狀」。
"""
from __future__ import annotations

import shutil
import sys
from pathlib import Path

import pytest

from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import SanitizerEngine
from codebus_agent.scanner.models import ScanResult
from codebus_agent.scanner.service import scan


# ---------------------------------------------------------------------------
# Shared path helpers
# ---------------------------------------------------------------------------


FIXTURES_ROOT = Path(__file__).parent / "fixtures"


def _copy_fixture(src: Path, dst: Path) -> Path:
    """把 fixture 複製到 tmp_path 下，避免測試意外汙染原始 fixture。"""
    shutil.copytree(src, dst)
    return dst


def _ctx(root: Path) -> ToolContext:
    # scanner-sanitizer-orchestration: ctx.sanitizer is required for text
    # FileEntries (Pass 1 orchestration); inject a fresh built-in engine so
    # these integration fixtures exercise the full pipeline.
    return ToolContext(
        workspace_root=root,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )


def _run_scan(root: Path) -> ScanResult:
    return scan(str(root), _ctx(root))


# ---------------------------------------------------------------------------
# mini-py-repo
# ---------------------------------------------------------------------------


def test_mini_py_repo_ignores_pycache_and_detects_tests(tmp_path: Path) -> None:
    """`__pycache__/` 完全不進 files；`tests/sample_test.py` → has_tests=True。"""
    workspace = _copy_fixture(FIXTURES_ROOT / "mini-py-repo", tmp_path / "ws")
    result = _run_scan(workspace)

    paths = {e.path for e in result.files}
    # 關鍵 source / test / docs 檔都在
    assert "src/main.py" in paths
    assert "src/utils.py" in paths
    assert "tests/sample_test.py" in paths
    assert "README.md" in paths
    # __pycache__ 底下的 .pyc 一定不能出現
    assert not any(p.startswith("__pycache__/") for p in paths), (
        f"__pycache__ 應被 built-in ignore 跳過，但出現在 files: {sorted(paths)}"
    )
    # has_tests 由 tests/sample_test.py 觸發
    assert result.content_summary.has_tests is True
    assert result.content_summary.has_docs is True


# ---------------------------------------------------------------------------
# mini-ts-repo
# ---------------------------------------------------------------------------


def test_mini_ts_repo_node_modules_ignored(tmp_path: Path) -> None:
    """node_modules/ 應同時被 built-in ignore 與 .gitignore 規則雙重排除。"""
    workspace = _copy_fixture(FIXTURES_ROOT / "mini-ts-repo", tmp_path / "ws")
    result = _run_scan(workspace)

    paths = {e.path for e in result.files}
    assert "src/index.ts" in paths
    assert "package.json" in paths
    assert ".gitignore" in paths
    # node_modules 整棵 subtree 不應被探到
    assert not any(p.startswith("node_modules/") for p in paths), (
        f"node_modules 應被 built-in + gitignore 雙重排除，但 files 仍含: "
        f"{sorted(p for p in paths if p.startswith('node_modules/'))}"
    )


# ---------------------------------------------------------------------------
# mixed-encoding
# ---------------------------------------------------------------------------


def test_mixed_encoding_fallback_chain_and_nul_sniff(tmp_path: Path) -> None:
    """UTF-8 / Big5 各自 decode 成功；null-byte binary reclass 為 binary。"""
    workspace = _copy_fixture(FIXTURES_ROOT / "mixed-encoding", tmp_path / "ws")
    result = _run_scan(workspace)

    by_path = {e.path: e for e in result.files}

    # UTF-8 首站命中
    utf8 = by_path["utf8.txt"]
    assert utf8.kind == "text"
    assert utf8.encoding == "utf-8"
    assert utf8.content is not None and "UTF-8" in utf8.content

    # Big5 經 fallback chain 命中
    big5 = by_path["big5.txt"]
    assert big5.kind == "text"
    assert big5.encoding == "big5"
    assert big5.content is not None and "繁體中文" in big5.content

    # Null-byte binary 經 sniff reclass → binary，content/encoding 皆 None
    binary = by_path["binary.dat"]
    assert binary.kind == "binary"
    assert binary.content is None
    assert binary.encoding is None


# ---------------------------------------------------------------------------
# symlink-cases（POSIX only）
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    sys.platform == "win32",
    reason="symlink creation on Windows requires elevated privileges",
)
def test_symlink_cases_inworkspace_and_outofworkspace(tmp_path: Path) -> None:
    """in-workspace symlink 記錄為 resolved_in_workspace=True；
    out-of-workspace 記錄為 False，且絕不觸及 target 檔。"""
    # 複製整個 fixture 樹到 tmp_path，包含 outside_target.txt 與 workspace/
    src = FIXTURES_ROOT / "symlink-cases"
    dst = tmp_path / "symlink-cases"
    shutil.copytree(src, dst)

    workspace = dst / "workspace"
    outside = dst / "outside_target.txt"
    assert workspace.is_dir() and outside.is_file()

    # 執行期建立 symlink：避免 git 在各平台存 symlink 行為不一致
    (workspace / "link.py").symlink_to("real.py")
    (workspace / "escape.lnk").symlink_to(outside)

    result = _run_scan(workspace)

    # files：只應含 real.py，symlink 本身絕不變 FileEntry
    file_paths = {e.path for e in result.files}
    assert file_paths == {"real.py"}

    # symlinks：兩支都要出現
    by_path = {sl.path: sl for sl in result.symlinks}
    assert set(by_path) == {"link.py", "escape.lnk"}
    assert by_path["link.py"].resolved_in_workspace is True
    assert by_path["escape.lnk"].resolved_in_workspace is False
