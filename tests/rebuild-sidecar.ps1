# tests/rebuild-sidecar.ps1 --kill stale sidecar processes + rebuild
# PyInstaller binary + copy into Tauri's debug target so the next
# `cargo tauri dev` run picks up fresh code without manual cleanup.
#
# Usage (Windows PowerShell 5.1 or PowerShell 7):
#   powershell tests/rebuild-sidecar.ps1               # default: keep Qdrant alive
#   powershell tests/rebuild-sidecar.ps1 -KillQdrant   # also kill orphan qdrant.exe
#   powershell tests/rebuild-sidecar.ps1 -SkipBuild    # only kill processes; reuse binary
#
# Why: cargo tauri dev caches the sidecar binary at
#   tauri/src-tauri/target/debug/codebus-sidecar.exe
# Stopping `cargo tauri dev` does not always reap the spawned sidecar
# (Tauri's child kill is best-effort); orphan codebus-sidecar.exe
# processes hold the loopback port + binary lock, blocking the next
# rebuild. This script unconditionally cleans before rebuilding.

[CmdletBinding()]
param(
    [switch]$KillQdrant,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

# Anchor the script's working directory to the repo root so relative
# paths below stay correct no matter where the user invoked it from.
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $RepoRoot
Write-Host "[rebuild] repo root: $RepoRoot" -ForegroundColor Cyan

function Stop-NamedProcess {
    param(
        [Parameter(Mandatory)] [string]$Name,
        [int]$GracefulMs = 1500
    )
    $procs = Get-Process -Name $Name -ErrorAction SilentlyContinue
    if (-not $procs) {
        Write-Host "[rebuild] no $Name processes to stop" -ForegroundColor DarkGray
        return
    }
    foreach ($p in $procs) {
        Write-Host "[rebuild] stopping $Name (PID $($p.Id))..." -ForegroundColor Yellow
        try {
            $p.CloseMainWindow() | Out-Null
            if (-not $p.WaitForExit($GracefulMs)) {
                Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
                $p.WaitForExit(1500) | Out-Null
            }
        } catch {
            # Process may have already exited between Get-Process and now.
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        }
    }
}

# 1. Kill all codebus-sidecar.exe processes (always --they hold the
#    binary lock and the loopback port).
Stop-NamedProcess -Name 'codebus-sidecar'

# 2. Optionally kill Qdrant. Default is to KEEP it running so Phase 7
#    iteration loops do not pay the 2~3s Qdrant cold-start every cycle.
#    The auto-spawn supervisor's reuse-first probe means a live Qdrant
#    is reused on the next sidecar boot.
if ($KillQdrant) {
    Stop-NamedProcess -Name 'qdrant'
} else {
    $qd = Get-Process -Name 'qdrant' -ErrorAction SilentlyContinue
    if ($qd) {
        Write-Host "[rebuild] keeping qdrant.exe (PID $($qd.Id)) alive --pass -KillQdrant to terminate it" -ForegroundColor DarkGray
    }
}

# 3. Rebuild PyInstaller binary unless explicitly skipped.
if (-not $SkipBuild) {
    Write-Host "[rebuild] running uv run pyinstaller codebus-sidecar.spec --noconfirm" -ForegroundColor Cyan
    Push-Location (Join-Path $RepoRoot 'sidecar')
    try {
        & uv run pyinstaller codebus-sidecar.spec --noconfirm
        if ($LASTEXITCODE -ne 0) {
            throw "PyInstaller exited with code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "[rebuild] -SkipBuild set; reusing existing binary" -ForegroundColor DarkGray
}

# 4. Copy fresh binary to Tauri's debug target so cargo tauri dev
#    picks it up on next spawn. Tauri's externalBin copy step only
#    runs at full bundle build; in dev mode the staged copy at
#    target/debug stays stale unless we refresh it ourselves.
$src = Join-Path $RepoRoot 'sidecar/dist/codebus-sidecar-x86_64-pc-windows-msvc.exe'
$dst = Join-Path $RepoRoot 'tauri/src-tauri/target/debug/codebus-sidecar.exe'
if (-not (Test-Path -LiteralPath $src -PathType Leaf)) {
    throw "Source binary not found at $src --did pyinstaller succeed?"
}
$dstDir = Split-Path -Parent $dst
if (-not (Test-Path -LiteralPath $dstDir)) {
    New-Item -ItemType Directory -Force -Path $dstDir | Out-Null
}
Copy-Item -LiteralPath $src -Destination $dst -Force
Write-Host "[rebuild] copied binary -> $dst" -ForegroundColor Green
$mtime = (Get-Item $dst).LastWriteTime
Write-Host "[rebuild] binary mtime: $mtime" -ForegroundColor Green

# 5. Print next-step hint so the user does not need to remember the
#    manual restart command.
Write-Host ""
Write-Host "[rebuild] DONE. Next:" -ForegroundColor Green
Write-Host "  cd tauri/src-tauri" -ForegroundColor Green
Write-Host "  cargo tauri dev" -ForegroundColor Green
