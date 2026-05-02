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
import os
import sys

import uvicorn

from codebus_agent import auth, handshake, healthz, net
from codebus_agent.api import create_app
from codebus_agent.auth.audit_logger import AuthorizationAuditLogger
from codebus_agent.auth.paths import authorization_audit_path
from codebus_agent.kb import qdrant_client as _kb_qdrant
from codebus_agent.watchdog import watch_parent


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


async def _serve(app, sock, port: int, bearer: str, parent_pid: int | None) -> None:
    handshake.emit(port=port, bearer=bearer)
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
        async def _supervise() -> None:
            await watch_parent(parent_pid=parent_pid)
            # Parent has vanished — force exit rather than rely on
            # uvicorn's graceful shutdown (which can block on open
            # connections and miss the 5-second SHALL budget).
            os._exit(0)

        asyncio.create_task(_supervise())

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
    try:
        asyncio.run(_serve(app, sock, port, bearer, args.parent_pid))
    finally:
        sock.close()


if __name__ == "__main__":  # pragma: no cover - defensive, exercised via PyInstaller
    run()
