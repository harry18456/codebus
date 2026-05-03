"""qdrant-auto-spawn task 1.1 PoC.

Verifies that ``subprocess.Popen`` from Python can spawn the Qdrant
binary as a child, observe ``127.0.0.1:6333`` come up, then terminate
the child cleanly. Single-shot script — not part of the production
codebase. Output gets pasted into ``docs/decisions.md`` D-027 追記.

Run:
    python sidecar/scripts/qdrant_spawn_poc.py
"""
from __future__ import annotations

import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path


def main() -> int:
    home = Path.home()
    binary = home / ".codebus" / "bin" / ("qdrant.exe" if os.name == "nt" else "qdrant")
    storage = home / ".codebus" / "kb"
    snapshots = storage / "snapshots"

    if not binary.is_file():
        print(f"[poc] binary missing: {binary}")
        return 1

    storage.mkdir(parents=True, exist_ok=True)
    snapshots.mkdir(parents=True, exist_ok=True)

    env = {
        **os.environ,
        "QDRANT__STORAGE__STORAGE_PATH": str(storage),
        "QDRANT__STORAGE__SNAPSHOTS_PATH": str(snapshots),
    }
    print(f"[poc] spawning {binary} with QDRANT__STORAGE__STORAGE_PATH={storage}")
    proc = subprocess.Popen(
        [str(binary)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        deadline = time.monotonic() + 10.0
        ready = False
        while time.monotonic() < deadline:
            try:
                with urllib.request.urlopen(
                    "http://127.0.0.1:6333/healthz", timeout=0.5
                ) as resp:
                    if 200 <= resp.status < 300:
                        ready = True
                        elapsed = time.monotonic() - (deadline - 10.0)
                        print(f"[poc] /healthz 2xx after {elapsed:.2f}s")
                        break
            except Exception:
                pass
            time.sleep(0.2)

        if not ready:
            print("[poc] /healthz did not become ready within 10s")
            return 2

        # Confirm child is still alive.
        if proc.poll() is not None:
            print(f"[poc] child unexpectedly exited with {proc.returncode}")
            return 3

        print(f"[poc] child PID {proc.pid} alive; sending terminate()")
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5.0)
                print(f"[poc] graceful terminate within 5s; rc={proc.returncode}")
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
                print(f"[poc] kill() after timeout; rc={proc.returncode}")
        else:
            print(f"[poc] child already exited rc={proc.returncode}")

    print("[poc] PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
