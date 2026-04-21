"""Encoding detection fallback chain for the folder scanner.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Encoding detection fallback chain

Implements tasks.md Task 4.1 (TDD green)：依序嘗試 UTF-8（含 BOM）→ UTF-16（僅
限 BOM 存在時）→ Big5 → GBK → Shift_JIS → charset-normalizer 保底；全部失敗
才回 ``(None, None)``。Scanner 端會把 ``(None, None)`` 重新歸類為 binary。

設計守則：
  * UTF-16 只在 BOM（``\\xff\\xfe`` / ``\\xfe\\xff``）出現時才啟用，禁用「每隔
    一 byte 是 0x00」等統計式 heuristic —— 否則會把 ASCII 文字誤判成 UTF-16。
  * CJK 三家（Big5 / GBK / Shift_JIS）在 short payload 上 byte range 嚴重重疊，
    光靠「第一個 strict decode 成功就收工」會把 GBK payload 誤判成 Big5（反之
    亦然）。因此我們對三個 codec 全都試 strict decode + roundtrip，再用「解碼
    結果落在該 codec Level 1（常用字）區段的比例」選出最可信的那個；分數平手
    時才退回 spec 的 ``big5 → gbk → shift_jis`` 順序。
  * charset-normalizer 回傳的 best match 要 **再做一次** strict decode 驗證；
    若 strict decode 失敗（例如全 ``\\x00\\xff`` 這類 degenerate payload），視
    為無匹配往下掉，讓上層回 ``(None, None)``。
"""
from __future__ import annotations

from charset_normalizer import from_bytes

# UTF-8 / UTF-16 BOM 常數 —— 集中在模組頂，避免 magic bytes 散落。
_UTF8_BOM = b"\xef\xbb\xbf"
_UTF16_LE_BOM = b"\xff\xfe"
_UTF16_BE_BOM = b"\xfe\xff"

# Legacy CJK codec 嘗試清單：Big5（繁中）/ GBK（簡中）/ Shift_JIS（日文）。
# 順序同時也是 spec fallback 順序，score 平手時當 tie-breaker。
_CJK_CODECS: tuple[str, ...] = ("big5", "gbk", "shift_jis")

# 每個 CJK codec 的「Level 1（常用字）lead byte 區段」：解碼結果若真屬於該
# codec 的文字，re-encode 後的 lead byte 應大量落在此區；誤判成其他 codec
# 則會往 Level 2 / 罕見區飄。這是短 payload 上區分三家 CJK codec 的關鍵信號。
#   * Big5 Level 1: A4-C6（常用 5401 字）
#   * GBK Level 1: B0-D7（GB2312 一級常用 3755 字）
#   * Shift_JIS Level 1: 88-98 + E0-EA（JIS 第一水準 2965 字）
_CJK_LEVEL1_LEADS: dict[str, frozenset[int]] = {
    "big5": frozenset(range(0xA4, 0xC7)),
    "gbk": frozenset(range(0xB0, 0xD8)),
    "shift_jis": frozenset(list(range(0x88, 0x99)) + list(range(0xE0, 0xEB))),
}


def detect_encoding(data: bytes) -> tuple[str | None, str | None]:
    """回傳 ``(encoding_name, decoded_text)``；全部 fallback 失敗時回 ``(None, None)``。

    Fallback chain（first viable wins）：
      1. UTF-8（優先檢查 BOM，無 BOM 也嘗試）
      2. UTF-16 LE/BE —— 只在 BOM 存在時
      3. Big5 / GBK / Shift_JIS（strict decode + Level 1 頻度仲裁）
      4. charset-normalizer best match（再以 strict decode 驗證）
      5. ``(None, None)``
    """
    # 1. UTF-8：先剝 BOM（若有）再 strict decode。空 bytes 也會在這一步命中，
    #    得到 ``("utf-8", "")``。
    utf8_result = _try_utf8(data)
    if utf8_result is not None:
        return utf8_result

    # 2. UTF-16：僅限 BOM 存在時自動判定；無 BOM 絕不啟用。
    utf16_result = _try_utf16_with_bom(data)
    if utf16_result is not None:
        return utf16_result

    # 3. CJK legacy codecs：strict decode 全部試過，再用 Level 1 頻度挑最佳。
    cjk_result = _try_cjk_codecs(data)
    if cjk_result is not None:
        return cjk_result

    # 4. charset-normalizer 保底；對 best match 再做一次 strict decode 驗證。
    cn_result = _try_charset_normalizer(data)
    if cn_result is not None:
        return cn_result

    # 5. 全失敗 —— scanner 會把這視為 binary。
    return None, None


def _try_utf8(data: bytes) -> tuple[str, str] | None:
    """UTF-8 嘗試：有 BOM 先剝掉再 decode；無 BOM 也直接 strict decode。"""
    if data.startswith(_UTF8_BOM):
        try:
            return "utf-8", data[len(_UTF8_BOM) :].decode("utf-8", errors="strict")
        except UnicodeDecodeError:
            return None
    try:
        return "utf-8", data.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return None


def _try_utf16_with_bom(data: bytes) -> tuple[str, str] | None:
    """UTF-16 只在 BOM 存在時啟用；BOM 缺席一律回 ``None`` 讓下一層處理。"""
    if data.startswith(_UTF16_LE_BOM):
        try:
            return "utf-16", data[len(_UTF16_LE_BOM) :].decode("utf-16-le", errors="strict")
        except UnicodeDecodeError:
            return None
    if data.startswith(_UTF16_BE_BOM):
        try:
            return "utf-16", data[len(_UTF16_BE_BOM) :].decode("utf-16-be", errors="strict")
        except UnicodeDecodeError:
            return None
    return None


def _try_cjk_codecs(data: bytes) -> tuple[str, str] | None:
    """對 Big5 / GBK / Shift_JIS 三家 codec 全試 strict decode，挑最可信的。

    挑選策略：
      * strict decode 失敗的直接淘汰
      * decoded 字串 re-encode 回原 codec 的 lead byte 落在 Level 1 區段的比例
        最高者勝出
      * 比例平手時按 ``_CJK_CODECS`` 順序（spec 要求的 big5 → gbk → shift_jis）
    """
    best_codec: str | None = None
    best_text: str | None = None
    best_score: float = -1.0
    for codec in _CJK_CODECS:
        try:
            decoded = data.decode(codec, errors="strict")
        except UnicodeDecodeError:
            continue
        score = _level1_lead_ratio(decoded, codec)
        # 嚴格「大於」—— 相同分數讓更前面的 codec 保留（tie-breaker 按 spec 順序）
        if score > best_score:
            best_score = score
            best_codec = codec
            best_text = decoded
    if best_codec is None:
        return None
    # best_text 在 best_codec 非 None 時必不為 None；顯式 cast 幫 mypy 收尾。
    assert best_text is not None
    return best_codec, best_text


def _level1_lead_ratio(decoded: str, codec: str) -> float:
    """計算 ``decoded`` 在 ``codec`` 下的「Level 1 lead byte 比例」。

    先把字串以目標 codec re-encode 回 bytes，再數 multi-byte sequence 的 lead
    byte 有多少落在 ``_CJK_LEVEL1_LEADS[codec]`` 裡。ASCII byte（< 0x80）不計
    入分母；若 decoded 根本無法被該 codec encode，回 ``-1.0`` 代表完全不該選
    這個 codec。
    """
    try:
        raw = decoded.encode(codec)
    except UnicodeEncodeError:
        # decoded 裡有字元不在 codec 字集 —— 強訊號「不是這個 codec 解的」。
        return -1.0
    level1_leads = _CJK_LEVEL1_LEADS[codec]
    total = 0
    hits = 0
    # 掃 lead byte：< 0x80 是 ASCII 跳過；否則視為 multi-byte lead，吃兩 byte。
    # CJK legacy codec 幾乎都是 DBCS（shift_jis 有少數 single-byte katakana，
    # 但出現在 BOM-free 現代文字時很少；此處的近似已足以當 disambiguator）。
    i = 0
    length = len(raw)
    while i < length:
        b = raw[i]
        if b < 0x80:
            i += 1
            continue
        total += 1
        if b in level1_leads:
            hits += 1
        i += 2
    if total == 0:
        return 0.0
    return hits / total


def _try_charset_normalizer(data: bytes) -> tuple[str, str] | None:
    """charset-normalizer 保底：取 best match，再以 strict decode 驗證一次。

    驗證的用意：charset-normalizer 偶爾會對極 degenerate 的 payload（例如
    ``\\x00\\x00...\\xff\\xff``）回一個勉強的 match；我們寧可退回 ``None`` 讓
    scanner 改把檔案歸類為 binary，也不要交出半殘的解碼結果。
    """
    matches = from_bytes(data)
    best = matches.best()
    if best is None or not best.encoding:
        return None
    try:
        # strict 再驗：若 codec / payload 對不上就放棄，讓上層回 (None, None)。
        decoded = data.decode(best.encoding, errors="strict")
    except (UnicodeDecodeError, LookupError):
        return None
    return best.encoding, decoded
