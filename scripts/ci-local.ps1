#!/usr/bin/env pwsh
# Local CI-equivalent check — run before `git push` to catch what
# .github/workflows/ci.yml catches, using the SAME commands (not a subset).
#
# Why this exists: running `cargo test -p <crate>` per-crate does NOT compile
# codebus-app-tauri (it only builds under `--workspace` or when targeted), so a
# tauri build-script failure (missing bin-staging/codebus.exe) can pass locally
# yet fail CI. This script runs the full workspace path so "local green / CI
# red" gaps don't happen.
#
# Mirrors .github/workflows/ci.yml — keep the two in sync if ci.yml changes
# (runner steps, clippy baseline numbers, npm scripts).
#
# Usage:  pwsh scripts/ci-local.ps1     (PowerShell 7 recommended; matches CI)
# Assumes codebus-app deps are installed (run `npm ci` in codebus-app once).

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

function Fail($msg) { Write-Host "`nFAILED: $msg" -ForegroundColor Red; exit 1 }
function Section($name) { Write-Host "`n=== $name ===" -ForegroundColor Cyan }

# 1. Stage CLI binary for the tauri bundle resource. codebus-app-tauri's build
#    script requires bin-staging/codebus.exe (tauri.conf.json bundle.resources)
#    to exist even for compile-only `cargo test`/`clippy`. Debug build is enough
#    here (tests don't bundle); release path uses codebus-app/scripts/stage-cli.mjs.
Section "Stage CLI -> codebus-app/src-tauri/bin-staging/codebus.exe"
cargo build -p codebus-cli
if ($LASTEXITCODE -ne 0) { Fail "cargo build -p codebus-cli" }
New-Item -ItemType Directory -Force "codebus-app/src-tauri/bin-staging" | Out-Null
Copy-Item "target/debug/codebus.exe" "codebus-app/src-tauri/bin-staging/codebus.exe" -Force

# 2. Rust workspace tests (compiles codebus-app-tauri too — the part per-crate misses).
Section "cargo test --workspace"
cargo test --workspace
if ($LASTEXITCODE -ne 0) { Fail "cargo test --workspace" }

# 3. Rust clippy with baseline guard (NOT -D warnings; per-package warning count
#    must stay at or below the accepted baseline).
Section "cargo clippy --workspace (baseline guard)"
$baseline = @{ "codebus-core" = 8; "codebus-cli" = 5; "codebus-app-tauri" = 6 }
# Capture stdout (the JSON) into a variable rather than redirecting to a file —
# 5.1's `>` writes UTF-16 which can trip the parse. stderr (progress) stays on
# the console; $LASTEXITCODE still reflects cargo's exit.
$clippyJson = cargo clippy --workspace --message-format=json
if ($LASTEXITCODE -ne 0) { Fail "cargo clippy --workspace (exit $LASTEXITCODE)" }
$counts = @{}; foreach ($p in $baseline.Keys) { $counts[$p] = 0 }
foreach ($line in $clippyJson) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    try { $m = $line | ConvertFrom-Json } catch { continue }
    if ($m.reason -ne "compiler-message" -or $m.message.level -ne "warning") { continue }
    $manifest = ([string]$m.manifest_path) -replace "\\", "/"
    if ($manifest.EndsWith("/codebus-core/Cargo.toml")) { $counts["codebus-core"]++ }
    elseif ($manifest.EndsWith("/codebus-cli/Cargo.toml")) { $counts["codebus-cli"]++ }
    elseif ($manifest.EndsWith("/codebus-app/src-tauri/Cargo.toml")) { $counts["codebus-app-tauri"]++ }
}
$over = $false
foreach ($p in ($baseline.Keys | Sort-Object)) {
    Write-Host ("  {0}: {1} warnings (baseline {2})" -f $p, $counts[$p], $baseline[$p])
    if ($counts[$p] -gt $baseline[$p]) { Write-Host "  ^ ABOVE BASELINE" -ForegroundColor Red; $over = $true }
}
if ($over) { Fail "clippy warnings above baseline" }

# 4. Frontend (codebus-app): vitest + tsc typecheck.
Section "npm run test + typecheck (codebus-app)"
Set-Location (Join-Path $repoRoot "codebus-app")
npm run test
if ($LASTEXITCODE -ne 0) { Set-Location $repoRoot; Fail "npm run test" }
npm run typecheck
if ($LASTEXITCODE -ne 0) { Set-Location $repoRoot; Fail "npm run typecheck" }

Set-Location $repoRoot
Write-Host "`n=== ALL LOCAL CI-EQUIVALENT CHECKS PASSED ===" -ForegroundColor Green
