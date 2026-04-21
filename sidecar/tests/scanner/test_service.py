"""TDD red tests for `scanner/service.py::scan` — Task 6.1.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Workspace scan endpoint
  Requirement: Deferred subsystem schema preservation
  Requirement: Content type summary generation
  Requirement: File classification by extension and content sniffing
  Requirement: Encoding detection fallback chain
  Requirement: Language identification
  Requirement: Symlink handling without following
  Requirement: Sandbox boundary enforcement

service.scan() 是 walk → classify → encode → language → summary 的 orchestrator。
它吃 `(workspace_root: str, ctx: ToolContext)`，吐出完整 `ScanResult`，包括
skeleton 階段 MUST 保留的 stub defaults（`git=None`、`is_monorepo=False`、
`monorepo_type=None`、`sub_packages=[]`、每個 `FileEntry.sanitize_stats={}`）。
"""
from __future__ import annotations

import sys
from datetime import datetime
from pathlib import Path

import pytest

from codebus_agent.sandbox import ToolContext
from codebus_agent.scanner.models import FileEntry, ScanResult, Symlink
from codebus_agent.scanner.service import scan


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------


def _ctx(root: Path) -> ToolContext:
    return ToolContext(workspace_root=root, workspace_type="folder")


def _file_by_path(result: ScanResult, path: str) -> FileEntry:
    matches = [e for e in result.files if e.path == path]
    assert len(matches) == 1, (
        f"expected exactly one FileEntry with path={path!r}, "
        f"found {len(matches)} in {[e.path for e in result.files]}"
    )
    return matches[0]


_POSIX_ONLY = pytest.mark.skipif(
    sys.platform == "win32",
    reason="symlink creation on Windows requires elevated privileges",
)


# ---------------------------------------------------------------------------
# 1. Happy path —— ScanResult shape 與 top-level 欄位
# ---------------------------------------------------------------------------


def test_scan_empty_workspace_returns_minimal_scan_result(tmp_path: Path) -> None:
    """空 workspace → 空 files / symlinks / warnings，summary=all-zero / mixed。"""
    result = scan(str(tmp_path), _ctx(tmp_path))
    assert isinstance(result, ScanResult)
    assert result.files == []
    assert result.symlinks == []
    assert result.warnings == []
    assert result.content_summary.total_files == 0
    assert result.content_summary.dominant_category == "mixed"


def test_scan_workspace_root_resolved_absolute(tmp_path: Path) -> None:
    """ScanResult.workspace_root 應是 resolved 後的絕對路徑字串。"""
    result = scan(str(tmp_path), _ctx(tmp_path))
    wr = Path(result.workspace_root)
    assert wr.is_absolute()
    # 比對 resolve 後的路徑（ToolContext validator 已 resolve）
    assert wr == tmp_path.resolve(strict=False)


def test_scan_timestamps_are_iso_ordered(tmp_path: Path) -> None:
    """scan_started_at <= scan_completed_at，兩者皆為 datetime 且 tz-aware。"""
    result = scan(str(tmp_path), _ctx(tmp_path))
    assert isinstance(result.scan_started_at, datetime)
    assert isinstance(result.scan_completed_at, datetime)
    assert result.scan_completed_at >= result.scan_started_at


# ---------------------------------------------------------------------------
# 2. Deferred subsystem stubs（schema preservation 不變式）
# ---------------------------------------------------------------------------


def test_scan_git_defaults_none(tmp_path: Path) -> None:
    """skeleton 階段 ScanResult.git 一律 None，即便 workspace 內有 .git/。"""
    # 弄一個假 .git 目錄，它會被 built-in ignore 跳過但 skeleton 不該讀 git
    (tmp_path / ".git").mkdir()
    (tmp_path / ".git" / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
    (tmp_path / "app.py").write_text("x = 1\n", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    assert result.git is None


def test_scan_is_monorepo_defaults_false(tmp_path: Path) -> None:
    """即便有 pnpm-workspace.yaml，skeleton is_monorepo 仍為 False。"""
    (tmp_path / "pnpm-workspace.yaml").write_text(
        "packages:\n  - packages/*\n", encoding="utf-8"
    )
    result = scan(str(tmp_path), _ctx(tmp_path))
    assert result.is_monorepo is False
    assert result.content_summary.is_monorepo is False


def test_scan_monorepo_type_and_sub_packages_default_stub(tmp_path: Path) -> None:
    """monorepo_type=None、sub_packages=[] 是 deferred stub 的契約欄位。"""
    result = scan(str(tmp_path), _ctx(tmp_path))
    assert result.monorepo_type is None
    assert result.sub_packages == []


def test_scan_file_entries_have_empty_sanitize_stats(tmp_path: Path) -> None:
    """每個 FileEntry.sanitize_stats 在 skeleton 階段 MUST 為空 dict。"""
    (tmp_path / "a.py").write_text("a", encoding="utf-8")
    (tmp_path / "b.md").write_text("# b", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    assert len(result.files) == 2
    for entry in result.files:
        assert entry.sanitize_stats == {}


# ---------------------------------------------------------------------------
# 3. Pipeline：walk → classify → encoding → language → content
# ---------------------------------------------------------------------------


def test_scan_text_file_populates_encoding_language_content(tmp_path: Path) -> None:
    """UTF-8 python 檔：kind=text、encoding=utf-8、language=python、content 正確。

    用 ``write_bytes`` 避免 Windows ``write_text`` 將 ``\\n`` 自動轉 ``\\r\\n``，
    確保 content assertion 在 POSIX / Windows 上結果一致。
    """
    source = "print('hello')\n"
    (tmp_path / "main.py").write_bytes(source.encode("utf-8"))

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "main.py")
    assert entry.kind == "text"
    assert entry.encoding == "utf-8"
    assert entry.content == source
    assert entry.language == "python"
    assert entry.language_confidence == "extension"


def test_scan_big5_file_decoded_via_fallback_chain(tmp_path: Path) -> None:
    """Big5 檔：UTF-8 decode 失敗 → fallback chain 命中 big5。"""
    payload = "繁體中文測試資料"
    (tmp_path / "notes.txt").write_bytes(payload.encode("big5"))

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "notes.txt")
    assert entry.kind == "text"
    assert entry.encoding == "big5"
    assert entry.content == payload


def test_scan_binary_file_has_no_content_or_encoding(tmp_path: Path) -> None:
    """副檔名屬於 BINARY_EXTENSIONS → kind=binary，content/encoding 必為 None。"""
    (tmp_path / "logo.png").write_bytes(b"\x89PNG\r\n\x1a\n" + b"\x00" * 64)

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "logo.png")
    assert entry.kind == "binary"
    assert entry.content is None
    assert entry.encoding is None


def test_scan_null_byte_file_classified_as_binary(tmp_path: Path) -> None:
    """head 含 NUL 的檔案（無已知副檔名）→ kind=binary，encoding/content=None。"""
    (tmp_path / "data.dat").write_bytes(b"hello\x00world")

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "data.dat")
    assert entry.kind == "binary"
    assert entry.content is None
    assert entry.encoding is None


def test_scan_generated_file_has_no_content(tmp_path: Path) -> None:
    """`*.min.js` → kind=generated，content/encoding 必為 None。"""
    (tmp_path / "app.min.js").write_text("var a=1;var b=2;", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "app.min.js")
    assert entry.kind == "generated"
    assert entry.content is None
    assert entry.encoding is None


def test_scan_lockfile_has_no_content(tmp_path: Path) -> None:
    """uv.lock → kind=lockfile，content/encoding 必為 None；size 仍紀錄。

    ``write_bytes`` 確保 Windows 不自動換行 → size 斷言兩平台一致。
    """
    body = "# lock file\nname = 'x'\n"
    raw = body.encode("utf-8")
    (tmp_path / "uv.lock").write_bytes(raw)

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "uv.lock")
    assert entry.kind == "lockfile"
    assert entry.content is None
    assert entry.encoding is None
    assert entry.size == len(raw)


def test_scan_shebang_identifies_language_for_extensionless_file(tmp_path: Path) -> None:
    """無副檔名 + shebang `#!/usr/bin/env bash` → language=bash、confidence=shebang。"""
    (tmp_path / "run").write_text("#!/usr/bin/env bash\necho hi\n", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "run")
    assert entry.language == "bash"
    assert entry.language_confidence == "shebang"


def test_scan_unknown_extension_yields_null_language(tmp_path: Path) -> None:
    """未知副檔名且無 shebang → language=None、confidence=unknown。"""
    (tmp_path / "notes.xyz").write_text("random text\n", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    entry = _file_by_path(result, "notes.xyz")
    assert entry.language is None
    assert entry.language_confidence == "unknown"


# ---------------------------------------------------------------------------
# 4. Ignore / symlink / sandbox
# ---------------------------------------------------------------------------


def test_scan_applies_gitignore(tmp_path: Path) -> None:
    """.gitignore 規則應在 service 整條 pipeline 上生效。"""
    (tmp_path / ".gitignore").write_text("*.log\n", encoding="utf-8")
    (tmp_path / "keep.py").write_text("k", encoding="utf-8")
    (tmp_path / "drop.log").write_text("d", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    paths = {e.path for e in result.files}
    assert "keep.py" in paths
    assert "drop.log" not in paths


def test_scan_builtin_always_ignore_applied(tmp_path: Path) -> None:
    """built-in always-ignore（node_modules）全 subtree 不進 ScanResult。"""
    (tmp_path / "node_modules").mkdir()
    (tmp_path / "node_modules" / "foo.js").write_text("m", encoding="utf-8")
    (tmp_path / "src.py").write_text("s", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    paths = {e.path for e in result.files}
    assert paths == {"src.py"}


@_POSIX_ONLY
def test_scan_records_symlink_without_following(tmp_path: Path) -> None:
    """in-workspace symlink → Symlink 條目出現於 symlinks；link 本身不在 files。"""
    (tmp_path / "real.py").write_text("real", encoding="utf-8")
    (tmp_path / "link.py").symlink_to("real.py")

    result = scan(str(tmp_path), _ctx(tmp_path))
    assert {e.path for e in result.files} == {"real.py"}
    assert len(result.symlinks) == 1
    sl = result.symlinks[0]
    assert isinstance(sl, Symlink)
    assert sl.path == "link.py"
    assert sl.resolved_in_workspace is True


def test_scan_sandbox_escape_produces_warning(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """ensure_in_workspace fail → entry 被 skip，ScanResult.warnings 累積訊息。"""
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

    result = scan(str(tmp_path), _ctx(tmp_path))
    paths = {e.path for e in result.files}
    assert "keep.py" in paths
    assert "bad.py" not in paths
    assert any("bad" in w for w in result.warnings), (
        f"expected a warning mentioning 'bad', got: {result.warnings!r}"
    )


# ---------------------------------------------------------------------------
# 5. ContentTypeSummary 整合
# ---------------------------------------------------------------------------


def test_scan_content_summary_counts_match_files(tmp_path: Path) -> None:
    """summary.total_files / kind_counts / language_counts 應與 ScanResult.files 一致。"""
    (tmp_path / "a.py").write_text("a", encoding="utf-8")
    (tmp_path / "b.py").write_text("b", encoding="utf-8")
    (tmp_path / "README.md").write_text("# title\n", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    s = result.content_summary
    assert s.total_files == len(result.files) == 3
    assert s.language_counts.get("python") == 2
    assert s.language_counts.get("markdown") == 1
    assert s.kind_counts.get("text") == 3


def test_scan_content_summary_has_docs_detected(tmp_path: Path) -> None:
    """README.md → has_docs=True。"""
    (tmp_path / "README.md").write_text("# hi\n", encoding="utf-8")
    (tmp_path / "a.py").write_text("a", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    assert result.content_summary.has_docs is True


def test_scan_content_summary_has_tests_detected(tmp_path: Path) -> None:
    """`tests/` 目錄 → has_tests=True。"""
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "test_sample.py").write_text("def test_ok(): pass\n", encoding="utf-8")
    (tmp_path / "main.py").write_text("m", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    assert result.content_summary.has_tests is True


def test_scan_python_dominant_repo(tmp_path: Path) -> None:
    """code 檔 > 60% → dominant_category='code'，python 進 dominant_languages 首位。"""
    for i in range(8):
        (tmp_path / f"mod{i}.py").write_text(f"x = {i}\n", encoding="utf-8")
    for i in range(2):
        (tmp_path / f"doc{i}.md").write_text(f"# doc {i}\n", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    s = result.content_summary
    assert s.dominant_category == "code"
    assert s.dominant_languages[0] == "python"
    assert s.category_counts.get("code") == 8


# ---------------------------------------------------------------------------
# 6. ScanStats
# ---------------------------------------------------------------------------


def test_scan_stats_populated(tmp_path: Path) -> None:
    """ScanStats 應有合理非負值；included 應等於 files 長度。"""
    (tmp_path / "a.py").write_text("a", encoding="utf-8")
    (tmp_path / "b.py").write_text("b", encoding="utf-8")

    result = scan(str(tmp_path), _ctx(tmp_path))
    st = result.stats
    assert st.total_files_walked >= 2
    assert st.total_files_included == len(result.files) == 2
    assert st.total_bytes_read >= 2  # at least the two 1-byte files
    assert st.duration_seconds >= 0.0
    assert st.quarantined_count >= 0
    assert st.skipped_count >= 0


def test_scan_stats_skipped_count_reflects_warnings(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """sandbox 違規造成的 skip 應反映在 stats.skipped_count 或 warnings 長度對齊。"""
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

    result = scan(str(tmp_path), _ctx(tmp_path))
    # 至少一條 warning 且 stats.skipped_count 應 >=1
    assert len(result.warnings) >= 1
    assert result.stats.skipped_count >= 1
