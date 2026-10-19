<#
.SYNOPSIS
    Architecture dependency boundary verification script.
    Guarantees pure domain crates (aim-analysis, aim-coach) never depend directly or transitively
    on UI, graphics, network, or database engines.

.PARAMETER ForbiddenPatterns
    Regex patterns representing forbidden crate families.
    Matches e.g. ^bevy, ^tauri, ^wry$, ^winit$, ^reqwest, ^rusqlite$, ^libsqlite3-sys$

.PARAMETER TargetPackages
    Domain packages whose dependency trees are inspected.

.PARAMETER SelfTest
    Runs internal self-test validation asserting that violations are detected with full trace and return non-zero exit code.
#>

param(
    [string[]]$ForbiddenPatterns = @("^bevy", "^tauri", "^wry$", "^winit$", "^reqwest", "^rusqlite$", "^libsqlite3-sys$"),
    [string[]]$TargetPackages = @("aim-analysis", "aim-coach"),
    [switch]$SelfTest
)

# Ensure cargo is accessible from default location if not in PATH
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = "$env:USERPROFILE\.cargo\bin"
    if (Test-Path $cargoBin) {
        $env:PATH = "$cargoBin;$env:PATH"
    }
}

function Test-WorkspaceBoundaries {
    param(
        [psobject]$MockMetadata = $null,
        [switch]$Silent = $false
    )

    $meta = $null
    if ($MockMetadata) {
        $meta = $MockMetadata
    } else {
        $metadataJson = cargo metadata --format-version 1
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to execute 'cargo metadata'"
            return @{ Success = $false; Error = "Failed to run cargo metadata" }
        }
        $meta = $metadataJson | ConvertFrom-Json
    }

    $pkgMap = @{}
    foreach ($pkg in $meta.packages) {
        $pkgMap[$pkg.id] = $pkg.name
    }

    $nodeMap = @{}
    foreach ($node in $meta.resolve.nodes) {
        $nodeMap[$node.id] = $node.dependencies
    }

    $violations = New-Object 'System.Collections.Generic.List[string]'

    foreach ($target in $TargetPackages) {
        $rootPkg = $meta.packages | Where-Object { $_.name -eq $target }
        if (-not $rootPkg) {
            $msg = "Target package '$target' not found in workspace."
            if (-not $Silent) { Write-Error $msg }
            $violations.Add($msg)
            continue
        }

        # BFS tracking full path from root to node: Queue holds @{ Id = $id; Path = @($pkgName, ...) }
        $queue = New-Object 'System.Collections.Generic.Queue[psobject]'
        $visited = New-Object 'System.Collections.Generic.HashSet[string]'

        $rootObj = [pscustomobject]@{
            Id = $rootPkg.id
            Path = @($rootPkg.name)
        }
        $queue.Enqueue($rootObj)
        [void]$visited.Add($rootPkg.id)

        while ($queue.Count -gt 0) {
            $curr = $queue.Dequeue()
            $depIds = $nodeMap[$curr.Id]
            if ($depIds) {
                foreach ($depId in $depIds) {
                    $depName = $pkgMap[$depId]
                    if (-not $depName) { continue }

                    $currentPath = @($curr.Path) + @($depName)

                    # Check forbidden patterns against depName
                    foreach ($pattern in $ForbiddenPatterns) {
                        if ($depName -match $pattern) {
                            $pathStr = $currentPath -join " -> "
                            $violationMsg = "VIOLATION in '$target': matched forbidden pattern '$pattern' via: $pathStr"
                            $violations.Add($violationMsg)
                            if (-not $Silent) {
                                Write-Error $violationMsg
                            }
                        }
                    }

                    if (-not $visited.Contains($depId)) {
                        [void]$visited.Add($depId)
                        $nextObj = [pscustomobject]@{
                            Id = $depId
                            Path = $currentPath
                        }
                        $queue.Enqueue($nextObj)
                    }
                }
            }
        }
    }

    return @{
        Success = ($violations.Count -eq 0)
        Violations = $violations
    }
}

if ($SelfTest) {
    Write-Host "[SelfTest] Running boundary verification self-test fixtures..."

    # Construct mock metadata with deliberate architectural violation:
    # aim-analysis -> dummy-helper -> bevy_ecs
    $mockMeta = [pscustomobject]@{
        packages = @(
            [pscustomobject]@{ id = "pkg-analysis"; name = "aim-analysis" },
            [pscustomobject]@{ id = "pkg-coach"; name = "aim-coach" },
            [pscustomobject]@{ id = "pkg-helper"; name = "dummy-helper" },
            [pscustomobject]@{ id = "pkg-bevy-ecs"; name = "bevy_ecs" },
            [pscustomobject]@{ id = "pkg-tauri-runtime"; name = "tauri-runtime" }
        )
        resolve = [pscustomobject]@{
            nodes = @(
                [pscustomobject]@{ id = "pkg-analysis"; dependencies = @("pkg-helper") },
                [pscustomobject]@{ id = "pkg-helper"; dependencies = @("pkg-bevy-ecs") },
                [pscustomobject]@{ id = "pkg-coach"; dependencies = @("pkg-tauri-runtime") },
                [pscustomobject]@{ id = "pkg-bevy-ecs"; dependencies = @() },
                [pscustomobject]@{ id = "pkg-tauri-runtime"; dependencies = @() }
            )
        }
    }

    $result = Test-WorkspaceBoundaries -MockMetadata $mockMeta -Silent
    if ($result.Success) {
        Write-Error "[SelfTest FAILED] Mock violation was NOT caught!"
        exit 1
    }

    $foundBevy = $result.Violations | Where-Object { $_ -match "aim-analysis -> dummy-helper -> bevy_ecs" }
    $foundTauri = $result.Violations | Where-Object { $_ -match "aim-coach -> tauri-runtime" }

    if (-not $foundBevy -or -not $foundTauri) {
        Write-Error "[SelfTest FAILED] Expected violation paths were missing. Violations: $($result.Violations -join '; ')"
        exit 1
    }

    Write-Host "[SelfTest PASSED] Both transitive and prefix forbidden dependencies caught with exact path trace:" -ForegroundColor Green
    foreach ($v in $result.Violations) {
        Write-Host "   OK: $v" -ForegroundColor Cyan
    }
    exit 0
}

Write-Host "Checking architecture dependency boundaries for: $($TargetPackages -join ', ')"
$res = Test-WorkspaceBoundaries
if (-not $res.Success) {
    Write-Host "Architecture boundary check FAILED with $($res.Violations.Count) violation(s)." -ForegroundColor Red
    exit 1
}

Write-Host "Architecture boundary check PASSED: pure domain crates comply with isolation rules." -ForegroundColor Green
exit 0

