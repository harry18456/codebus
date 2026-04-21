"""TDD red tests for `scanner/walk.py` — Task 5.1.

Backs spec Requirements：
  * File tree traversal with gitignore stacking
  * Symlink handling without following
  * Sandbox boundary enforcement

設計決策（由測試驅動、鎖定 walk.py 5.2 實作契約）：

1. ``walk(workspace_root, ctx) -> Iterator[FileEntry | Symlink]`` 嚴格照
   ``openspec/changes/scanner-skeleton/tasks.md`` 5.2 的簽章；``warnings``
   以 keyword-only 參數（``*, warnings: list[str] | None = None``）回傳
   sandbox / 檔案系統層級的人類可讀警告。tasks.md 5.2 文字說「skip 並寫
   warning」但未指定如何傳出，這是 test-first 最小侵入設計。
2. ``FileEntry.path`` / ``Symlink.path`` 均以 ``workspace_root`` 相對、POSIX
   斜線表示（``sub/file.py``，永不含 ``\\``）。Windows ``pathlib`` 原生會給
   ``\\``，walk 實作 MUST 統一轉 ``as_posix``。
3. walk 不跟隨 symlink（``Path.is_symlink()`` 判別），symlink 僅作為 ``Symlink``
   記錄 yield，不 produce ``FileEntry``；``resolved_in_workspace`` 依 ``resolve``
   後是否在 ``workspace_root`` 內決定。
4. built-in always-ignore prefixes 與 ``.gitignore`` 同階疊加（``pathspec``
   ``gitwildmatch`` 語法），目錄命中 ignore 即不 rglob 進去。
5. sandbox 違規（``ensure_in_workspace`` raise ``PathEscapeError``）→ 該 entry
   不 yield、warning 被 append。
"""
from __future__ import annotations

import shutil
import sys
from pathlib import Path

import pytest

from codebus_agent.sandbox import ToolContext
from codebus_agent.scanner.models import FileEntry, Symlink
from codebus_agent.scanner.walk import walk


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------


def _ctx(root: Path) -> ToolContext:
    """所有測試都只用 folder workspace（topic 在 skeleton 階段回 501）。"""
    return ToolContext(workspace_root=root, workspace_type="folder")


def _file_paths(entries: list) -> list[str]:
    """從 walk 結果抽出 FileEntry 的 path，已排序利於 assert。"""
    return sorted(e.path for e in entries if isinstance(e, FileEntry))


def _symlink_entries(entries: list) -> list[Symlink]:
    """從 walk 結果抽出 Symlink，保留原順序以便檢查單筆欄位。"""
    return [e for e in entries if isinstance(e, Symlink)]


_POSIX_ONLY = pytest.mark.skipif(
    sys.platform == "win32",
    reason="symlink creation on Windows requires elevated privileges",
)


# ---------------------------------------------------------------------------
# 1. Basic traversal
# ---------------------------------------------------------------------------


def test_walk_empty_workspace_yields_nothing(tmp_path: Path) -> None:
    """空資料夾不應產生任何 FileEntry / Symlink。"""
    assert list(walk(tmp_path, _ctx(tmp_path))) == []


def test_walk_single_file_yielded_as_file_entry(tmp_path: Path) -> None:
    """單一普通檔案 → 單一 FileEntry，path 用相對 POSIX 形式。"""
    (tmp_path / "main.py").write_text("print('ok')\n", encoding="utf-8")
    entries = list(walk(tmp_path, _ctx(tmp_path)))
    assert len(entries) == 1
    assert isinstance(entries[0], FileEntry)
    assert entries[0].path == "main.py"


def test_walk_nested_directories_traversed(tmp_path: Path) -> None:
    """多層巢狀目錄全部都要被 rglob 到；path 一律 POSIX 斜線。"""
    (tmp_path / "a.py").write_text("a", encoding="utf-8")
    (tmp_path / "sub").mkdir()
    (tmp_path / "sub" / "b.py").write_text("b", encoding="utf-8")
    (tmp_path / "sub" / "deep").mkdir()
    (tmp_path / "sub" / "deep" / "c.py").write_text("c", encoding="utf-8")

    entries = list(walk(tmp_path, _ctx(tmp_path)))
    assert set(_file_paths(entries)) == {"a.py", "sub/b.py", "sub/deep/c.py"}


def test_walk_file_entry_paths_use_forward_slashes(tmp_path: Path) -> None:
    """FileEntry.path 不得含 ``\\``（Windows 保險條款）。"""
    (tmp_path / "dir").mkdir()
    (tmp_path / "dir" / "file.py").write_text("x", encoding="utf-8")
    paths = _file_paths(list(walk(tmp_path, _ctx(tmp_path))))
    assert "dir/file.py" in paths
    assert not any("\\" in p for p in paths)


# ---------------------------------------------------------------------------
# 2. Built-in always-ignore
# ---------------------------------------------------------------------------

# spec 列的 built-in always-ignore 目錄前綴（完整清單見 spec 第 45 行）
_BUILTIN_IGNORE_DIRS = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    "dist",
    "build",
    "target",
    "out",
]


@pytest.mark.parametrize("dirname", _BUILTIN_IGNORE_DIRS)
def test_walk_builtin_directory_ignored(tmp_path: Path, dirname: str) -> None:
    """每個 built-in 目錄單測：內部檔案完全不得 yield。"""
    (tmp_path / "keep.py").write_text("keep", encoding="utf-8")
    ignored = tmp_path / dirname
    ignored.mkdir()
    (ignored / "secret.py").write_text("secret", encoding="utf-8")

    file_paths = _file_paths(list(walk(tmp_path, _ctx(tmp_path))))
    assert "keep.py" in file_paths
    assert not any(p.startswith(f"{dirname}/") for p in file_paths)


@pytest.mark.parametrize("filename", [".DS_Store", "Thumbs.db"])
def test_walk_builtin_file_ignored(tmp_path: Path, filename: str) -> None:
    """root 層的 OS junk 檔（.DS_Store / Thumbs.db）必須被 skip。"""
    (tmp_path / "keep.py").write_text("keep", encoding="utf-8")
    (tmp_path / filename).write_text("junk", encoding="utf-8")

    file_paths = _file_paths(list(walk(tmp_path, _ctx(tmp_path))))
    assert "keep.py" in file_paths
    assert filename not in file_paths


def test_walk_builtin_ignored_dir_subtree_not_descended(tmp_path: Path) -> None:
    """命中 ignore 的目錄整個 subtree 不得被探索（性能不變式 + 不讀內容）。"""
    nm = tmp_path / "node_modules"
    nm.mkdir()
    for i in range(5):
        pkg = nm / f"pkg{i}"
        pkg.mkdir()
        (pkg / "index.js").write_text("m", encoding="utf-8")
        (pkg / "package.json").write_text("{}", encoding="utf-8")

    (tmp_path / "app.py").write_text("a", encoding="utf-8")

    file_paths = _file_paths(list(walk(tmp_path, _ctx(tmp_path))))
    assert file_paths == ["app.py"]


# ---------------------------------------------------------------------------
# 3. .gitignore stacking
# ---------------------------------------------------------------------------


def test_walk_root_gitignore_excludes_files(tmp_path: Path) -> None:
    """root ``.gitignore`` 規則應作用於整棵樹。"""
    (tmp_path / ".gitignore").write_text("*.log\n", encoding="utf-8")
    (tmp_path / "keep.py").write_text("k", encoding="utf-8")
    (tmp_path / "drop.log").write_text("d", encoding="utf-8")

    file_paths = set(_file_paths(list(walk(tmp_path, _ctx(tmp_path)))))
    assert "keep.py" in file_paths
    assert "drop.log" not in file_paths


def test_walk_gitignore_file_itself_is_emitted(tmp_path: Path) -> None:
    """``.gitignore`` 檔自己不在任何 ignore 規則內，必須被納入 FileEntry。"""
    (tmp_path / ".gitignore").write_text("*.log\n", encoding="utf-8")
    (tmp_path / "x.py").write_text("x", encoding="utf-8")

    file_paths = set(_file_paths(list(walk(tmp_path, _ctx(tmp_path)))))
    assert ".gitignore" in file_paths
    assert "x.py" in file_paths


def test_walk_nested_gitignore_stacks_with_parent(tmp_path: Path) -> None:
    """子目錄 ``.gitignore`` 與 parent 規則 union，兩套都生效。"""
    (tmp_path / ".gitignore").write_text("*.log\n", encoding="utf-8")
    (tmp_path / "app.log").write_text("root log", encoding="utf-8")
    (tmp_path / "a.py").write_text("a", encoding="utf-8")

    sub = tmp_path / "sub"
    sub.mkdir()
    (sub / ".gitignore").write_text("local.tmp\n", encoding="utf-8")
    (sub / "sub.log").write_text("hidden by root rule", encoding="utf-8")
    (sub / "local.tmp").write_text("hidden by local rule", encoding="utf-8")
    (sub / "ok.py").write_text("ok", encoding="utf-8")

    file_paths = set(_file_paths(list(walk(tmp_path, _ctx(tmp_path)))))
    assert "a.py" in file_paths
    assert "sub/ok.py" in file_paths
    # parent rule cascades down
    assert "app.log" not in file_paths
    assert "sub/sub.log" not in file_paths
    # local rule works
    assert "sub/local.tmp" not in file_paths


def test_walk_gitignore_negation_respected(tmp_path: Path) -> None:
    """``!important.log`` 應覆蓋同層 ``*.log`` 的 exclude。"""
    (tmp_path / ".gitignore").write_text("*.log\n!important.log\n", encoding="utf-8")
    (tmp_path / "noise.log").write_text("n", encoding="utf-8")
    (tmp_path / "important.log").write_text("i", encoding="utf-8")
    (tmp_path / "a.py").write_text("a", encoding="utf-8")

    file_paths = set(_file_paths(list(walk(tmp_path, _ctx(tmp_path)))))
    assert "important.log" in file_paths
    assert "noise.log" not in file_paths
    assert "a.py" in file_paths


def test_walk_gitignore_directory_pattern_skips_subtree(tmp_path: Path) -> None:
    """``build/`` pattern 應讓整個 build 子樹不被 rglob 進去。"""
    (tmp_path / ".gitignore").write_text("build/\n", encoding="utf-8")
    build = tmp_path / "build"
    build.mkdir()
    (build / "artifact.o").write_text("a", encoding="utf-8")
    (build / "nested").mkdir()
    (build / "nested" / "deeper.o").write_text("d", encoding="utf-8")
    (tmp_path / "src.py").write_text("s", encoding="utf-8")

    file_paths = set(_file_paths(list(walk(tmp_path, _ctx(tmp_path)))))
    assert "src.py" in file_paths
    assert not any(p.startswith("build/") for p in file_paths)


# ---------------------------------------------------------------------------
# 4. Symlink handling (POSIX only — Windows 需 elevated privilege)
# ---------------------------------------------------------------------------


@_POSIX_ONLY
def test_walk_symlink_in_workspace_recorded_not_followed(tmp_path: Path) -> None:
    """指向同 workspace 內檔案的 symlink → Symlink(resolved_in_workspace=True)；
    不得為 link 本身 yield FileEntry。"""
    (tmp_path / "real.py").write_text("real", encoding="utf-8")
    (tmp_path / "link.py").symlink_to("real.py")

    entries = list(walk(tmp_path, _ctx(tmp_path)))
    files = [e for e in entries if isinstance(e, FileEntry)]
    links = _symlink_entries(entries)

    assert [f.path for f in files] == ["real.py"]
    assert len(links) == 1
    sl = links[0]
    assert sl.path == "link.py"
    assert sl.target == "real.py"
    assert sl.resolved_in_workspace is True


@_POSIX_ONLY
def test_walk_symlink_out_of_workspace_marked_outside(tmp_path: Path) -> None:
    """指向 workspace 外的 symlink → Symlink(resolved_in_workspace=False)；
    target 絕不被讀，連 FileEntry 都不產。"""
    outside = tmp_path.parent / f"{tmp_path.name}_outside_target"
    outside.mkdir()
    try:
        secret = outside / "secret.txt"
        secret.write_text("SECRET", encoding="utf-8")
        (tmp_path / "escape").symlink_to(secret)

        entries = list(walk(tmp_path, _ctx(tmp_path)))
        files = [e for e in entries if isinstance(e, FileEntry)]
        links = _symlink_entries(entries)

        assert files == []
        assert len(links) == 1
        sl = links[0]
        assert sl.path == "escape"
        assert sl.resolved_in_workspace is False
    finally:
        shutil.rmtree(outside, ignore_errors=True)


@_POSIX_ONLY
def test_walk_symlink_to_directory_not_descended(tmp_path: Path) -> None:
    """指向目錄的 symlink 不得被 rglob 進去 —— 避免 target 被重複列舉 / 無窮迴圈。"""
    (tmp_path / "realdir").mkdir()
    (tmp_path / "realdir" / "r.py").write_text("r", encoding="utf-8")
    (tmp_path / "linkdir").symlink_to("realdir", target_is_directory=True)

    entries = list(walk(tmp_path, _ctx(tmp_path)))
    file_paths = _file_paths(entries)
    assert "realdir/r.py" in file_paths
    assert not any(p.startswith("linkdir/") for p in file_paths)


# ---------------------------------------------------------------------------
# 5. Sandbox boundary enforcement
# ---------------------------------------------------------------------------


def test_walk_ensure_in_workspace_failure_skipped_and_warned(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """當 ensure_in_workspace raise PathEscapeError，該 entry 應被 skip 並寫 warning。

    以 monkeypatch 注入 fake ensure_in_workspace：這是 integration-level 契約測試
    ——真的 PathEscapeError 具體觸發路徑（symlink escape / Windows UNC 等）已在
    ``tests/sandbox/`` 有紅隊 fixture，此處只驗 walk 對 sandbox raise 的反應。
    """
    (tmp_path / "keep.py").write_text("k", encoding="utf-8")
    (tmp_path / "bad.py").write_text("b", encoding="utf-8")

    from codebus_agent.sandbox import PathEscapeError
    from codebus_agent.scanner import walk as walk_mod

    original = walk_mod.ensure_in_workspace

    def fake_ensure(p, ctx):
        if "bad" in str(p):
            raise PathEscapeError(f"synthetic escape: {p}")
        return original(p, ctx)

    monkeypatch.setattr(walk_mod, "ensure_in_workspace", fake_ensure)

    warnings: list[str] = []
    entries = list(walk(tmp_path, _ctx(tmp_path), warnings=warnings))
    file_paths = set(_file_paths(entries))

    assert "keep.py" in file_paths
    assert "bad.py" not in file_paths
    assert any("bad" in w for w in warnings), (
        f"expected a warning mentioning 'bad', got: {warnings!r}"
    )


def test_walk_warnings_kwarg_optional(tmp_path: Path) -> None:
    """不傳 ``warnings`` kwarg 呼叫 walk 必須正常運作（保持默認 None）。"""
    (tmp_path / "a.py").write_text("a", encoding="utf-8")
    entries = list(walk(tmp_path, _ctx(tmp_path)))
    assert _file_paths(entries) == ["a.py"]


def test_walk_no_warnings_on_clean_workspace(tmp_path: Path) -> None:
    """完全正常的 workspace 不得產生任何 warning。"""
    (tmp_path / "a.py").write_text("a", encoding="utf-8")
    (tmp_path / "sub").mkdir()
    (tmp_path / "sub" / "b.py").write_text("b", encoding="utf-8")

    warnings: list[str] = []
    list(walk(tmp_path, _ctx(tmp_path), warnings=warnings))
    assert warnings == []
