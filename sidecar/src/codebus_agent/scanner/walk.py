"""File tree traversal with built-in always-ignore + `.gitignore` stacking.

Backs openspec/changes/scanner-skeleton/specs/folder-scanner/spec.md
  Requirement: File tree traversal with gitignore stacking
  Requirement: Symlink handling without following
  Requirement: Sandbox boundary enforcement
Implements tasks.md Task 5.2 (TDD green phase for `tests/scanner/test_walk.py`).

設計守則：

1. **Built-in always-ignore** 先於任何 `.gitignore` 規則套用，且實作為 path prefix
   （目錄）或 exact filename（OS junk）match。命中即不 rglob 子樹。
2. **`.gitignore` 階層疊加** 走 `pathspec.PathSpec.from_lines("gitwildmatch", ...)`，
   effective spec = 從 workspace_root 一路 join 下來的所有 spec line 串連（同
   git 本身的 semantics：後面的 rule 可覆蓋前面的 include/exclude，negation
   `!pattern` 亦然）。
3. **Symlink 不跟隨**：`Path.is_symlink()` 判別；目錄 symlink 不 descend，檔案
   symlink 不 yield `FileEntry`。僅吐 `Symlink` 記錄，`resolved_in_workspace`
   依 `resolve(strict=False)` 與 `workspace_root` 的包含關係判定。
4. **Sandbox boundary**：每個 entry yield 前先 `ensure_in_workspace(p, ctx)`；
   `PathEscapeError` 時 skip + 追加 human-readable warning（透過 keyword-only
   `warnings` 參數傳出 —— tasks.md signature 保留 `Iterator[FileEntry | Symlink]`
   純粹性，warning 走 side channel）。
5. **POSIX path**：所有 yield 出的 `FileEntry.path` / `Symlink.path` 均以
   `Path.relative_to(...).as_posix()` 正規化，確保 Windows 也吐 `/` 分隔字串。
6. **kind 欄位**：`FileEntry` 必填 `kind`；walk 呼叫 `classify()` 以 head bytes
   決定（與 classify.py 契約一致：只讀前 8 KB）。encoding / language / content
   不在 walk 階段填，留給 service pipeline。
"""
from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path

import pathspec

from codebus_agent.sandbox import PathEscapeError, ToolContext, ensure_in_workspace
from codebus_agent.scanner.classify import classify
from codebus_agent.scanner.models import FileEntry, Symlink

# ---------------------------------------------------------------------------
# Built-in always-ignore 清單（spec 第 45 行列表，順序同 spec）
# ---------------------------------------------------------------------------

# 目錄前綴 —— 命中則整個 subtree 不 descend
_BUILTIN_IGNORE_DIRS: frozenset[str] = frozenset(
    {
        ".git",
        ".hg",
        ".svn",
        "node_modules",
        ".venv",
        "venv",
        "__pycache__",
        ".mypy_cache",
        ".pytest_cache",
        "dist",
        "build",
        "target",
        "out",
    }
)

# 檔案名稱 —— OS 產生的垃圾檔，root 與子目錄一視同仁 skip
_BUILTIN_IGNORE_FILES: frozenset[str] = frozenset(
    {
        ".DS_Store",
        "Thumbs.db",
    }
)

# classify() 需要 head bytes 做 NUL sniff；spec 固定 8 KB
_HEAD_BYTES_SIZE = 8 * 1024


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def walk(
    workspace_root: Path,
    ctx: ToolContext,
    *,
    warnings: list[str] | None = None,
) -> Iterator[FileEntry | Symlink]:
    """遞迴掃描 ``workspace_root``，依序 yield ``FileEntry`` / ``Symlink``。

    參數
    ----
    workspace_root:
        掃描起點；必須是已存在的目錄（caller 端負責檢查，walk 不 stat）。
    ctx:
        ``ToolContext``，walk 透過 ``ensure_in_workspace`` 把每個候選路徑 clamp
        回 workspace 內。
    warnings:
        可選的 warning accumulator。sandbox 違規（`PathEscapeError`）或 stat
        錯誤會往這裡追加人類可讀訊息。預設 ``None`` 代表 caller 不關心 warning。

    Yields
    ------
    FileEntry
        對每個通過 ignore + sandbox 的檔案產出；``kind`` 已由 ``classify`` 決定，
        但 ``language`` / ``encoding`` / ``content`` 留給下游 pipeline。
    Symlink
        對每個 symlink 產出；target 為 link 上的字面值，`resolved_in_workspace`
        依 ``resolve(strict=False)`` 後的 parent 關係判定。不 descend 進 symlink。
    """
    # ignore spec accumulator：從 root 起，每進一層目錄就 join 子 .gitignore
    root_spec = _load_gitignore_spec(workspace_root)
    yield from _walk_dir(
        current=workspace_root,
        workspace_root=workspace_root,
        ctx=ctx,
        parent_spec=root_spec,
        warnings=warnings,
    )


# ---------------------------------------------------------------------------
# Internal — 遞迴 driver
# ---------------------------------------------------------------------------


def _walk_dir(
    *,
    current: Path,
    workspace_root: Path,
    ctx: ToolContext,
    parent_spec: pathspec.PathSpec,
    warnings: list[str] | None,
) -> Iterator[FileEntry | Symlink]:
    """遞迴下探 ``current``，吐 FileEntry / Symlink。

    規則順序：symlink 判別 → built-in ignore → ensure_in_workspace → .gitignore
    → directory（遞迴）vs file（classify + yield）。
    """
    # current 目錄自己的 .gitignore 併入 parent spec（本層生效）
    effective_spec = _stack_gitignore(parent_spec, current)

    try:
        children = sorted(current.iterdir(), key=lambda p: p.name)
    except OSError as exc:
        _warn(warnings, f"cannot list directory {current}: {exc}")
        return

    for child in children:
        name = child.name

        # 1. Symlink 先判（不 follow、不讀、不 descend）
        #    os.DirEntry 或 Path.is_symlink() 都 OK；Path 在 Windows 行為一致
        if child.is_symlink():
            yield from _handle_symlink(
                link=child,
                workspace_root=workspace_root,
                ctx=ctx,
                warnings=warnings,
            )
            continue

        # 2. Built-in always-ignore（目錄名 / 檔案名 exact match）
        if child.is_dir():
            if name in _BUILTIN_IGNORE_DIRS:
                continue
        else:
            if name in _BUILTIN_IGNORE_FILES:
                continue

        # 3. Sandbox boundary：ensure_in_workspace 過不了就 skip + warn
        try:
            resolved = ensure_in_workspace(child, ctx)
        except PathEscapeError as exc:
            _warn(warnings, f"sandbox rejected {child}: {exc}")
            continue

        # 4. .gitignore（stacked spec）—— 目錄 match 整個 subtree 不 descend
        rel = _posix_relative(resolved, workspace_root)
        if child.is_dir():
            if _dir_matches_ignore(effective_spec, rel):
                continue
            # 遞迴下探，帶入本層 effective_spec
            yield from _walk_dir(
                current=child,
                workspace_root=workspace_root,
                ctx=ctx,
                parent_spec=effective_spec,
                warnings=warnings,
            )
            continue

        # 5. 檔案：先過 .gitignore
        if effective_spec.match_file(rel):
            continue

        # 6. 過關 —— 讀 head + classify，組成 FileEntry yield 出去
        entry = _build_file_entry(path=resolved, rel_posix=rel, warnings=warnings)
        if entry is not None:
            yield entry


# ---------------------------------------------------------------------------
# Internal — Symlink 處理
# ---------------------------------------------------------------------------


def _handle_symlink(
    *,
    link: Path,
    workspace_root: Path,
    ctx: ToolContext,
    warnings: list[str] | None,
) -> Iterator[Symlink]:
    """Symlink 永不跟隨；只輸出單筆 `Symlink` 記錄。"""
    # path：相對 workspace_root 的 POSIX 表示
    try:
        rel = _posix_relative(link, workspace_root)
    except ValueError as exc:
        _warn(warnings, f"symlink {link} not relative to workspace: {exc}")
        return

    # target：link 上的字面值（readlink），不 resolve
    try:
        target_raw = link.readlink()
    except OSError as exc:
        _warn(warnings, f"cannot readlink {link}: {exc}")
        return
    target_str = str(target_raw).replace("\\", "/")

    # resolved_in_workspace：target resolve 後是否仍在 workspace 內
    try:
        # strict=False 讓 dangling symlink 也能判定
        resolved_target = link.resolve(strict=False)
    except OSError as exc:
        _warn(warnings, f"cannot resolve symlink {link}: {exc}")
        resolved_in_workspace = False
    else:
        resolved_in_workspace = _is_within_workspace(resolved_target, workspace_root)

    yield Symlink(
        path=rel,
        target=target_str,
        resolved_in_workspace=resolved_in_workspace,
    )


# ---------------------------------------------------------------------------
# Internal — FileEntry 組裝
# ---------------------------------------------------------------------------


def _build_file_entry(
    *,
    path: Path,
    rel_posix: str,
    warnings: list[str] | None,
) -> FileEntry | None:
    """讀 stat + head bytes，call classify，組成 FileEntry。

    stat / read 失敗時 skip 並 warn，保持 walk 的 generator 不中斷。
    """
    try:
        st = path.stat()
    except OSError as exc:
        _warn(warnings, f"cannot stat {path}: {exc}")
        return None

    size = st.st_size

    # 讀 head bytes —— classify 用於 NUL sniff；讀失敗就當空 bytes（classify
    # 會 fallback 到 extension / 大小判定）
    try:
        with path.open("rb") as fp:
            head = fp.read(_HEAD_BYTES_SIZE)
    except OSError as exc:
        _warn(warnings, f"cannot read head of {path}: {exc}")
        head = b""

    kind = classify(path, size=size, head_bytes=head)

    return FileEntry(
        path=rel_posix,
        size=size,
        kind=kind,
    )


# ---------------------------------------------------------------------------
# Internal — pathspec helpers
# ---------------------------------------------------------------------------


def _load_gitignore_spec(directory: Path) -> pathspec.PathSpec:
    """讀 ``directory/.gitignore``，沒有就回空 spec。"""
    gitignore = directory / ".gitignore"
    if not gitignore.is_file():
        return pathspec.PathSpec.from_lines("gitwildmatch", [])
    try:
        with gitignore.open("r", encoding="utf-8", errors="replace") as fp:
            return pathspec.PathSpec.from_lines("gitwildmatch", fp)
    except OSError:
        # 無法讀 .gitignore 就當作沒有 —— 寧可 include 過多也不 silently drop
        return pathspec.PathSpec.from_lines("gitwildmatch", [])


def _stack_gitignore(
    parent: pathspec.PathSpec, current: Path
) -> pathspec.PathSpec:
    """把 current 目錄的 .gitignore pattern 併入 parent spec。

    pathspec 沒有「疊加兩個 spec」的 public API；直接把兩邊 patterns concat
    成單一 PathSpec 即可 —— 這也是 git 自身的 semantics（後面的 rule 覆蓋
    前面的，negation 亦如此）。
    """
    current_spec = _load_gitignore_spec(current)
    # 0.x 以 `.patterns` 暴露內部 list；concat 後建新 PathSpec。
    combined_patterns = list(parent.patterns) + list(current_spec.patterns)
    return pathspec.PathSpec(combined_patterns)


def _dir_matches_ignore(spec: pathspec.PathSpec, rel_posix: str) -> bool:
    """判斷目錄是否被 ignore。

    pathspec 對目錄的 match 需要後接 `/`（例如 `build/` pattern 要對 `build/`
    而不是 `build`）。我們顯式把 rel 後加 `/` 做一次 match —— 這是 pathspec
    官方 FAQ 的建議做法。
    """
    # 空字串代表 workspace root 自己，不應該被判為 ignored
    if not rel_posix:
        return False
    if spec.match_file(rel_posix + "/"):
        return True
    # 有些 pattern 只寫 `build` 不含 `/`，也會吃到目錄
    return spec.match_file(rel_posix)


# ---------------------------------------------------------------------------
# Internal — path helpers
# ---------------------------------------------------------------------------


def _posix_relative(path: Path, workspace_root: Path) -> str:
    """回傳 path 相對 workspace_root 的 POSIX 表示（斜線永遠是 `/`）。"""
    return path.relative_to(workspace_root).as_posix()


def _is_within_workspace(candidate: Path, workspace_root: Path) -> bool:
    """candidate 是否在 workspace_root 內（包含自身）。

    ``Path.is_relative_to`` 需 Python 3.9+；兩邊都已 resolve，直接字串比即可。
    """
    try:
        candidate.relative_to(workspace_root)
        return True
    except ValueError:
        return False


def _warn(warnings: list[str] | None, message: str) -> None:
    """把 warning message append 到 caller 提供的 list；None 代表 caller 不收。"""
    if warnings is not None:
        warnings.append(message)


__all__ = ["walk"]
