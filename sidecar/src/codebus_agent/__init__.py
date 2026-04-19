"""codebus-agent package root.

The uv-declared script ``codebus-agent`` dispatches to :func:`main` which
forwards to :func:`codebus_agent.api.main.run` (the same symbol
PyInstaller binds to, per D-local-5).
"""
from __future__ import annotations


def main() -> None:
    from codebus_agent.api.main import run
    run()
