"""檔案分類核心：以副檔名與內容嗅探決定 FileKind。

對應 spec Requirement「File classification by extension and content sniffing」
（`openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md`）與
`openspec/changes/scanner-skeleton/tasks.md` Task 4.2。

分類優先順序（先命中先贏，順序不可更動）：

    1. generated  —— 打包 / 壓縮產物（`*.min.js` / `*.min.css` / `*.bundle.js`）
    2. lockfile   —— 已知 lockfile 檔名或 `*-lock.json`
    3. binary     —— 副檔名落在 binary set
    4. binary     —— head_bytes 內含 NUL（`b"\\x00"`）
    5. oversized  —— `size > max_file_size_kb * 1024`（嚴格大於，邊界值仍算 text）
    6. text       —— 預設 fallthrough

本模組**不觸碰檔案系統**：所有內容嗅探透過呼叫端注入 `head_bytes`，
以利純函式測試與排除 I/O 副作用。
"""

from __future__ import annotations

from pathlib import Path
from typing import Literal

FileKind = Literal["text", "binary", "oversized", "lockfile", "generated"]


# ---------------------------------------------------------------------------
# 模組級常數
# ---------------------------------------------------------------------------

# 視為二進位的副檔名集合（皆以小寫儲存，比對時對來源做 `.lower()`）。
# 注意：`.tar.gz` 在 `Path.suffix` 只會拿到 `.gz`，所以我們納入 `.gz` 即可，
# 不需要 `.tar.gz` 複合鍵。
BINARY_EXTENSIONS: frozenset[str] = frozenset(
    {
        # 圖片
        ".png",
        ".jpg",
        ".jpeg",
        ".gif",
        ".ico",
        ".webp",
        # 文件 / 壓縮
        ".pdf",
        ".zip",
        ".tar",
        ".gz",
        ".7z",
        # 執行檔 / 動態庫
        ".exe",
        ".dll",
        ".so",
        ".dylib",
        # 字型
        ".woff",
        ".woff2",
        ".ttf",
        ".eot",
        # 音訊 / 影片
        ".mp3",
        ".mp4",
        ".wav",
        ".ogg",
    }
)

# 常見 lockfile 的確切檔名（大小寫敏感，對齊各生態系官方命名）。
LOCKFILE_NAMES: frozenset[str] = frozenset(
    {
        "yarn.lock",
        "poetry.lock",
        "Cargo.lock",
        "uv.lock",
        "Gemfile.lock",
    }
)

# generated（打包 / 壓縮產物）檔名後綴；用 `str.endswith()` 的 tuple 形式。
GENERATED_SUFFIXES: tuple[str, ...] = (
    ".min.js",
    ".min.css",
    ".bundle.js",
)


# ---------------------------------------------------------------------------
# 公開 API
# ---------------------------------------------------------------------------


def classify(
    path: Path,
    size: int,
    head_bytes: bytes,
    *,
    max_file_size_kb: int = 512,
) -> FileKind:
    """依檔名與前 N 位元組內容判斷檔案分類。

    Args:
        path: 僅用於取檔名與副檔名，**不會被開啟**。
        size: 檔案大小（bytes），由呼叫端提供（通常由 `Path.stat().st_size`）。
        head_bytes: 檔案前段位元組（建議前 8 KB），供 NUL 嗅探用。
        max_file_size_kb: oversized 門檻（KB），嚴格大於才判 oversized。

    Returns:
        FileKind：`"text"` / `"binary"` / `"oversized"` / `"lockfile"` / `"generated"`。
    """

    name = path.name

    # 1. generated —— 打包 / 壓縮產物優先，否則大檔 vendor bundle 會被誤判成 oversized。
    if name.endswith(GENERATED_SUFFIXES):
        return "generated"

    # 2. lockfile —— 依賴鎖定檔須早於 oversized 檢查，確保巨型 lockfile 仍被正確歸類。
    if name in LOCKFILE_NAMES or name.endswith("-lock.json"):
        return "lockfile"

    # 3. binary（by extension）—— 已知二進位副檔名直接判斷，跳過 NUL 嗅探。
    suffix = path.suffix.lower()
    if suffix in BINARY_EXTENSIONS:
        return "binary"

    # 4. binary（by content sniff）—— head 含 NUL 視為二進位（涵蓋雜項 `.dat` 等）。
    if b"\x00" in head_bytes:
        return "binary"

    # 5. oversized —— 嚴格大於門檻才算超標；等於門檻仍為 text（邊界內）。
    if size > max_file_size_kb * 1024:
        return "oversized"

    # 6. text —— 預設 fallthrough。
    return "text"
