param(
    [int]$WarmRuns = 3,
    [string]$CargoTargetDir = "",
    [switch]$RefreshJavaCache,
    [switch]$RefreshRustCache,
    [switch]$AllowExistingBuildProcesses,
    [switch]$AllowJavaOnCacheHit
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$testName = "local_worldgen_can_be_compared_with_java_oracle_when_enabled"
$processPattern = "^(cargo|rustc|link|qexed|qexed[-_].*|.*test)$"
$javaProcessPattern = "^(java|javaw)$"

function Get-BuildProcess {
    Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.ProcessName -match $processPattern } |
        Select-Object Id, ProcessName, CPU, Path
}

function Invoke-TimedCargoTest {
    param(
        [string]$Label
    )

    $env:QEXED_WORLDGEN_COMPARE_JAVA = "1"
    $env:QEXED_WORLDGEN_COMPARE_JAVA_SEEDS = "0"
    if ($RefreshJavaCache) {
        $env:QEXED_WORLDGEN_REFRESH_JAVA_CACHE = "1"
    } else {
        Remove-Item Env:QEXED_WORLDGEN_REFRESH_JAVA_CACHE -ErrorAction SilentlyContinue
    }
    if ($RefreshRustCache) {
        $env:QEXED_WORLDGEN_REFRESH_RUST_CACHE = "1"
    } else {
        Remove-Item Env:QEXED_WORLDGEN_REFRESH_RUST_CACHE -ErrorAction SilentlyContinue
    }
    if ($CargoTargetDir) {
        $env:CARGO_TARGET_DIR = $CargoTargetDir
    } else {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }

    $javaBefore = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match $javaProcessPattern } | Select-Object -ExpandProperty Id)
    $startedAt = Get-Date
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & cargo test -p qexed $testName -- --nocapture --test-threads=1 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $watch.Stop()
    $javaAfter = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match $javaProcessPattern } | Select-Object Id, ProcessName, Path)
    $newJavaProcesses = @($javaAfter | Where-Object { $javaBefore -notcontains $_.Id })

    $output | ForEach-Object { Write-Host $_ }
    Write-Host ("benchmark_label={0}" -f $Label)
    Write-Host ("started_at={0:o}" -f $startedAt)
    Write-Host ("elapsed_ms={0}" -f [int64]$watch.Elapsed.TotalMilliseconds)
    Write-Host ("elapsed_seconds={0:n3}" -f $watch.Elapsed.TotalSeconds)
    Write-Host ("exit_code={0}" -f $exitCode)
    Write-Host ("target_dir={0}" -f $(if ($CargoTargetDir) { $CargoTargetDir } else { Join-Path $repoRoot "target" }))
    Write-Host ("refresh_java_cache={0}" -f [bool]$RefreshJavaCache)
    Write-Host ("refresh_rust_cache={0}" -f [bool]$RefreshRustCache)
    Write-Host ("new_java_processes={0}" -f $newJavaProcesses.Count)

    if ($exitCode -ne 0) {
        throw "cargo test failed with exit code $exitCode"
    }
    if (-not $RefreshJavaCache -and -not $AllowJavaOnCacheHit -and $newJavaProcesses.Count -gt 0) {
        $newJavaProcesses | Format-Table -AutoSize
        throw "cache-hit benchmark started Java; pass -RefreshJavaCache for refresh runs or -AllowJavaOnCacheHit to only report it"
    }
}

if ($WarmRuns -lt 1) {
    throw "WarmRuns must be >= 1"
}

$existing = Get-BuildProcess
if ($existing -and -not $AllowExistingBuildProcesses) {
    Write-Host "Existing cargo/rustc/link/qexed test processes detected. Stop them or pass -AllowExistingBuildProcesses."
    $existing | Format-Table -AutoSize
    exit 2
}

Push-Location $repoRoot
try {
    if ($CargoTargetDir) {
        New-Item -ItemType Directory -Force -Path $CargoTargetDir | Out-Null
    }

    Invoke-TimedCargoTest -Label "cold-or-current-cache"
    for ($index = 1; $index -le $WarmRuns; $index++) {
        Invoke-TimedCargoTest -Label "warm-$index"
    }
} finally {
    Pop-Location
}
