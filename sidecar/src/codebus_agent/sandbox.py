"""Tool sandbox — ToolContext schema + path-escape guard + audit log.

Backs openspec/changes/m1-power-on/specs/tool-sandbox/spec.md
  Requirement: ToolContext carries workspace type discriminator
  Requirement: ensure_in_workspace blocks path escape
and openspec/changes/sanitizer-safety-chain/specs/tool-sandbox/spec.md
  Requirement: ToolSandbox appends every invocation to tool_audit.jsonl
  Requirement: Tools declare their auditable field whitelist
  Requirement: Schema version on every tool audit line
and openspec/changes/scanner-sanitizer-orchestration/specs/folder-scanner/spec.md
  Requirement: Pass 1 sanitizer orchestration for text FileEntries
    (ToolContext.sanitizer carries the shared SanitizerEngine)

The M1 ToolContext is the stripped-down skeleton — just the fields we
actually use here plus the discriminator (D-002 day-1 invariant).
``sanitizer`` landed with scanner-sanitizer-orchestration so scanner
Pass 1 can reuse a single engine instance across a scan.  Future fields
(kb, audit_log, usage_tracker, ...) will be added in subsequent
milestones; adding them later is schema-compatible because every future
field is either required-with-default or optional.
"""
from __future__ import annotations

import json
import os
import re
import sys
import threading
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import TYPE_CHECKING, Any, Literal, Protocol, runtime_checkable

from pydantic import BaseModel, ConfigDict, field_validator

from codebus_agent.sanitizer import SanitizerEngine

if TYPE_CHECKING:
    from codebus_agent.kb.knowledge_base import KnowledgeBase
    from codebus_agent.providers.usage_tracker import UsageTracker


class PathEscapeError(ValueError):
    """Raised when a requested path resolves outside the workspace root."""


class ToolContext(BaseModel):
    """Authoritative per-run context handed to every sandboxed tool.

    ``frozen=True`` guarantees tools cannot silently relocate the
    workspace mid-run by mutating the context.  Per D-002 the
    ``workspace_type`` discriminator MUST be present day 1.

    Optional dependency slots (``sanitizer`` / ``kb`` / ``usage_tracker``)
    all default to ``None`` so existing sandbox / red-team fixtures keep
    constructing ``ToolContext`` without them; each real code path that
    needs a dep MUST inject explicitly:
      - ``sanitizer``: scanner Pass 1 + explorer-tools-p0 read_file
      - ``kb``: explorer-tools-p0 search (KB path; falls back to grep
        when ``None``)
      - ``usage_tracker``: future tools that themselves consume LLM budget
    """

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True)

    workspace_root: Path
    workspace_type: Literal["folder", "topic"]
    workspace_id: str = ""
    session_id: str = ""
    sanitizer: SanitizerEngine | None = None
    # Type-checker hint: ``KnowledgeBase | None``. Kept as ``Any`` at the
    # Pydantic level to sidestep forward-ref rebuild gymnastics —
    # ``arbitrary_types_allowed=True`` still lets callers pass a real
    # ``KnowledgeBase`` instance, and the ``TYPE_CHECKING`` import above
    # keeps IDE / mypy navigation honest.
    kb: Any = None
    # Type-checker hint: ``UsageTracker | None``.
    usage_tracker: Any = None

    @field_validator("workspace_root", mode="after")
    @classmethod
    def _resolve_root(cls, v: Path) -> Path:
        return v.resolve(strict=False)


_LONG_PATH_PREFIX = "\\\\?\\"
_LONG_PATH_UNC_PREFIX = "\\\\?\\UNC\\"


def _strip_long_path_prefix(s: str) -> str:
    """Strip the Windows ``\\\\?\\`` / ``\\\\?\\UNC\\`` prefix so we can
    compare prefixed and non-prefixed paths structurally.

    ``Path.resolve`` does NOT normalise this prefix away — without
    stripping, a long-path-prefixed in-workspace path would fail the
    ``startswith`` check against the bare workspace root.
    """
    if s.startswith(_LONG_PATH_UNC_PREFIX):
        return "\\\\" + s[len(_LONG_PATH_UNC_PREFIX):]
    if s.startswith(_LONG_PATH_PREFIX):
        return s[len(_LONG_PATH_PREFIX):]
    return s


_WIN_SEP_RE = re.compile(r"[\\/]")


def _strip_trailing_dots_spaces_per_component(requested: str) -> str:
    """Strip trailing dots and spaces from each path component.

    Windows kernel does this at filesystem-call time — ``CreateFile``
    on ``foo.txt.`` opens ``foo.txt`` — but Python's ``pathlib`` does
    not replicate the behavior at ``resolve(strict=False)`` time.
    Without this preprocessing, an attack like ``.. /secret`` slips
    through comparison as a literal ``.. `` component even though the
    real filesystem would treat it as ``..`` and escape the workspace.
    """
    if sys.platform != "win32":
        return requested

    def _strip_component(comp: str) -> str:
        # Bare traversal operators survive unchanged.
        if comp in ("", ".", ".."):
            return comp
        stripped = comp.rstrip(". ")
        if stripped:
            return stripped
        # Pure dots/spaces component (e.g. "..." or ".. ") — Windows kernel
        # collapses trailing dots+spaces, so "..." and ".. " both behave like
        # ".." at filesystem-call time.  Canonicalize here so the resolver
        # sees the traversal and rejects the escape.
        dots = comp.count(".")
        if dots >= 2:
            return ".."
        if dots == 1:
            return "."
        return comp

    # Preserve the original separators by splitting/joining via regex.
    tokens: list[str] = []
    pos = 0
    for m in _WIN_SEP_RE.finditer(requested):
        tokens.append(_strip_component(requested[pos:m.start()]))
        tokens.append(m.group(0))
        pos = m.end()
    tokens.append(_strip_component(requested[pos:]))
    return "".join(tokens)


def _normalize(p: Path) -> str:
    """Return a normcase+normpath string for case-insensitive comparison.

    On Windows ``normcase`` lowercases ASCII and flips forward slashes to
    backslashes, which covers the case-only attack variants.  On POSIX it
    is a no-op, so behavior on Unix is unchanged.
    """
    return os.path.normcase(os.path.normpath(_strip_long_path_prefix(str(p))))


def _is_within(candidate: Path, root: Path) -> bool:
    c = _normalize(candidate)
    r = _normalize(root)
    if c == r:
        return True
    return c.startswith(r + os.sep)


def ensure_in_workspace(requested: str | os.PathLike[str], ctx: ToolContext) -> Path:
    """Resolve ``requested`` and assert it is inside ``ctx.workspace_root``.

    Per D-local-3 we ``resolve(strict=False)`` first — this follows
    symlinks (closing the symlink-escape vector) and normalizes Windows
    long-path prefixes (``\\\\?\\``) and UNC paths.  Trailing dots /
    spaces collapse through ``normpath`` at compare-time.

    Returns the resolved absolute Path on success; raises
    :class:`PathEscapeError` otherwise.  Never returns a path outside
    the workspace.
    """
    root = ctx.workspace_root  # already resolved by validator
    cleaned = _strip_trailing_dots_spaces_per_component(str(requested))
    p = Path(cleaned)
    candidate = p if p.is_absolute() else (root / p)
    resolved = candidate.resolve(strict=False)

    if not _is_within(resolved, root):
        raise PathEscapeError(
            f"Path {str(requested)!r} resolves to {resolved} which is outside "
            f"workspace {root}"
        )
    # Strip the \\?\ prefix from the returned path so downstream tool
    # code gets the canonical in-workspace form regardless of how the
    # caller spelled the input.
    stripped = _strip_long_path_prefix(str(resolved))
    return Path(stripped) if stripped != str(resolved) else resolved


_TOOL_AUDIT_SCHEMA_VERSION: Literal[1] = 1

_DENIAL_REASON_LITERALS = (
    "path_escape",
    "symlink_outside",
    "unc_path",
    "long_path_prefix_invalid",
    "case_variant",
    "trailing_whitespace",
)


@runtime_checkable
class SandboxTool(Protocol):
    """Structural contract every registrable tool satisfies.

    Tools MUST declare ``audit_fields`` so `ToolSandbox` can filter the
    args dict down to a safe-to-persist summary before writing the
    audit line (see Requirement "Tools declare their auditable field
    whitelist"). The optional ``path_args`` names arguments that must
    pass `ensure_in_workspace` before the body runs.
    """

    name: str
    audit_fields: list[str]

    def run(self, args: dict[str, Any], ctx: ToolContext) -> Any: ...


def _classify_denial(requested: str) -> str:
    """Map a raw user-supplied path string to one of the closed-set
    denial reasons required by the tool-sandbox spec.

    We bias toward the most specific reason we can prove from the
    input alone — symlink detection would require a filesystem probe
    the caller never granted, so "path_escape" is the conservative
    default for any generic escape.
    """
    if requested.endswith((" ", ".")):
        return "trailing_whitespace"
    if requested.startswith("\\\\?\\"):
        return "long_path_prefix_invalid"
    if requested.startswith("\\\\"):
        return "unc_path"
    return "path_escape"


class ToolSandbox:
    """Registrar + dispatcher that writes one audit line per invocation.

    The sandbox is the single choke point for tool execution: it
    validates path-like arguments via `ensure_in_workspace` before the
    tool body runs, serializes audit writes via a process-local lock,
    and records both allowed and denied invocations. Inner tools are
    plain classes satisfying the `SandboxTool` Protocol.
    """

    def __init__(self, *, audit_log_path: Path | str) -> None:
        self._audit_path = Path(audit_log_path)
        self._audit_path.parent.mkdir(parents=True, exist_ok=True)
        self._tools: dict[str, SandboxTool] = {}
        self._lock = threading.Lock()

    def register(self, tool: SandboxTool) -> None:
        audit_fields = getattr(tool, "audit_fields", None)
        if audit_fields is None:
            raise ValueError(
                f"tool {type(tool).__name__!r} is missing `audit_fields`; "
                f"every registered tool MUST declare its audit whitelist "
                f"(see sanitizer-safety-chain tool-sandbox spec)."
            )
        if not isinstance(audit_fields, list) or not all(
            isinstance(f, str) for f in audit_fields
        ):
            raise ValueError(
                f"tool {type(tool).__name__!r}.audit_fields must be a list[str]; "
                f"got {audit_fields!r}."
            )
        name = getattr(tool, "name", None)
        if not isinstance(name, str) or not name:
            raise ValueError(
                f"tool {type(tool).__name__!r} must expose a non-empty "
                f"`name: str` attribute."
            )
        self._tools[name] = tool

    def invoke(
        self, tool_name: str, args: dict[str, Any], ctx: ToolContext
    ) -> Any:
        try:
            tool = self._tools[tool_name]
        except KeyError as exc:
            raise KeyError(
                f"no tool registered under {tool_name!r}; "
                f"known tools: {sorted(self._tools)}"
            ) from exc

        args_summary = {k: args[k] for k in tool.audit_fields if k in args}
        path_args: list[str] = list(getattr(tool, "path_args", []))
        resolved_path: Path | None = None

        for arg_name in path_args:
            if arg_name not in args:
                continue
            raw = str(args[arg_name])
            try:
                resolved_path = ensure_in_workspace(raw, ctx)
            except PathEscapeError:
                self._append_audit(
                    tool_name=tool_name,
                    args_summary=args_summary,
                    resolved_path=None,
                    allowed=False,
                    denial_reason=_classify_denial(raw),
                    ctx=ctx,
                )
                raise

        result = tool.run(args, ctx)
        self._append_audit(
            tool_name=tool_name,
            args_summary=args_summary,
            resolved_path=str(resolved_path) if resolved_path is not None else None,
            allowed=True,
            denial_reason=None,
            ctx=ctx,
        )
        return result

    def _append_audit(
        self,
        *,
        tool_name: str,
        args_summary: dict[str, Any],
        resolved_path: str | None,
        allowed: bool,
        denial_reason: str | None,
        ctx: ToolContext,
    ) -> None:
        append_tool_audit_line(
            audit_path=self._audit_path,
            lock=self._lock,
            tool_name=tool_name,
            args_summary=args_summary,
            resolved_path=resolved_path,
            allowed=allowed,
            denial_reason=denial_reason,
            ctx=ctx,
        )


# Module-level process-wide lock so async tool dispatchers (FolderTools)
# and the sync ToolSandbox both serialize writes through one critical
# section. Spec `ToolSandbox appends every invocation to tool_audit.jsonl`
# mandates one JSONL line per invocation with no partial writes.
_MODULE_AUDIT_LOCK = threading.Lock()


def append_tool_audit_line(
    *,
    audit_path: Path,
    lock: threading.Lock | None,
    tool_name: str,
    args_summary: dict[str, Any],
    resolved_path: str | None,
    allowed: bool,
    denial_reason: str | None,
    ctx: ToolContext,
) -> None:
    """Append one `tool_audit.jsonl` line in the schema pinned by
    openspec/specs/tool-sandbox/spec.md `ToolSandbox appends every
    invocation to tool_audit.jsonl`.

    Exposed at module scope so async FolderTools (`codebus-agent-p0`)
    can share the same writer as the sync ToolSandbox — keeping the
    schema, lock discipline, and classification code paths in one place.
    """
    line = {
        "ts": datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
        "schema_version": _TOOL_AUDIT_SCHEMA_VERSION,
        "workspace_type": ctx.workspace_type,
        "tool_name": tool_name,
        "args_summary": args_summary,
        "resolved_path": resolved_path,
        "allowed": allowed,
        "denial_reason": denial_reason,
        "session_id": ctx.session_id or str(uuid.uuid4()),
    }
    payload = json.dumps(line, ensure_ascii=False, default=str) + "\n"
    audit_path.parent.mkdir(parents=True, exist_ok=True)
    chosen_lock = lock if lock is not None else _MODULE_AUDIT_LOCK
    with chosen_lock:
        with audit_path.open("a", encoding="utf-8") as fp:
            fp.write(payload)
