"""Language identification for the folder scanner skeleton.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Language identification
Implements Task 4.3 of openspec/changes/scanner-skeleton/tasks.md (TDD green
phase — makes `tests/scanner/test_language.py` pass).

優先序（與 spec + red test 鎖死）：
  1. 副檔名（extension）為主路徑。命中就回 (language, "extension")。
  2. 無副檔名時才 fallback 檢查 shebang interpreter；命中回 (language, "shebang")。
  3. 以上皆未命中 → (None, "unknown")。

擴充方式：直接編輯下方兩個 module-level dict（`_EXTENSION_TABLE`、
`_SHEBANG_INTERPRETER_TABLE`）。比對一律走小寫 normalize，以保持
case-insensitive 行為。
"""
from __future__ import annotations

from pathlib import Path
from typing import Literal

# ---------------------------------------------------------------------------
# 對映表（module-level，留給後續 change proposal 擴充）
# ---------------------------------------------------------------------------

# 副檔名 → 語言。key 一律已 lowercase，比對前會把 `Path.suffix` 也轉小寫。
_EXTENSION_TABLE: dict[str, str] = {
    ".py": "python",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript",
    ".rs": "rust",
    ".go": "go",
    ".rb": "ruby",
    ".md": "markdown",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".toml": "toml",
    ".json": "json",
    ".sh": "bash",
    ".html": "html",
    ".htm": "html",
    ".css": "css",
}

# Shebang interpreter token → 語言。
# sh 併入 bash 家族（skeleton 不細分；見 test_shebang_bin_sh_resolves_to_bash_family）。
_SHEBANG_INTERPRETER_TABLE: dict[str, str] = {
    "python": "python",
    "python2": "python",
    "python3": "python",
    "bash": "bash",
    "sh": "bash",
    "node": "javascript",
}

LanguageConfidence = Literal["extension", "shebang", "unknown"]


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def identify(
    path: Path,
    shebang: str | None,
) -> tuple[str | None, LanguageConfidence]:
    """回傳 `(language, confidence)`；找不到就是 `(None, "unknown")`。

    參數
    ----
    path:
        檔案路徑。只看 `path.suffix`（Python 的 suffix 僅取最後一段，例如
        `archive.tar.gz` → `.gz`）。
    shebang:
        整行 shebang 字串（含開頭 `#!`）；若 caller 沒讀到 shebang 就傳 None。
        只在 `path.suffix` 為空字串時才會被使用。
    """
    # 優先序 1：副檔名（case-insensitive）
    suffix = path.suffix.lower()
    if suffix:
        language = _EXTENSION_TABLE.get(suffix)
        if language is not None:
            return language, "extension"
        # 有副檔名但不在表裡 → 不 fallback 到 shebang，直接 unknown。
        # （spec 明文：shebang 只在「無副檔名」時啟用）
        return None, "unknown"

    # 優先序 2：shebang interpreter（僅在沒有副檔名時才走）
    if shebang is not None:
        interpreter = _interpreter_from_shebang(shebang)
        if interpreter is not None:
            language = _SHEBANG_INTERPRETER_TABLE.get(interpreter)
            if language is not None:
                return language, "shebang"

    # 優先序 3：全部摃龜
    return None, "unknown"


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _interpreter_from_shebang(shebang: str) -> str | None:
    """從一行 shebang 解出 interpreter token。

    規則：
      * 必須以 `#!` 開頭，否則視為非法 shebang 回 None。
      * `#!/usr/bin/env <cmd>` → 回 `<cmd>`（例：`python3`、`bash`、`node`）。
      * `#!/bin/sh` / `#!/usr/local/bin/python3` → 取 basename（`sh`、`python3`）。
      * 空白或只剩 `#!` → None。
    """
    if not shebang.startswith("#!"):
        return None
    body = shebang[2:].strip()
    if not body:
        return None
    parts = body.split()
    executable = parts[0]
    # `/usr/bin/env foo` 的情境：取 env 後面的第一個 token。
    if executable.endswith("/env") and len(parts) >= 2:
        return parts[1]
    # 其他情境：取 executable 的 basename（`/bin/sh` → `sh`）。
    return executable.rsplit("/", 1)[-1] or None
