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
import os

import uvicorn

from codebus_agent import auth, handshake, net
from codebus_agent.api import create_app
from codebus_agent.watchdog import watch_parent


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="codebus-sidecar")
    parser.add_argument(
        "--parent-pid",
        type=int,
        default=None,
        help="pid of the parent Tauri process; sidecar exits if this pid disappears",
    )
    # --healthz lands in 8.5.
    return parser.parse_args(argv)


async def _serve(app, sock, port: int, bearer: str, parent_pid: int | None) -> None:
    handshake.emit(port=port, bearer=bearer)
    config = uvicorn.Config(
        app,
        host="127.0.0.1",
        port=port,
        log_config=None,
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


def run(argv: list[str] | None = None) -> None:
    args = _parse_args(argv)
    bearer = auth.generate_token()
    sock, port = net.bind_ephemeral_loopback()
    app = create_app(bearer_token=bearer)
    try:
        asyncio.run(_serve(app, sock, port, bearer, args.parent_pid))
    finally:
        sock.close()


if __name__ == "__main__":  # pragma: no cover - defensive, exercised via PyInstaller
    run()
