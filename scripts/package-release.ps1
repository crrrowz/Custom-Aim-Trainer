# Builds the Windows release of AimForge: an installer and a portable zip, both in dist\.
# Usage: powershell -ExecutionPolicy Bypass -File scripts\package-release.ps1
#
# The installer (NSIS) and the zip hold the same three things side by side: the desktop app,
# trainer.exe, and the content\ folder the trainer reads its scenarios from.

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $root "target" }
$target = $env:CARGO_TARGET_DIR
$desktop = Join-Path $root "apps\desktop\src-tauri"
$ui = Join-Path $root "apps\desktop\ui"
$version = (Get-Content (Join-Path $desktop "tauri.conf.json") -Raw | ConvertFrom-Json).version
$dist = Join-Path $root "dist"

function Invoke-Checked([scriptblock]$step) {
    & $step
    if ($LASTEXITCODE -ne 0) { throw "Step failed with exit code $LASTEXITCODE" }
}

Set-Location $root
Write-Host "Building the trainer $version"
Invoke-Checked { cargo build --release -p trainer }
$stage = Join-Path $desktop "stage"
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $target "release\trainer.exe") (Join-Path $stage "trainer.exe") -Force

if (-not (Test-Path (Join-Path $ui "node_modules"))) {
    Invoke-Checked { npm --prefix $ui ci --no-audit --no-fund }
}

Write-Host "Building the desktop app and the installer"
Set-Location $desktop
Invoke-Checked { & (Join-Path $ui "node_modules\.bin\tauri.cmd") build --config tauri.release.conf.json }

New-Item -ItemType Directory -Force $dist | Out-Null
$setup = Get-ChildItem (Join-Path $target "release\bundle\nsis") -Filter "*_$version*-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "The installer was not found under $target\release\bundle\nsis" }
$setupOut = Join-Path $dist "AimForge-$version-windows-x64-setup.exe"
Copy-Item $setup.FullName $setupOut -Force

$portable = Join-Path $dist "AimForge-$version"
if (Test-Path $portable) { Remove-Item -Recurse -Force $portable }
New-Item -ItemType Directory -Force $portable | Out-Null
Copy-Item (Join-Path $target "release\aimforge-desktop.exe") (Join-Path $portable "AimForge.exe")
Copy-Item (Join-Path $stage "trainer.exe") $portable
Copy-Item (Join-Path $root "content") (Join-Path $portable "content") -Recurse
Copy-Item (Join-Path $root "LICENSE-MIT"), (Join-Path $root "LICENSE-APACHE") $portable
$zip = Join-Path $dist "AimForge-$version-windows-x64-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $portable -DestinationPath $zip

Get-ChildItem $setupOut, $zip | ForEach-Object {
    $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()
    "$hash  $($_.Name)"
} | Set-Content -Encoding ascii (Join-Path $dist "SHA256SUMS.txt")

Write-Host "Done:"
Get-ChildItem $dist -File | ForEach-Object { "  {0}  ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB) }
