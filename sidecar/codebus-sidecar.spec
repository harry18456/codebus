# PyInstaller spec — backs SHALL clauses in
# openspec/changes/m1-power-on/specs/app-packaging/spec.md
#   Requirement: PyInstaller onefile sidecar binary
#     Scenario: PyInstaller spec exists and builds
#     Scenario: Hidden imports declared
#
# Invoke from repo root:
#   cd sidecar && uv run pyinstaller codebus-sidecar.spec
# Produces one binary at sidecar/dist/codebus-sidecar-<target-triple>(.exe).
# The target-triple suffix is required by Tauri's `externalBin`
# convention (it auto-selects the artifact matching `rustc -Vv | host`).
# PyInstaller executes this spec with `exec`, so `__file__` is not
# defined — we resolve paths against the current working directory
# (which `pyinstaller` sets to the spec's directory).
# ruff: noqa
import subprocess
import sys
from pathlib import Path

from PyInstaller.utils.hooks import collect_submodules

_SPEC_DIR = Path.cwd()
_ENTRYPOINT = str(_SPEC_DIR / "src" / "codebus_agent" / "api" / "main.py")


def _target_triple() -> str:
    """Ask rustc for the host triple so the output file matches
    Tauri's externalBin lookup (<name>-<triple>(.exe))."""
    try:
        out = subprocess.check_output(
            ["rustc", "-Vv"], text=True, stderr=subprocess.DEVNULL
        )
    except (FileNotFoundError, subprocess.CalledProcessError) as exc:
        print(
            f"[codebus-sidecar.spec] rustc lookup failed ({exc}); "
            "falling back to un-suffixed name. Install Rust or run "
            "`rustup show` to enable Tauri externalBin integration.",
            file=sys.stderr,
        )
        return ""
    for line in out.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    return ""


_TRIPLE = _target_triple()
_BINARY_NAME = f"codebus-sidecar-{_TRIPLE}" if _TRIPLE else "codebus-sidecar"

# Hidden imports: packages / modules that PyInstaller's static analyser
# cannot see because they are resolved dynamically at runtime.
# - uvicorn.protocols.http.auto: picked by uvicorn based on installed deps
# - instructor: pulls provider adapters via entry points
# - qdrant_client: lazy-loads grpc/http transports
_HIDDEN_IMPORTS = [
    "uvicorn.protocols.http.auto",
    *collect_submodules("instructor"),
    *collect_submodules("qdrant_client"),
]


a = Analysis(
    [_ENTRYPOINT],
    pathex=[str(_SPEC_DIR / "src")],
    binaries=[],
    datas=[],
    hiddenimports=_HIDDEN_IMPORTS,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name=_BINARY_NAME,
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
