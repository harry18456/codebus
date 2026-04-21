"""Content type summary aggregation for the folder scanner.

Backs spec Requirement「Content type summary generation」
（`openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md`）與
`openspec/changes/scanner-skeleton/tasks.md` Task 4.4。

`build_summary` 把一個 scan 的 `list[FileEntry]` 聚合成 `ContentTypeSummary`：

    - total_files       — 檔案總數
    - kind_counts       — 依 FileEntry.kind 計數
    - language_counts   — 依 FileEntry.language 計數（排除 None）
    - category_counts   — 依分類（code / docs / config / test / other）計數
    - dominant_category — 單一主分類 > 60% 時回傳該分類，否則 "mixed"
    - dominant_languages— 按計數降冪取前三（排除 None，計數相同以字母序 tie-break）
    - has_tests / has_docs — 是否有測試 / 文件
    - is_monorepo       — skeleton 階段恆為 False（後續 change 才偵測）

分類規則（依 `docs/module-1-scanner.md` §十一；first-match-wins，順序重要）：

    1. test   — path 落在 tests/ / test/ / __tests__/ / spec/，或檔名符合
                *_test.py / *.test.ts / *.test.js / *.spec.ts / *.spec.js
    2. docs   — language ∈ {markdown, rst, asciidoc}、或 path 含 docs/、或
                檔名以 README / CHANGELOG 起頭
    3. config — language ∈ {yaml, toml, json, ini}、或檔名符合 *.config.*、
                或檔名正好是 Dockerfile / Makefile
    4. code   — language 在語言白名單內
    5. other  — 其餘（含 language is None）

test rule 必須先於 code rule（例：`tests/foo_test.py` 語言是 python 仍應歸為
test）；Dockerfile/Makefile 等沒有 language 的檔名必須先於 other rule。
"""
from __future__ import annotations

from collections import Counter
from pathlib import PurePosixPath

from codebus_agent.scanner.models import ContentTypeSummary, FileEntry

# ---------------------------------------------------------------------------
# 分類常數
# ---------------------------------------------------------------------------

# 測試目錄前綴（以 '/' 結尾確保只比對完整目錄名）
_TEST_DIR_PREFIXES: tuple[str, ...] = (
    "tests/",
    "test/",
    "__tests__/",
    "spec/",
)

# 測試檔名的 suffix pattern（檔名層級，非 path 層級）
_TEST_FILENAME_SUFFIXES: tuple[str, ...] = (
    "_test.py",
    ".test.ts",
    ".test.js",
    ".spec.ts",
    ".spec.js",
)

# docs 相關的語言
_DOC_LANGUAGES: frozenset[str] = frozenset({"markdown", "rst", "asciidoc"})

# docs 相關的 root filename 前綴（case-sensitive — test fixture 用 README.md）
_DOC_FILENAME_PREFIXES: tuple[str, ...] = ("README", "CHANGELOG")

# config 相關的語言
_CONFIG_LANGUAGES: frozenset[str] = frozenset({"yaml", "toml", "json", "ini"})

# config 相關的特殊檔名（完整檔名比對）
_CONFIG_FILENAMES: frozenset[str] = frozenset({"Dockerfile", "Makefile"})

# code 相關的語言白名單
_CODE_LANGUAGES: frozenset[str] = frozenset(
    {
        "python",
        "typescript",
        "javascript",
        "rust",
        "go",
        "java",
        "c",
        "cpp",
        "ruby",
        "bash",
    }
)

# category_counts 的固定 key 集合（確保空輸入也能全零初始化）
_CATEGORY_KEYS: tuple[str, ...] = ("code", "docs", "config", "test", "other")


# ---------------------------------------------------------------------------
# 分類輔助函式
# ---------------------------------------------------------------------------


def _posix_path(entry: FileEntry) -> PurePosixPath:
    """把 FileEntry.path 正規化為 PurePosixPath，便於 prefix / name 檢查。"""
    return PurePosixPath(entry.path)


def _is_test(entry: FileEntry) -> bool:
    """rule 1：目錄名或檔名 pattern 符合測試慣例。"""
    path = entry.path.replace("\\", "/")  # Windows 保險（雖然 scanner 本來就吐 POSIX）
    if any(path.startswith(prefix) for prefix in _TEST_DIR_PREFIXES):
        return True

    filename = _posix_path(entry).name
    return any(filename.endswith(suffix) for suffix in _TEST_FILENAME_SUFFIXES)


def _is_docs(entry: FileEntry) -> bool:
    """rule 2：docs 類語言、docs/ 路徑或 README/CHANGELOG 檔名。"""
    if entry.language in _DOC_LANGUAGES:
        return True

    path = entry.path.replace("\\", "/")
    if "docs/" in path:
        return True

    filename = _posix_path(entry).name
    return any(filename.startswith(prefix) for prefix in _DOC_FILENAME_PREFIXES)


def _is_config(entry: FileEntry) -> bool:
    """rule 3：config 類語言、*.config.*、或 Dockerfile / Makefile。"""
    if entry.language in _CONFIG_LANGUAGES:
        return True

    filename = _posix_path(entry).name
    if filename in _CONFIG_FILENAMES:
        return True

    # *.config.* — 例如 `vite.config.ts`、`jest.config.js`
    # 至少要有兩個 '.' 且中段恰為 'config'
    parts = filename.split(".")
    if len(parts) >= 3 and "config" in parts[1:-1]:
        return True

    return False


def _is_code(entry: FileEntry) -> bool:
    """rule 4：語言白名單。"""
    return entry.language in _CODE_LANGUAGES


def _classify_category(entry: FileEntry) -> str:
    """first-match-wins 的分類函式；順序嚴格依 spec §十一。"""
    if _is_test(entry):
        return "test"
    if _is_docs(entry):
        return "docs"
    if _is_config(entry):
        return "config"
    if _is_code(entry):
        return "code"
    return "other"


# ---------------------------------------------------------------------------
# dominant_* 計算
# ---------------------------------------------------------------------------


def _compute_dominant_category(
    category_counts: dict[str, int], total_files: int
) -> str:
    """單一分類 > 60% 時回傳該分類；否則 "mixed"。

    依 spec：只有 code / docs / config 能成為 dominant；test / other 永遠不會。
    total_files == 0 時直接 mixed。
    """
    if total_files == 0:
        return "mixed"

    # 只從可成為 dominant 的分類裡挑
    eligible = {k: category_counts.get(k, 0) for k in ("code", "docs", "config")}
    winner = max(eligible, key=lambda k: eligible[k])
    winner_count = eligible[winner]

    if winner_count / total_files > 0.60:
        return winner
    return "mixed"


def _compute_dominant_languages(language_counts: dict[str, int]) -> list[str]:
    """按計數降冪取前三；同分以字母序 tie-break；排除 None（本來就不會進來）。"""
    # 排序 key：(-count, language)，確保計數高者先出，同分者字母序小的先出
    ranked = sorted(language_counts.items(), key=lambda kv: (-kv[1], kv[0]))
    return [lang for lang, _ in ranked[:3]]


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def build_summary(files: list[FileEntry]) -> ContentTypeSummary:
    """聚合 scan 結果為 `ContentTypeSummary`。

    空輸入時回傳全零 / mixed 的 summary；`is_monorepo` 在 skeleton 階段恆為 False。
    函式本身是純函式，不觸碰檔案系統、不修改傳入 list。
    """
    total_files = len(files)

    # 空輸入：全零初始化 + mixed
    if total_files == 0:
        return ContentTypeSummary(
            total_files=0,
            kind_counts={},
            language_counts={},
            category_counts={key: 0 for key in _CATEGORY_KEYS},
            dominant_category="mixed",
            dominant_languages=[],
            has_tests=False,
            has_docs=False,
            is_monorepo=False,
        )

    # kind_counts — 直接從 FileEntry.kind 聚合
    kind_counter: Counter[str] = Counter(entry.kind for entry in files)

    # language_counts — 排除 None
    language_counter: Counter[str] = Counter(
        entry.language for entry in files if entry.language is not None
    )

    # category_counts — 先以 _CATEGORY_KEYS 建立零初值，再累加
    category_counter: Counter[str] = Counter({key: 0 for key in _CATEGORY_KEYS})
    for entry in files:
        category_counter[_classify_category(entry)] += 1

    dominant_category = _compute_dominant_category(
        dict(category_counter), total_files
    )
    dominant_languages = _compute_dominant_languages(dict(language_counter))

    has_tests = category_counter["test"] > 0
    has_docs = category_counter["docs"] > 0

    return ContentTypeSummary(
        total_files=total_files,
        kind_counts=dict(kind_counter),
        language_counts=dict(language_counter),
        category_counts=dict(category_counter),
        dominant_category=dominant_category,  # type: ignore[arg-type]
        dominant_languages=dominant_languages,
        has_tests=has_tests,
        has_docs=has_docs,
        is_monorepo=False,  # skeleton 階段恆為 False（D-002 延後）
    )


__all__ = ["build_summary"]
