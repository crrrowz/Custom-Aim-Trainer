# Architecture boundary check
# Enforces that pure domain crates (aim-analysis and aim-coach) never depend on bevy, tauri, reqwest, or rusqlite.

param(
    [string[]]$ForbiddenCrates = @("bevy", "tauri", "reqwest", "rusqlite"),
    [string[]]$TargetPackages = @("aim-analysis", "aim-coach")
)

Write-Host "Checking architecture dependency boundaries for: $($TargetPackages -join ', ')"

$metadataJson = cargo metadata --format-version 1
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to execute 'cargo metadata'"
    exit 1
}

$meta = $metadataJson | ConvertFrom-Json
$pkgMap = @{}
foreach ($pkg in $meta.packages) {
    $pkgMap[$pkg.id] = $pkg.name
}

$nodeMap = @{}
foreach ($node in $meta.resolve.nodes) {
    $nodeMap[$node.id] = $node.dependencies
}

$failed = $false

foreach ($target in $TargetPackages) {
    $rootPkg = $meta.packages | Where-Object { $_.name -eq $target }
    if (-not $rootPkg) {
        Write-Error "Could not find package '$target' in workspace metadata."
        $failed = $true
        continue
    }

    $visited = New-Object 'System.Collections.Generic.HashSet[string]'
    $queue = New-Object 'System.Collections.Generic.Queue[string]'
    $queue.Enqueue($rootPkg.id)
    [void]$visited.Add($rootPkg.id)

    $allDeps = New-Object 'System.Collections.Generic.List[string]'

    while ($queue.Count -gt 0) {
        $currId = $queue.Dequeue()
        $depIds = $nodeMap[$currId]
        if ($depIds) {
            foreach ($depId in $depIds) {
                if (-not $visited.Contains($depId)) {
                    [void]$visited.Add($depId)
                    $queue.Enqueue($depId)
                    $depName = $pkgMap[$depId]
                    if ($depName) {
                        $allDeps.Add($depName)
                    }
                }
            }
        }
    }

    foreach ($forbidden in $ForbiddenCrates) {
        if ($allDeps.Contains($forbidden)) {
            Write-Error "ARCHITECTURE BOUNDARY VIOLATION: '$target' depends directly or transitively on forbidden crate '$forbidden'!"
            $failed = $true
        }
    }
}

if ($failed) {
    Write-Host "Boundary check FAILED." -ForegroundColor Red
    exit 1
}

Write-Host "Boundary check PASSED: pure domain crates have no forbidden dependencies." -ForegroundColor Green
exit 0
