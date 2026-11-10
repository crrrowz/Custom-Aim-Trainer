# Builds the game and opens the AimForge desktop app.
# Usage: powershell -ExecutionPolicy Bypass -File scripts\run-desktop.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = "D:\cargo-target\aimforge" }
# Leave two cores free so the PC stays usable during a build.
$env:CARGO_BUILD_JOBS = [Math]::Max(1, [Environment]::ProcessorCount - 2)
$env:AIMFORGE_ROOT = $root
$env:AIMFORGE_TRAINER = Join-Path $env:CARGO_TARGET_DIR "play\trainer.exe"

Set-Location $root
cargo build --profile play -p trainer
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ui = Join-Path $root "apps\desktop\ui"
if (-not (Test-Path (Join-Path $ui "node_modules"))) {
    npm --prefix $ui install --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Set-Location (Join-Path $root "apps\desktop\src-tauri")
& (Join-Path $ui "node_modules\.bin\tauri.cmd") dev
exit $LASTEXITCODE
