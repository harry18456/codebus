# start-qdrant.ps1 — launch a Qdrant standalone binary locally on Windows.
# Backs openspec/changes/m1-power-on/specs/qdrant-client/spec.md
#   Requirement: Local Qdrant launch recipe
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

& $binPath --storage-path $storagePath @args
exit $LASTEXITCODE
