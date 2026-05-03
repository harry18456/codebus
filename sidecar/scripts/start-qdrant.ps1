# start-qdrant.ps1 — launch a Qdrant standalone binary locally on Windows.
# Backs openspec/changes/m1-power-on/specs/qdrant-client/spec.md
#   Requirement: Local Qdrant launch recipe
#
# Role since `qdrant-auto-spawn` (D-027 follow-up):
#   * The sidecar auto-spawns Qdrant on startup, so end users running
#     `cargo tauri dev` do NOT need to invoke this script directly.
#   * This script is preserved as a dev tool and a degraded-mode
#     fallback: run it manually when the auto-spawn path warns about a
#     missing binary, or when debugging Qdrant in isolation from the
#     sidecar process tree.
#
# Resolves the binary via, in order:
#   1. $env:CODEBUS_QDRANT_BIN (absolute path to qdrant.exe)
#   2. $HOME\.codebus\bin\qdrant.exe
# Persistent storage defaults to $HOME\.codebus\kb\, overridable via
# $env:CODEBUS_QDRANT_STORAGE.  See docs/decisions.md D-027.

$ErrorActionPreference = 'Stop'

$DownloadUrl = 'https://github.com/qdrant/qdrant/releases'
$DefaultBin = Join-Path $HOME '.codebus\bin\qdrant.exe'
$DefaultStorage = Join-Path $HOME '.codebus\kb'

$binPath = if ($env:CODEBUS_QDRANT_BIN) { $env:CODEBUS_QDRANT_BIN } else { $DefaultBin }
$storagePath = if ($env:CODEBUS_QDRANT_STORAGE) { $env:CODEBUS_QDRANT_STORAGE } else { $DefaultStorage }
$snapshotsPath = Join-Path $storagePath 'snapshots'

if (-not (Test-Path -LiteralPath $binPath -PathType Leaf)) {
    $msg = @"
[start-qdrant] Qdrant binary not found at: $binPath

Download the standalone binary for Windows from:
  $DownloadUrl

Then drop the extracted file at:
  $DefaultBin
or set `$env:CODEBUS_QDRANT_BIN to its absolute path.
"@
    [Console]::Error.WriteLine($msg)
    exit 1
}

New-Item -ItemType Directory -Force -Path $storagePath | Out-Null
New-Item -ItemType Directory -Force -Path $snapshotsPath | Out-Null

# Qdrant configures storage via env vars, not CLI flags (as of v1.17).
# Set both storage + snapshots paths so nothing pollutes $PWD.
$env:QDRANT__STORAGE__STORAGE_PATH = $storagePath
$env:QDRANT__STORAGE__SNAPSHOTS_PATH = $snapshotsPath
& $binPath @args
exit $LASTEXITCODE
