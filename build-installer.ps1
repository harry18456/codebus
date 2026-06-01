#requires -Version 5.1
<#
.SYNOPSIS
  Build the codebus Windows installer (NSIS -setup.exe) from the repo root.

.DESCRIPTION
  The installer is produced by `tauri build`, which MUST run inside
  codebus-app/ (that is where tauri.conf.json and the tauri npm CLI live;
  `cargo build` alone cannot bundle the NSIS installer). Its
  beforeBuildCommand stages the release CLI (`cargo build -p codebus-cli
  --release`) and builds the frontend first, so this one command yields an
  installer containing BOTH the GUI (codebus-app.exe) and the CLI
  (bin/codebus.exe). This script just wraps that so you can build from the
  repo root without remembering to cd into codebus-app.

.EXAMPLE
  .\build-installer.ps1

.EXAMPLE
  .\build-installer.ps1 -Open    # also reveal the .exe in Explorer when done
#>
[CmdletBinding()]
param(
    [switch]$Open
)

$ErrorActionPreference = "Stop"

$repoRoot  = $PSScriptRoot
$appDir    = Join-Path $repoRoot "codebus-app"
$bundleDir = Join-Path $repoRoot "target\release\bundle\nsis"

Write-Host "[build-installer] running 'npm run tauri build' in $appDir" -ForegroundColor Cyan
Write-Host "[build-installer] compiles CLI + frontend + GUI and bundles the NSIS installer (a few minutes)..."

Push-Location $appDir
try {
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed (exit code $LASTEXITCODE)" }
}
finally {
    Pop-Location
}

$installer = Get-ChildItem -Path $bundleDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw "build finished but no *-setup.exe found under $bundleDir" }

Write-Host ""
Write-Host "[build-installer] DONE" -ForegroundColor Green
Write-Host ("  installer : {0}" -f $installer.FullName)
Write-Host ("  size      : {0:N1} MB" -f ($installer.Length / 1MB))
Write-Host ("  built     : {0}" -f $installer.LastWriteTime)

if ($Open) { Start-Process explorer.exe "/select,`"$($installer.FullName)`"" }
