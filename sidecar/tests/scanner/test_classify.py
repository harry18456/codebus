"""測試 codebus_agent.scanner.classify.classify 的檔案分類契約。

對應 spec Requirement「File classification by extension and content sniffing」
（`openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md`）以及
`openspec/changes/scanner-skeleton/tasks.md` Task 3.2 的 TDD 紅燈階段。

classify 採用優先順序：
    1. generated（*.min.js / *.min.css / *.bundle.js）
    2. lockfile（*-lock.json / yarn.lock / poetry.lock / Cargo.lock / uv.lock / Gemfile.lock）
    3. binary（副檔名位於 binary set）
    4. binary（前 8 KB head 含 null byte）
    5. oversized（size > max_file_size_kb * 1024）
    6. text（其他）

本測試**不觸碰檔案系統**，所有位元組資料透過 head_bytes 參數注入。
"""

from __future__ import annotations

from pathlib import Path

import pytest

# 紅燈階段：此 import 預期會 ImportError，實作在 Task 4.2 才落地。
from codebus_agent.scanner.classify import classify


# ---------------------------------------------------------------------------
# 副檔名判為 binary
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "filename",
    [
        "image.png",
        "photo.jpg",
        "photo.jpeg",
        "animation.gif",
        "favicon.ico",
        "hero.webp",
        "report.pdf",
        "archive.zip",
        "bundle.tar",
        "bundle.tar.gz",
        "backup.7z",
        "installer.exe",
        "driver.dll",
        "lib.so",
        "lib.dylib",
        "font.woff",
        "font.woff2",
        "font.ttf",
        "font.eot",
        "song.mp3",
        "clip.mp4",
        "sound.wav",
        "audio.ogg",
    ],
)
def test_binary_by_extension(filename: str) -> None:
    """副檔名落在 binary set 時即使 head_bytes 為空也要判為 binary。"""

    assert classify(Path(filename), size=1024, head_bytes=b"") == "binary"


def test_png_extension_with_empty_head() -> None:
    """.png 不需要內容探測即可判為 binary（優先於 null-byte sniff）。"""

    assert classify(Path("logo.png"), size=0, head_bytes=b"") == "binary"


def test_pdf_extension() -> None:
    assert classify(Path("manual.pdf"), size=4096, head_bytes=b"%PDF-1.7") == "binary"


def test_exe_extension() -> None:
    assert classify(Path("setup.exe"), size=2048, head_bytes=b"MZ\x00") == "binary"


# ---------------------------------------------------------------------------
# lockfile 檔名
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "filename",
    [
        "uv.lock",
        "yarn.lock",
        "poetry.lock",
        "Cargo.lock",
        "Gemfile.lock",
    ],
)
def test_lockfile_by_exact_name(filename: str) -> None:
    assert classify(Path(filename), size=2048, head_bytes=b"{}") == "lockfile"


def test_package_lock_json_matches_lock_suffix() -> None:
    """package-lock.json 應符合 *-lock.json glob。"""

    assert (
        classify(Path("package-lock.json"), size=10_240, head_bytes=b'{"name":"x"}')
        == "lockfile"
    )


def test_pnpm_lock_json_matches_lock_suffix() -> None:
    """任意 *-lock.json 都應命中 lockfile 規則。"""

    assert (
        classify(Path("composer-lock.json"), size=1024, head_bytes=b"{}") == "lockfile"
    )


# ---------------------------------------------------------------------------
# generated（打包 / 壓縮產物）
# ---------------------------------------------------------------------------


def test_min_js_is_generated() -> None:
    assert (
        classify(Path("app.min.js"), size=2048, head_bytes=b"!function(){}()")
        == "generated"
    )


def test_min_css_is_generated() -> None:
    assert (
        classify(Path("styles.min.css"), size=2048, head_bytes=b".a{color:red}")
        == "generated"
    )


def test_bundle_js_is_generated() -> None:
    assert (
        classify(Path("bundle.bundle.js"), size=4096, head_bytes=b"(() => {})();")
        == "generated"
    )


# ---------------------------------------------------------------------------
# 優先順序：generated > lockfile > binary-ext > null-byte > oversized > text
# ---------------------------------------------------------------------------


def test_generated_beats_oversized() -> None:
    """vendor.min.js 即使 10 MB 仍應是 generated（優先於 oversized）。"""

    ten_mb = 10 * 1024 * 1024
    assert (
        classify(Path("vendor.min.js"), size=ten_mb, head_bytes=b"!function(){}();")
        == "generated"
    )


def test_lockfile_beats_oversized() -> None:
    """大型 *-lock.json 仍應判為 lockfile，不降級成 oversized。"""

    big_size = 600 * 1024  # 600 KB，超過預設 512 KB 門檻
    assert (
        classify(Path("weird-lock.json"), size=big_size, head_bytes=b"{}")
        == "lockfile"
    )


def test_binary_extension_beats_oversized() -> None:
    """副檔名為 .zip 的大檔應判為 binary 而非 oversized。"""

    big_size = 5 * 1024 * 1024
    assert (
        classify(Path("backup.zip"), size=big_size, head_bytes=b"PK\x03\x04")
        == "binary"
    )


# ---------------------------------------------------------------------------
# null-byte sniff
# ---------------------------------------------------------------------------


def test_null_byte_in_head_classified_as_binary() -> None:
    """副檔名看似 text 但 head 含 \\x00 時應判為 binary。"""

    assert (
        classify(Path("script.txt"), size=12, head_bytes=b"hello\x00world") == "binary"
    )


def test_no_null_byte_remains_text() -> None:
    """純文字內容且無 null byte，應判為 text。"""

    assert classify(Path("notes.md"), size=200, head_bytes=b"# hi\nworld") == "text"


def test_null_byte_beats_oversized() -> None:
    """含 null byte 的大檔應判為 binary，不是 oversized。"""

    big_size = 2 * 1024 * 1024
    assert (
        classify(Path("mystery.dat"), size=big_size, head_bytes=b"abc\x00def")
        == "binary"
    )


# ---------------------------------------------------------------------------
# oversized（size 超過門檻且其他規則不命中）
# ---------------------------------------------------------------------------


def test_oversized_text_file_with_default_threshold() -> None:
    """600 KB 的 markdown 檔超過預設 512 KB → oversized。"""

    assert (
        classify(Path("README.md"), size=600 * 1024, head_bytes=b"hi") == "oversized"
    )


def test_custom_max_file_size_kb_triggers_oversized() -> None:
    """max_file_size_kb=1 時，2 KB 的 txt 應判為 oversized。"""

    assert (
        classify(
            Path("notes.txt"),
            size=2 * 1024,
            head_bytes=b"plain text",
            max_file_size_kb=1,
        )
        == "oversized"
    )


def test_exactly_at_threshold_is_not_oversized() -> None:
    """size == max_file_size_kb * 1024 時為邊界內，應為 text（非嚴格大於）。"""

    threshold_bytes = 512 * 1024
    assert (
        classify(Path("boundary.md"), size=threshold_bytes, head_bytes=b"# hi")
        == "text"
    )


# ---------------------------------------------------------------------------
# text（預設 fallthrough）
# ---------------------------------------------------------------------------


def test_python_source_is_text() -> None:
    assert classify(Path("main.py"), size=200, head_bytes=b"import os\n") == "text"


def test_markdown_under_threshold_is_text() -> None:
    assert (
        classify(Path("CHANGELOG.md"), size=4096, head_bytes=b"# Changelog\n")
        == "text"
    )


def test_empty_text_file_is_text() -> None:
    """0 位元組的 text 檔應判為 text（未超過門檻、無 null byte、非 binary ext）。"""

    assert classify(Path("empty.txt"), size=0, head_bytes=b"") == "text"
