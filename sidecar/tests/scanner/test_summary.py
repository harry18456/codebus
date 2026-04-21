"""測試 codebus_agent.scanner.summary.build_summary 的內容摘要契約。

對應 spec Requirement「Content type summary generation」
（`openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md`）以及
`openspec/changes/scanner-skeleton/tasks.md` Task 3.4 的 TDD 紅燈階段。

build_summary 將 `list[FileEntry]` 聚合為 `ContentTypeSummary`，依據
`docs/module-1-scanner.md` §十一 的分類規則：

    - code   — language ∈ 常見語言白名單，且路徑 / 檔名非測試
    - docs   — markdown/rst/asciidoc、路徑含 docs/、或檔名以 README/CHANGELOG 起頭
    - config — yaml/toml/json/ini、*.config.*、Dockerfile、Makefile
    - test   — 路徑含 tests/ / test/ / __tests__/ / spec/，或檔名符合 *_test.py
               / *.test.(ts|js) / *.spec.(ts|js)
    - other  — 其餘（含 language is None）

dominant_category 為單一分類 > 60% 時該分類，否則為 "mixed"；monorepo 偵測
在 skeleton 階段延後，is_monorepo 恆為 False。

本測試**不觸碰檔案系統**，`FileEntry` 物件直接在測試內建構。
"""

from __future__ import annotations

import pytest

from codebus_agent.scanner.models import ContentTypeSummary, FileEntry

# 紅燈階段：此 import 預期會 ImportError，實作在 Task 4.4 才落地。
from codebus_agent.scanner.summary import build_summary


# ---------------------------------------------------------------------------
# Helper — 建構 text-kind FileEntry（大多數分類測試的主角）
# ---------------------------------------------------------------------------


def _text(path: str, language: str | None) -> FileEntry:
    """建立最小可行的 text FileEntry；size/encoding/content 採合理預設。"""
    return FileEntry(
        path=path,
        size=100,
        kind="text",
        language=language,
        language_confidence="extension" if language else "unknown",
        encoding="utf-8",
        content="...",
    )


# ---------------------------------------------------------------------------
# 空輸入：全零初始化 + mixed
# ---------------------------------------------------------------------------


def test_empty_files_returns_mixed_zeroed_summary() -> None:
    summary = build_summary([])

    assert isinstance(summary, ContentTypeSummary)
    assert summary.total_files == 0
    assert summary.dominant_category == "mixed"
    assert summary.dominant_languages == []
    assert summary.has_tests is False
    assert summary.has_docs is False
    assert summary.is_monorepo is False
    # 計數 dict 應為空或全零；兩者都算合理的「zero-initialized」
    assert sum(summary.kind_counts.values()) == 0
    assert sum(summary.language_counts.values()) == 0
    assert sum(summary.category_counts.values()) == 0


# ---------------------------------------------------------------------------
# dominant_category — 單一分類超過 60% 即為 dominant；否則 mixed
# ---------------------------------------------------------------------------


def test_code_dominant_60_percent() -> None:
    files = [_text(f"src/mod_{i}.py", "python") for i in range(8)]
    files += [_text("README.md", "markdown"), _text("docs/guide.md", "markdown")]

    summary = build_summary(files)

    assert summary.total_files == 10
    assert summary.dominant_category == "code"
    assert summary.category_counts["code"] == 8
    assert summary.category_counts["docs"] == 2
    assert summary.dominant_languages[0] == "python"


def test_mixed_when_no_category_exceeds_60_percent() -> None:
    # 5 code + 4 docs + 3 config = 12；沒有任何分類 > 7.2
    files = [_text(f"src/m_{i}.py", "python") for i in range(5)]
    files += [_text(f"docs/d_{i}.md", "markdown") for i in range(4)]
    files += [_text(f"cfg_{i}.toml", "toml") for i in range(3)]

    summary = build_summary(files)

    assert summary.total_files == 12
    assert summary.dominant_category == "mixed"


# ---------------------------------------------------------------------------
# test 分類 — 目錄命名 vs 檔名 pattern 皆須偵測
# ---------------------------------------------------------------------------


def test_tests_directory_detected() -> None:
    files = [_text("tests/foo_test.py", "python")]

    summary = build_summary(files)

    assert summary.has_tests is True
    assert summary.category_counts.get("test", 0) >= 1


def test_pytest_style_test_filename_detected() -> None:
    # 即使不在 tests/ 目錄，*_test.py 仍須歸為 test
    files = [_text("src/foo_test.py", "python")]

    summary = build_summary(files)

    assert summary.category_counts.get("test", 0) == 1
    assert summary.has_tests is True


def test_jest_test_filename_detected() -> None:
    files = [_text("src/foo.test.ts", "typescript")]

    summary = build_summary(files)

    assert summary.category_counts.get("test", 0) == 1
    assert summary.has_tests is True


def test_spec_filename_detected() -> None:
    files = [_text("src/foo.spec.ts", "typescript")]

    summary = build_summary(files)

    assert summary.category_counts.get("test", 0) == 1


# ---------------------------------------------------------------------------
# docs 分類 — 副檔名 vs README 檔名
# ---------------------------------------------------------------------------


def test_docs_from_markdown() -> None:
    files = [_text(f"docs/guide_{i}.md", "markdown") for i in range(3)]

    summary = build_summary(files)

    assert summary.category_counts["docs"] == 3
    assert summary.has_docs is True


def test_docs_from_readme_filename() -> None:
    # 單一 README.md 放在 root，也必須被認出是 docs
    files = [_text("README.md", "markdown")]

    summary = build_summary(files)

    assert summary.has_docs is True
    assert summary.category_counts.get("docs", 0) == 1


# ---------------------------------------------------------------------------
# config 分類 — 副檔名 vs 特殊檔名（Dockerfile / Makefile）
# ---------------------------------------------------------------------------


def test_config_from_toml() -> None:
    files = [_text("pyproject.toml", "toml")]

    summary = build_summary(files)

    assert summary.category_counts.get("config", 0) == 1


def test_config_from_dockerfile() -> None:
    # Dockerfile 沒副檔名、classify 不會給 language；但 build_summary 仍須認得檔名
    files = [_text("Dockerfile", None)]

    summary = build_summary(files)

    assert summary.category_counts.get("config", 0) == 1


# ---------------------------------------------------------------------------
# other 分類 — 無 language 也無特殊 hint
# ---------------------------------------------------------------------------


def test_other_when_no_language_and_no_hint() -> None:
    files = [_text("notes.xyz", None)]

    summary = build_summary(files)

    assert summary.category_counts.get("other", 0) == 1
    assert summary.category_counts.get("code", 0) == 0
    assert summary.category_counts.get("docs", 0) == 0
    assert summary.category_counts.get("config", 0) == 0
    assert summary.category_counts.get("test", 0) == 0


# ---------------------------------------------------------------------------
# is_monorepo — skeleton 階段恆為 False（延後到後續 change 實作）
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "files",
    [
        [],
        [_text("src/app.py", "python")],
        [
            _text("packages/a/src/index.ts", "typescript"),
            _text("packages/b/src/index.ts", "typescript"),
            _text("pnpm-workspace.yaml", "yaml"),
        ],
    ],
    ids=["empty", "single-package", "monorepo-shaped"],
)
def test_is_monorepo_always_false_in_skeleton(files: list[FileEntry]) -> None:
    summary = build_summary(files)
    assert summary.is_monorepo is False


# ---------------------------------------------------------------------------
# dominant_languages — top 3 by file count；None 排除
# ---------------------------------------------------------------------------


def test_dominant_languages_top3() -> None:
    files = [_text(f"a_{i}.py", "python") for i in range(5)]
    files += [_text(f"b_{i}.ts", "typescript") for i in range(4)]
    files += [_text(f"c_{i}.rs", "rust") for i in range(3)]
    files += [_text(f"d_{i}.go", "go") for i in range(2)]

    summary = build_summary(files)

    # 僅前三名，且依計數降冪
    assert summary.dominant_languages == ["python", "typescript", "rust"]
    assert "go" not in summary.dominant_languages


def test_dominant_languages_excludes_none() -> None:
    files = [_text(f"a_{i}.py", "python") for i in range(3)]
    files += [_text("unknown_1.bin", None), _text("unknown_2.bin", None)]

    summary = build_summary(files)

    assert summary.dominant_languages[0] == "python"
    assert None not in summary.dominant_languages  # type: ignore[operator]


# ---------------------------------------------------------------------------
# kind_counts — 反映 FileEntry.kind 的實際分布
# ---------------------------------------------------------------------------


def test_kind_counts_tracks_all_kinds() -> None:
    files: list[FileEntry] = [
        # text
        FileEntry(
            path="src/app.py",
            size=100,
            kind="text",
            language="python",
            language_confidence="extension",
            encoding="utf-8",
            content="...",
        ),
        FileEntry(
            path="README.md",
            size=50,
            kind="text",
            language="markdown",
            language_confidence="extension",
            encoding="utf-8",
            content="...",
        ),
        # binary
        FileEntry(
            path="assets/logo.png",
            size=2048,
            kind="binary",
            language=None,
            language_confidence="unknown",
            encoding=None,
            content=None,
        ),
        # lockfile
        FileEntry(
            path="package-lock.json",
            size=4096,
            kind="lockfile",
            language=None,
            language_confidence="unknown",
            encoding=None,
            content=None,
        ),
    ]

    summary = build_summary(files)

    assert summary.kind_counts.get("text", 0) == 2
    assert summary.kind_counts.get("binary", 0) == 1
    assert summary.kind_counts.get("lockfile", 0) == 1
    assert summary.total_files == 4
