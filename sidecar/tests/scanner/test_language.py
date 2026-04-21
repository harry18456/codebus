"""Language identification contract tests.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Language identification

Drives Task 3.3 of openspec/changes/scanner-skeleton/tasks.md (TDD red phase
for the not-yet-implemented `codebus_agent.scanner.language` module).

Priority rules under test:
  1. Extension lookup wins（副檔名主路徑）→ confidence="extension"
  2. Shebang inspection used only when the file has no extension
     （無副檔名時才 fallback）→ confidence="shebang"
  3. Neither source resolves → (None, "unknown")

The target API is fixed in Task 4.3:

    def identify(
        path: Path,
        shebang: str | None,
    ) -> tuple[str | None, LanguageConfidence]

Right now `codebus_agent.scanner.language` only contains a placeholder
without `identify`, so every test in this file is expected to fail at
import time with ImportError — that's the red phase contract.
"""
from __future__ import annotations

from pathlib import Path

import pytest

# 這行故意放在 top-level：紅相位時 import 會炸，pytest collection 階段
# 就會秀出 ImportError，鎖定「module 尚未實作」狀態。
from codebus_agent.scanner.language import identify


# ---------------------------------------------------------------------------
# Primary path: extension lookup
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    ("filename", "expected_language"),
    [
        ("main.py", "python"),
        ("src/app.tsx", "typescript"),
        ("src/app.ts", "typescript"),
        ("server.js", "javascript"),
        ("lib.rs", "rust"),
        ("cmd/main.go", "go"),
        ("lib/worker.rb", "ruby"),
        ("README.md", "markdown"),
        ("config.yaml", "yaml"),
        ("config.yml", "yaml"),
        ("pyproject.toml", "toml"),
        ("package.json", "json"),
        ("scripts/run.sh", "bash"),
        ("index.html", "html"),
        ("style.css", "css"),
    ],
)
def test_extension_resolves_language_with_extension_confidence(
    filename: str, expected_language: str
) -> None:
    """副檔名命中時回 (language, "extension")，shebang 為 None 不參與。"""
    language, confidence = identify(Path(filename), None)

    assert language == expected_language
    assert confidence == "extension"


def test_extension_is_case_insensitive() -> None:
    """副檔名比對不分大小寫：`.PY` / `.Py` 仍解成 python。"""
    language, confidence = identify(Path("Main.PY"), None)

    assert language == "python"
    assert confidence == "extension"


# ---------------------------------------------------------------------------
# Precedence: extension wins over shebang
# ---------------------------------------------------------------------------


def test_extension_wins_over_shebang_when_both_present() -> None:
    """檔案同時有副檔名與 shebang 時，副檔名一律優先。

    `script.py` 帶 `#!/usr/bin/env bash` 仍必須判為 python/extension，
    不能被 shebang 覆寫——否則會破壞 spec 的優先序 1 > 2。
    """
    language, confidence = identify(Path("script.py"), "#!/usr/bin/env bash")

    assert language == "python"
    assert confidence == "extension"


# ---------------------------------------------------------------------------
# Secondary path: shebang when extension is absent
# ---------------------------------------------------------------------------


def test_shebang_env_python_resolves_when_no_extension() -> None:
    """無副檔名 + `#!/usr/bin/env python3` → python/shebang。"""
    language, confidence = identify(Path("run"), "#!/usr/bin/env python3")

    assert language == "python"
    assert confidence == "shebang"


def test_shebang_env_bash_resolves_when_no_extension() -> None:
    """無副檔名 + `#!/usr/bin/env bash` → bash/shebang。"""
    language, confidence = identify(Path("deploy"), "#!/usr/bin/env bash")

    assert language == "bash"
    assert confidence == "shebang"


def test_shebang_bin_sh_resolves_to_bash_family() -> None:
    """無副檔名 + `#!/bin/sh` → bash/shebang（sh 併入 bash 家族）。

    依 docs/module-1-scanner.md 約定，Scanner 不區分 sh vs bash；若實作
    方選擇獨立的 `sh` 分類，此 test 會需要調整對應預期值並在 rename 時
    同步 spec §語言識別章節。目前 spec 範例以 bash 家族舉例，因此此 test
    採 bash 為準。
    """
    language, confidence = identify(Path("install"), "#!/bin/sh")

    assert language == "bash"
    assert confidence == "shebang"


def test_shebang_env_node_resolves_to_javascript() -> None:
    """無副檔名 + `#!/usr/bin/env node` → javascript/shebang。"""
    language, confidence = identify(Path("build"), "#!/usr/bin/env node")

    assert language == "javascript"
    assert confidence == "shebang"


# ---------------------------------------------------------------------------
# Unknown fallthrough
# ---------------------------------------------------------------------------


def test_no_extension_and_no_shebang_is_unknown() -> None:
    """無副檔名、無 shebang → (None, "unknown")。"""
    language, confidence = identify(Path("notes"), None)

    assert language is None
    assert confidence == "unknown"


def test_no_extension_with_unrecognized_shebang_is_unknown() -> None:
    """無副檔名 + 不認識的 shebang interpreter → (None, "unknown")。"""
    language, confidence = identify(Path("wat"), "#!/usr/bin/env brainfuck")

    assert language is None
    assert confidence == "unknown"


def test_unknown_extension_is_unknown() -> None:
    """副檔名不在 mapping 內、且無 shebang → (None, "unknown")。"""
    language, confidence = identify(Path("notes.xyz"), None)

    assert language is None
    assert confidence == "unknown"


def test_compressed_archive_double_suffix_is_unknown() -> None:
    """`archive.tar.gz` 的最後一段 `.gz` 不在語言表裡 → (None, "unknown")。

    Python 的 `Path.suffix` 只看最後一個後綴；Scanner 無壓縮檔對映，
    因此整體應落到 unknown，而非被誤判為 tar / archive 的某個語言。
    """
    language, confidence = identify(Path("archive.tar.gz"), None)

    assert language is None
    assert confidence == "unknown"
