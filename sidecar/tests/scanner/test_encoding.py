"""Encoding fallback chain tests for `codebus_agent.scanner.encoding`.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Encoding detection fallback chain

Covers tasks.md Task 3.1（TDD red）：鎖定 `detect_encoding(bytes)` 回傳
`(encoding, decoded_text)` 的 fallback 順序 —— UTF-8 → UTF-16 (BOM only) →
Big5 → GBK → Shift_JIS → charset-normalizer 保底 → 全失敗回 `(None, None)`。

此檔在 Task 4.1 實作前應以 `ImportError` 紅燈（預期行為）。
"""
from __future__ import annotations

import pytest

# 預期紅燈：encoding.py 是 Task 4.1 才會長出來的 placeholder。
from codebus_agent.scanner.encoding import detect_encoding


# ---------------------------------------------------------------------------
# UTF-8 家族（含 BOM）
# ---------------------------------------------------------------------------


class TestUtf8:
    def test_ascii_utf8(self) -> None:
        data = "hello\n".encode("utf-8")
        encoding, content = detect_encoding(data)
        assert encoding == "utf-8"
        assert content == "hello\n"

    def test_utf8_with_bom(self) -> None:
        # BOM + 中文：允許回 "utf-8" 或 "utf-8-sig"（Python 兩種寫法都合理）。
        data = b"\xef\xbb\xbf" + "你好".encode("utf-8")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert encoding.lower() in {"utf-8", "utf-8-sig"}
        assert content == "你好"

    def test_empty_bytes_is_utf8_empty_string(self) -> None:
        # 空 bytes 在 UTF-8 decode 下是空字串 —— 必須在 fallback chain 第一步就命中。
        encoding, content = detect_encoding(b"")
        assert encoding == "utf-8"
        assert content == ""


# ---------------------------------------------------------------------------
# UTF-16：僅 BOM 在時才允許自動判定
# ---------------------------------------------------------------------------


class TestUtf16:
    def test_utf16_le_with_bom(self) -> None:
        # 0xFF 0xFE = UTF-16 LE BOM
        data = b"\xff\xfe" + "繁體".encode("utf-16-le")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert encoding.lower().startswith("utf-16")
        assert content == "繁體"

    def test_utf16_be_with_bom(self) -> None:
        # 0xFE 0xFF = UTF-16 BE BOM
        data = b"\xfe\xff" + "中文".encode("utf-16-be")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert encoding.lower().startswith("utf-16")
        assert content == "中文"

    def test_utf16_without_bom_does_not_auto_detect_as_utf16(self) -> None:
        # "abc" 的 utf-16-le bytes 是 b"a\x00b\x00c\x00" —— 沒 BOM。
        # Spec 要求：沒 BOM 就不能被判成 utf-16，必須往下掉到後面的 encoding
        # 或最終由 charset-normalizer 保底猜。這裡只斷言「不會是 utf-16」。
        data = "abc".encode("utf-16-le")
        encoding, content = detect_encoding(data)
        # 可能命中 big5 / gbk / shift_jis / charset-normalizer 的任何一個，
        # 也可能在極端狀況下回 (None, None) —— 但絕不能是 utf-16。
        if encoding is not None:
            assert not encoding.lower().startswith("utf-16")


# ---------------------------------------------------------------------------
# CJK legacy encodings
# ---------------------------------------------------------------------------


class TestCjkLegacy:
    def test_big5(self) -> None:
        original = "繁體中文"
        data = original.encode("big5")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert encoding.lower() in {"big5", "cp950"}
        assert content == original

    def test_gbk(self) -> None:
        original = "简体中文"
        data = original.encode("gbk")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert encoding.lower() in {"gbk", "cp936"}
        assert content == original

    def test_shift_jis(self) -> None:
        original = "日本語"
        data = original.encode("shift_jis")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert encoding.lower() in {"shift_jis", "cp932", "shift-jis"}
        assert content == original


# ---------------------------------------------------------------------------
# charset-normalizer 保底
# ---------------------------------------------------------------------------


class TestCharsetNormalizerFallback:
    def test_latin1_extended_falls_to_charset_normalizer(self) -> None:
        # extended Latin-1（é / é）: UTF-8 會 decode 失敗（0xE9 後沒有合法 continuation）
        # 因此必然走到 fallback chain 後段；不強求哪個 encoding 命中，只要有
        # 「拿到一個字串」即可 —— 這是 charset-normalizer 保底的用途。
        data = "café résumé".encode("latin-1")
        encoding, content = detect_encoding(data)
        assert encoding is not None
        assert content is not None
        # 不強制 roundtrip 比對：Big5 / GBK / Shift_JIS 都可能先吞掉這段 bytes
        # 但解出來的字串不會等於原文 —— 這個鬆散斷言是刻意設計。


# ---------------------------------------------------------------------------
# 全失敗 → binary
# ---------------------------------------------------------------------------


class TestAllFailBinary:
    def test_null_bytes_return_none(self) -> None:
        # 全 0x00 + 全 0xFF：任何 text encoding 都不會 happily decode，
        # charset-normalizer 也應該放棄。Scanner 端會把這種回傳重新歸類為 binary。
        data = b"\x00\x00\x00\x00\xff\xff\xff\xff"
        encoding, content = detect_encoding(data)
        assert encoding is None
        assert content is None


# ---------------------------------------------------------------------------
# 函式簽章契約（防止 Task 4.1 實作時手滑改回 str）
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "data",
    [
        b"",
        b"hello",
        "你好".encode("utf-8"),
        b"\x00\x00\x00\x00\xff\xff\xff\xff",
    ],
)
def test_return_type_is_two_tuple(data: bytes) -> None:
    result = detect_encoding(data)
    assert isinstance(result, tuple)
    assert len(result) == 2
    encoding, content = result
    assert encoding is None or isinstance(encoding, str)
    assert content is None or isinstance(content, str)
