"""Sidecar CLI entry — spawned by the Tauri shell.

PyInstaller binds to ``codebus_agent.api.main:run`` per D-local-5.

Startup sequence (D-local-1):
  1. generate bearer token (memory-only, per D-local-2)
  2. bind an ephemeral loopback port
  3. emit the handshake JSON line to stdout
  4. start uvicorn on the pre-bound socket
"""
from __future__ import annotations

import argparse
import asyncio
import json
import logging
import logging.handlers
import os
import sys
from pathlib import Path

import uvicorn

from codebus_agent import auth, handshake, healthz, net
from codebus_agent.api import create_app
from codebus_agent.auth.audit_logger import AuthorizationAuditLogger
from codebus_agent.auth.paths import authorization_audit_path
from codebus_agent.kb import qdrant_client as _kb_qdrant
from codebus_agent.qdrant_supervisor import (
    install_signal_cleanup,
    maybe_spawn_qdrant,
    register_cleanup,
)
from codebus_agent.watchdog import supervise_parent


def _install_file_logger() -> Path:
    """Mirror all root-logger output to ~/.codebus/sidecar.log.

    Tauri's sidecar spawn pipes stdout / stderr but drops the handles
    after reading the handshake line, so Python's StreamHandler ends up
    writing to a broken pipe and the traceback from ``logger.exception``
    in ``_run_background_task`` silently disappears. Adding an explicit
    file handler at boot guarantees those tracebacks survive in
    ``~/.codebus/sidecar.log`` regardless of Tauri's pipe handling.

    Rotation: 5 MiB per file, keep last 3 — bounded disk footprint that
    still preserves enough history for a multi-iteration debug loop.
    """
    log_path = Path.home() / ".codebus" / "sidecar.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)

    handler = logging.handlers.RotatingFileHandler(
        log_path, maxBytes=5 * 1024 * 1024, backupCount=3, encoding="utf-8"
    )
    handler.setFormatter(
        logging.Formatter(
            fmt="%(asctime)s %(levelname)s %(name)s: %(message)s",
            datefmt="%Y-%m-%d %H:%M:%S",
        )
    )

    root = logging.getLogger()
    if root.level == logging.NOTSET:
        root.setLevel(logging.INFO)
    # Avoid duplicate file handlers if run() is called more than once
    # in the same process (e.g. from tests).
    has_file_handler = any(
        isinstance(h, logging.handlers.RotatingFileHandler)
        and Path(h.baseFilename).resolve() == log_path.resolve()
        for h in root.handlers
    )
    if not has_file_handler:
        root.addHandler(handler)
    return log_path


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="codebus-sidecar")
    parser.add_argument(
        "--parent-pid",
        type=int,
        default=None,
        help="pid of the parent Tauri process; sidecar exits if this pid disappears",
    )
    parser.add_argument(
        "--healthz",
        action="store_true",
        help=(
            "run the packaged self-check: probe each dependency once, "
            "emit a single JSON line to stdout, exit 0 (status=\"ok\"|\"degraded\")"
        ),
    )
    return parser.parse_args(argv)


async def _serve(
    app,
    sock,
    port: int,
    bearer: str,
    parent_pid: int | None,
    on_parent_exit=None,
) -> None:
    config = uvicorn.Config(
        app,
        host="127.0.0.1",
        port=port,
        log_config=None,
        # MUST stay False: the SSE events endpoint accepts the bearer via
        # `?bearer=<token>` query parameter (browser EventSource cannot set
        # headers), and uvicorn's access log records the full query string.
        # Locked by `sidecar-sse-bearer-query-param-fallback` change.
        access_log=False,
    )
    server = uvicorn.Server(config)

    if parent_pid is not None:
        # `qdrant-auto-spawn` Decision 3: the watchdog now runs the
        # Qdrant cleanup hook (best-effort) BEFORE os._exit so the
        # spawned child does not orphan when Tauri force-exits.
        asyncio.create_task(
            supervise_parent(parent_pid=parent_pid, on_exit=on_parent_exit)
        )

    # Bearer / port are unused inside _serve now (handshake.emit moved
    # to run() per qdrant-auto-spawn §4 ordering invariant); kept on
    # the signature so external callers do not break.
    del bearer
    await server.serve(sockets=[sock])


def run_healthz() -> int:
    """CLI self-check: probe dependencies, print one JSON line, return 0.

    Backs openspec/changes/m1-power-on/specs/app-packaging/spec.md
      Requirement: Packaged binary health check
    Exit code is intentionally 0 even when degraded — the distinction
    lives in the ``status`` field so CI can separate "binary crashed"
    from "Qdrant not running yet".
    """
    report = asyncio.run(healthz.run_self_check())
    print(json.dumps(report.to_dict(), ensure_ascii=False))
    return 0


def run(argv: list[str] | None = None) -> None:
    args = _parse_args(argv)
    if args.healthz:
        sys.exit(run_healthz())
    log_path = _install_file_logger()
    logging.getLogger(__name__).info("sidecar.log mirror installed at %s", log_path)
    bearer = auth.generate_token()
    sock, port = net.bind_ephemeral_loopback()
    # D-027 + design「Startup policy：degraded-but-alive」— thread the
    # resolved Qdrant URL through to the app factory so /healthz reflects
    # live connectivity, but never block startup on Qdrant being reachable.
    # D-032 (kb-build-production-wiring): CODEBUS_OPENAI_API_KEY env var
    # feeds the KB embedding provider. Missing → sidecar still starts;
    # POST /kb/build returns 503 KB_NOT_CONFIGURED until the env is set
    # and the sidecar restarts.
    # `auth-flow`: default AuthorizationAuditLogger factory points at
    # the App-level audit log under ~/.codebus/. A fresh logger per
    # call mirrors the per-workspace KBGrowthLogger pattern; the
    # AuthorizationAuditLogger constructor auto-mkdirs ~/.codebus/.
    audit_path = authorization_audit_path()
    app = create_app(
        bearer_token=bearer,
        qdrant_url=_kb_qdrant.resolve_url(),
        openai_api_key=os.environ.get("CODEBUS_OPENAI_API_KEY"),
        auth_audit_logger_factory=lambda: AuthorizationAuditLogger(audit_path),
    )

    # `qdrant-auto-spawn` §4 ordering invariant: handshake MUST emit
    # first so the Tauri parent unblocks ASAP, then Qdrant spawn runs
    # synchronously (0~10s budget) before asyncio.run starts uvicorn.
    # The handshake → spawn → register_cleanup → install_signal_cleanup
    # → asyncio.run order is locked by spec scenario "Spawn never
    # blocks sidecar startup" + Requirement text "after handshake
    # emit ... and BEFORE asyncio.run".
    handshake.emit(port=port, bearer=bearer)
    qdrant_proc = maybe_spawn_qdrant(parent_pid=args.parent_pid)
    cleanup_hook = register_cleanup(qdrant_proc)
    install_signal_cleanup(qdrant_proc)

    try:
        asyncio.run(
            _serve(app, sock, port, bearer, args.parent_pid, on_parent_exit=cleanup_hook)
        )
    finally:
        sock.close()


if __name__ == "__main__":  # pragma: no cover - defensive, exercised via PyInstaller
    run()
