"""Scanner orchestrator：walk → classify → encode → language → sanitize Pass 1 → summary。

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: Workspace scan endpoint
  Requirement: Deferred subsystem schema preservation
  Requirement: Synchronous response without SSE progress events
  Requirement: File classification by extension and content sniffing
  Requirement: Encoding detection fallback chain
  Requirement: Language identification
and openspec/changes/scanner-sanitizer-orchestration/specs/folder-scanner/spec.md
  Requirement: Pass 1 sanitizer orchestration for text FileEntries
  Requirement: Sanitize audit logging during scan
  Requirement: File classification by extension and content sniffing
    (sanitize_stats semantics after Pass 1 wiring)

設計守則：

1. **stub defaults 仍一次到位**（D-002 / spec "Deferred subsystem schema preservation"）。
   `git=None`、`is_monorepo=False`、`monorepo_type=None`、`sub_packages=[]` 的
   stub 契約不動；`FileEntry.sanitize_stats` 在 Pass 1 串通後改為「真實 kind→count，
   無命中時 `{}`」——schema 欄位未動，只是從恆 `{}` 升級成實際聚合結果。
2. **walk 已決定 kind**：walk.py 在 yield FileEntry 時已 call classify；service
   僅負責對 kind=="text"/"oversized" 的條目跑 encoding + language，並在 encoding
   fallback 全失敗時把 kind 重新歸為 binary（spec 第 92 行規則 4）。
3. **二進位族群不讀 content**：`binary` / `lockfile` / `generated` 的 content 與
   encoding 都 MUST 為 None（spec 第 96 行）；service 不對它們跑 decode。
4. **oversized 不讀全檔**：僅讀前 8 KB head 供 encoding 判定；`oversized_preview`
   在 skeleton 先留 None，後續 change 可補「前 200 行」實作而不破 schema。
5. **summary & stats** 都走 pure function（`build_summary`），service 只負責把
   walk 出的 list 丟進去 + 累計 bytes / duration / quarantined。
6. **SSE 絕不開啟**：service 以同步方式回傳單一 ScanResult；API 層要給的是
   `Content-Type: application/json`，見 spec "Synchronous response without SSE
   progress events"。
7. **Pass 1 sanitizer fail-closed**：若 `ctx.sanitizer.sanitize(...)` 拋例外，
   該檔 **不** 進 `ScanResult.files`、`warnings` 追加相對路徑、
   `stats.quarantined_count += 1`；HTTP 層仍 200（D-015 fail-closed 條款）。
"""
from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

from codebus_agent.sandbox import ToolContext
from codebus_agent.sanitizer import FileSource, SanitizerAuditLogger
from codebus_agent.scanner.encoding import detect_encoding
from codebus_agent.scanner.language import identify
from codebus_agent.scanner.models import (
    FileEntry,
    ScannerProgressCallback,
    ScannerProgressEvent,
    ScanResult,
    ScanStats,
    Symlink,
)
from codebus_agent.scanner.summary import build_summary
from codebus_agent.scanner.walk import walk

# spec "Encoding detection fallback chain" → 首 8 KB 餵 detect_encoding 足矣；
# 與 classify.py 的 head_bytes 契約相同常數。
_HEAD_BYTES_SIZE = 8 * 1024

# 不跑 encoding / language 的 kind —— 永遠保持 content=None / encoding=None。
_NO_CONTENT_KINDS: frozenset[str] = frozenset({"binary", "lockfile", "generated"})

# `Scanner progress callback hook` 觸發頻率：兩段（walking / sanitizing）都每
# 50 檔 await 一次 callback。50 是 spec §四「emit cadence」的明定數字。
_PROGRESS_EMIT_EVERY = 50


async def scan(
    workspace_root: str,
    ctx: ToolContext,
    *,
    sanitize_audit: SanitizerAuditLogger | None = None,
    rules_version: str = "2026-04-20-1",
    session_id: str = "",
    on_progress: ScannerProgressCallback | None = None,
) -> ScanResult:
    """同步掃描 ``ctx.workspace_root``，回傳完整 ``ScanResult``。

    Args
    ----
    workspace_root:
        原字串路徑（API 層原樣傳入）。實際 workspace 邊界由 ``ctx.workspace_root``
        決定 —— ``ToolContext`` validator 已把它 resolve 到絕對路徑；本字串僅用來
        寫進 ``ScanResult.workspace_root`` 時對齊（會改寫為 resolved 形式）。
    ctx:
        ``ToolContext``（frozen）；``workspace_type`` 在此 skeleton 一律為 ``folder``，
        ``topic`` 由 API 層回 501，不會走到這裡。``ctx.sanitizer`` 會被用在 Pass 1；
        scanner 對 ``kind == "text"`` 的 FileEntry 呼叫一次 ``sanitize(...)``。
    sanitize_audit:
        可選。若提供，scanner 對每個 Pass 1 sanitize 命中追加一行 JSONL
        （由 ``SanitizerAuditLogger`` 序列化寫入）。``None`` 時不寫 audit，但
        ``FileEntry.content`` / ``sanitize_stats`` 仍會被 Pass 1 更新。
    rules_version:
        寫進 sanitize_audit line 的 rules version 標記。
    session_id:
        寫進 sanitize_audit line 的 session id。

    Returns
    -------
    ScanResult
        完整填好的結果；含 ``content_summary`` / ``stats`` / ``warnings``，
        以及 deferred subsystem 的 stub defaults。``files[*].content`` 為 Pass 1
        sanitized 版本（placeholder 形式 ``<REDACTED:kind#index>``）；
        ``files[*].sanitize_stats`` 為聚合後 kind→count dict，無命中時 ``{}``。
    """
    # 兩個時間戳都用 UTC；ISO-8601 由 Pydantic serializer 負責。
    scan_started_at = datetime.now(timezone.utc)

    resolved_root = ctx.workspace_root  # already resolved by ToolContext validator

    files: list[FileEntry] = []
    symlinks: list[Symlink] = []
    warnings: list[str] = []
    total_files_walked = 0
    total_bytes_read = 0
    quarantined_count = 0
    sanitized_count = 0

    for entry in walk(resolved_root, ctx, warnings=warnings):
        if isinstance(entry, Symlink):
            # symlink 不在 "walked file" 計數內；walk 已把 symlink 從 FileEntry
            # 家族排除，所以這裡只追加到 symlinks list。
            symlinks.append(entry)
            continue

        # entry is FileEntry —— 對它跑 encoding + language + content 填充。
        total_files_walked += 1
        # `Scanner progress callback hook` walking phase emit：每 50 檔 await 一次。
        # 第一檔即觸發（current==1 % 50 == 1，但 1 != 0），所以下方用 (n % N == 0)
        # 模式：每 50 檔末尾 emit；外加保證至少一筆（最後 finally 補 emit）。
        if on_progress is not None and total_files_walked % _PROGRESS_EMIT_EVERY == 0:
            await on_progress(
                ScannerProgressEvent(
                    phase="walking",
                    current=total_files_walked,
                    total=None,
                    current_file=entry.path,
                )
            )

        enriched = _enrich_file_entry(entry, resolved_root)
        if enriched is None:
            # 讀檔失敗 → walk 階段已 warn；這裡視為 skipped，不進 files。
            continue

        # Pass 1 sanitizer orchestration —— 只對 kind=="text" 且已有 decoded
        # content 的檔跑；其餘 kind（binary / lockfile / generated / oversized）
        # 繞過並保持 sanitize_stats == {}（spec "File classification"）。
        # Fail-closed：engine 拋例外 → 該檔不進 files + warning + quarantine++
        # （D-015 / spec "Sanitize audit logging during scan"）。
        file_entry = enriched.entry
        if file_entry.kind == "text" and file_entry.content is not None:
            try:
                file_entry = _apply_pass1_sanitize(
                    file_entry,
                    ctx=ctx,
                    sanitize_audit=sanitize_audit,
                    rules_version=rules_version,
                    session_id=session_id,
                )
            except Exception as exc:
                warnings.append(
                    f"sanitize failed for {file_entry.path}: {exc}"
                )
                quarantined_count += 1
                continue

            sanitized_count += 1
            # `Scanner progress callback hook` sanitizing phase emit：同樣每 50 檔。
            if (
                on_progress is not None
                and sanitized_count % _PROGRESS_EMIT_EVERY == 0
            ):
                await on_progress(
                    ScannerProgressEvent(
                        phase="sanitizing",
                        current=sanitized_count,
                        total=None,
                        current_file=file_entry.path,
                    )
                )

        files.append(file_entry)
        total_bytes_read += enriched.bytes_read

    # `Scanner progress callback hook` 至少一筆保證：spec 規定「Callback receives
    # at least one event per phase」，所以即使檔數不足 50 也補打一發。total 此時
    # 已知（迴圈跑完），可填入。
    if on_progress is not None:
        await on_progress(
            ScannerProgressEvent(
                phase="walking",
                current=total_files_walked,
                total=total_files_walked,
                current_file=None,
            )
        )
        await on_progress(
            ScannerProgressEvent(
                phase="sanitizing",
                current=sanitized_count,
                total=sanitized_count,
                current_file=None,
            )
        )

    content_summary = build_summary(files)
    scan_completed_at = datetime.now(timezone.utc)
    duration_seconds = (scan_completed_at - scan_started_at).total_seconds()

    # skipped_count = walk 丟出的 warning 總數（sandbox 違規 / stat fail / read fail
    # + Pass 1 sanitize 失敗的 quarantine warning）。
    skipped_count = len(warnings)

    stats = ScanStats(
        total_files_walked=total_files_walked,
        total_files_included=len(files),
        total_bytes_read=total_bytes_read,
        duration_seconds=duration_seconds,
        quarantined_count=quarantined_count,
        skipped_count=skipped_count,
    )

    return ScanResult(
        workspace_root=str(resolved_root),
        scan_started_at=scan_started_at,
        scan_completed_at=scan_completed_at,
        files=files,
        symlinks=symlinks,
        # deferred subsystem stubs —— 顯式標出，勿從預設值「靜默」推。
        is_monorepo=False,
        monorepo_type=None,
        sub_packages=[],
        git=None,
        content_summary=content_summary,
        stats=stats,
        warnings=warnings,
    )


# ---------------------------------------------------------------------------
# Internal — FileEntry enrichment
# ---------------------------------------------------------------------------


class _EnrichedEntry:
    """簡化的 NamedTuple 替代 —— service 內部用，不進 schema。"""

    __slots__ = ("entry", "bytes_read")

    def __init__(self, entry: FileEntry, bytes_read: int) -> None:
        self.entry = entry
        self.bytes_read = bytes_read


def _enrich_file_entry(
    base: FileEntry, workspace_root: Path
) -> _EnrichedEntry | None:
    """把 walk 吐的 bare FileEntry 補上 encoding / language / content。

    流程：
      * binary / lockfile / generated：直接過水；encoding / content 留 None。
        language 仍嘗試以副檔名判定（例如 `.lock` 不在表裡回 None；.min.js
        不在表裡回 None；.png 回 None）。
      * text：讀前 8 KB → detect_encoding；若 fallback chain 全失敗，把 kind
        重新標為 binary 並清空 encoding/content（spec 規則 4）。
      * oversized：同 text 的處理（先讀 head 判 encoding），但 content 留 None；
        oversized_preview 在 skeleton 暫留 None（後續 change 可補）。
    """
    abs_path = workspace_root / base.path
    kind = base.kind

    # language 先試副檔名（所有 kind 都跑一次）；shebang 僅對無副檔名的 text 生效
    language, confidence = _identify_language(abs_path, kind=kind)

    if kind in _NO_CONTENT_KINDS:
        return _EnrichedEntry(
            entry=base.model_copy(
                update={
                    "language": language,
                    "language_confidence": confidence,
                    "encoding": None,
                    "content": None,
                }
            ),
            bytes_read=0,
        )

    # text / oversized → 讀 head 做 encoding detect
    try:
        with abs_path.open("rb") as fp:
            head = fp.read(_HEAD_BYTES_SIZE)
    except OSError:
        # 讀檔失敗 —— 視為 skipped；caller 會靠 len(warnings) 觀察到偏差
        return None

    encoding, decoded_head = detect_encoding(head)

    if encoding is None:
        # fallback chain 全失敗 → spec 規則 4：reclass 為 binary
        return _EnrichedEntry(
            entry=base.model_copy(
                update={
                    "kind": "binary",
                    "language": language,
                    "language_confidence": confidence,
                    "encoding": None,
                    "content": None,
                }
            ),
            bytes_read=len(head),
        )

    # 已知 encoding —— 若 text 且檔案 ≤ head 大小就直接用 decoded_head；
    # 否則需讀整檔 decode。oversized 本來就不全讀，content=None。
    if kind == "oversized":
        return _EnrichedEntry(
            entry=base.model_copy(
                update={
                    "language": language,
                    "language_confidence": confidence,
                    "encoding": encoding,
                    "content": None,
                    # oversized_preview 留空；後續 change 再補
                    "oversized_preview": None,
                }
            ),
            bytes_read=len(head),
        )

    # text：讀整檔 decode
    content, total_bytes = _decode_full(abs_path, encoding=encoding, head_decoded=decoded_head, head_len=len(head))
    if content is None:
        # 整檔 decode fail 但 head OK —— 極罕見（head 以下 byte 不合 codec）；
        # 保守 reclass 為 binary
        return _EnrichedEntry(
            entry=base.model_copy(
                update={
                    "kind": "binary",
                    "language": language,
                    "language_confidence": confidence,
                    "encoding": None,
                    "content": None,
                }
            ),
            bytes_read=total_bytes,
        )

    return _EnrichedEntry(
        entry=base.model_copy(
            update={
                "language": language,
                "language_confidence": confidence,
                "encoding": encoding,
                "content": content,
            }
        ),
        bytes_read=total_bytes,
    )


def _apply_pass1_sanitize(
    file_entry: FileEntry,
    *,
    ctx: ToolContext,
    sanitize_audit: SanitizerAuditLogger | None,
    rules_version: str,
    session_id: str,
) -> FileEntry:
    """對單一 text FileEntry 跑 Pass 1 sanitize，回傳 content / sanitize_stats
    更新後的新實例。

    Pass 1 責任（D-015 / spec "Pass 1 sanitizer orchestration for text FileEntries"）：
      * ``content`` 改存 ``SanitizedResult.text`` —— placeholder 形式，原值不保留
      * ``sanitize_stats`` 依 kind 聚合 ``SanitizedResult.entries``；無命中時 ``{}``
      * ``FileSource(pass_="scanner", path=...)`` 讓 audit line 的 ``source`` 以
        ``{"pass": "scanner", "path": ...}`` 結構化形式落盤（下游 Trust-Layer
        inspector 靠 ``source.pass`` filter）
      * audit line 逐檔 flush（``SanitizerAuditLogger.append`` 內部以 lock 包住
        ``open(...).write(...).close()``，不跨檔 batch）

    Engine 拋例外不在此處理 —— 直接讓 caller 的 ``except Exception`` 接到，
    由 scan() 統一落 warning + quarantined_count +=1。
    """
    assert ctx.sanitizer is not None, (
        "ctx.sanitizer must be injected for Pass 1 orchestration; "
        "see scanner-sanitizer-orchestration change"
    )
    sanitized = ctx.sanitizer.sanitize(
        file_entry.content or "",
        FileSource(pass_="scanner", path=file_entry.path),
    )

    stats_map: dict[str, int] = {}
    for audit_entry in sanitized.entries:
        stats_map[audit_entry.kind] = stats_map.get(audit_entry.kind, 0) + 1

    if sanitize_audit is not None:
        for audit_entry in sanitized.entries:
            sanitize_audit.append(
                entry=audit_entry,
                pass_num=1,
                rules_version=rules_version,
                session_id=session_id,
            )

    return file_entry.model_copy(
        update={"content": sanitized.text, "sanitize_stats": stats_map}
    )


def _identify_language(
    abs_path: Path, *, kind: str
) -> tuple[str | None, str]:
    """對 abs_path 做 language identify；shebang 僅對無副檔名且 text-like 的檔生效。

    language.identify() 已經把「有副檔名但不認識」的 case 回 unknown；所以
    此包裝的主要目的是：對非 text 檔（binary/lockfile/generated）不要讀檔拿
    shebang（沒意義且浪費 I/O）—— 只走副檔名那條路。
    """
    if kind in _NO_CONTENT_KINDS:
        # 不讀 shebang；identify 以 shebang=None 呼叫，會 fallback 到副檔名 / unknown
        return identify(abs_path, shebang=None)

    # text / oversized：若無副檔名才嘗試讀 shebang（省 I/O）
    if abs_path.suffix:
        return identify(abs_path, shebang=None)

    shebang = _read_shebang_line(abs_path)
    return identify(abs_path, shebang=shebang)


def _read_shebang_line(abs_path: Path) -> str | None:
    """讀檔第一行；失敗 / 非 shebang 皆回 None。"""
    try:
        with abs_path.open("rb") as fp:
            first_line = fp.readline(256)  # shebang 通常 < 128 bytes
    except OSError:
        return None
    try:
        decoded = first_line.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return None
    stripped = decoded.rstrip("\r\n")
    return stripped if stripped.startswith("#!") else None


def _decode_full(
    abs_path: Path,
    *,
    encoding: str,
    head_decoded: str | None,
    head_len: int,
) -> tuple[str | None, int]:
    """讀整檔並以 ``encoding`` strict decode；回 ``(content, bytes_read)``。

    若檔案 ≤ head_len 就直接用 head_decoded（避免重複 I/O）。
    decode 失敗回 ``(None, bytes_read)`` —— caller 會 reclass 為 binary。
    """
    try:
        raw = abs_path.read_bytes()
    except OSError:
        return None, 0

    if len(raw) <= head_len and head_decoded is not None:
        # head 已涵蓋整檔，省一次 decode
        return head_decoded, len(raw)

    # 剝 UTF-8 BOM（detect_encoding 在 head 已做；這裡補對整檔）
    payload = raw
    if encoding == "utf-8" and payload.startswith(b"\xef\xbb\xbf"):
        payload = payload[3:]

    try:
        return payload.decode(encoding, errors="strict"), len(raw)
    except (UnicodeDecodeError, LookupError):
        return None, len(raw)


__all__ = ["scan"]
