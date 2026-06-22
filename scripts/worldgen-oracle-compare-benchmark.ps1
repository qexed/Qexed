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

function Get-TextTail {
    param(
        [object[]]$Lines,
        [int]$Count = 80
    )

    if (-not $Lines -or $Lines.Count -eq 0) {
        return @()
    }

    $skip = [Math]::Max(0, $Lines.Count - $Count)
    return @($Lines | Select-Object -Skip $skip)
}

function Get-LastWorldgenStage {
    param(
        [object[]]$Lines
    )

    $stageLine = @($Lines | Where-Object { $_ -match "worldgen_oracle_compare_stage=([^ ]+)" } | Select-Object -Last 1)
    if ($stageLine.Count -eq 0) {
        return "unknown"
    }

    return ([regex]::Match([string]$stageLine[-1], "worldgen_oracle_compare_stage=([^ ]+)")).Groups[1].Value
}

function Write-RefreshFailureDiagnostic {
    param(
        [object[]]$Stdout,
        [object[]]$Stderr
    )

    $combined = @($Stdout) + @($Stderr)
    Write-Host ("refresh_failure_stage={0}" -f (Get-LastWorldgenStage -Lines $combined))
    Write-Host "refresh_failure_stdout_tail_begin"
    Get-TextTail -Lines $Stdout | ForEach-Object { Write-Host $_ }
    Write-Host "refresh_failure_stdout_tail_end"
    Write-Host "refresh_failure_stderr_tail_begin"
    Get-TextTail -Lines $Stderr | ForEach-Object { Write-Host $_ }
    Write-Host "refresh_failure_stderr_tail_end"
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
    $stdoutPath = [System.IO.Path]::GetTempFileName()
    $stderrPath = [System.IO.Path]::GetTempFileName()
    try {
        & cargo test -p qexed $testName -- --nocapture --test-threads=1 > $stdoutPath 2> $stderrPath
        $exitCode = $LASTEXITCODE
    } finally {
        $watch.Stop()
    }
    $stdout = @(Get-Content -LiteralPath $stdoutPath -ErrorAction SilentlyContinue)
    $stderr = @(Get-Content -LiteralPath $stderrPath -ErrorAction SilentlyContinue)
    Remove-Item -LiteralPath $stdoutPath, $stderrPath -ErrorAction SilentlyContinue
    $javaAfter = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match $javaProcessPattern } | Select-Object Id, ProcessName, Path)
    $newJavaProcesses = @($javaAfter | Where-Object { $javaBefore -notcontains $_.Id })

    $stdout | ForEach-Object { Write-Host $_ }
    $stderr | ForEach-Object { Write-Host $_ }
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
        if (($RefreshJavaCache -or $RefreshRustCache) -and $exitCode -eq -1) {
            Write-RefreshFailureDiagnostic -Stdout $stdout -Stderr $stderr
        }
        throw "cargo test failed with exit code $exitCode"
    }
    if (-not $RefreshJavaCache -and -not $AllowJavaOnCacheHit -and $newJavaProcesses.Count -gt 0) {
        $newJavaProcesses | Format-Table -AutoSize
        throw "cache-hit benchmark started Java; pass -RefreshJavaCache for refresh runs or -AllowJavaOnCacheHit to only report it"
    }
}

if ($WarmRuns -lt 0) {
    throw "WarmRuns must be >= 0"
}

$effectiveWarmRuns = $WarmRuns
if (($RefreshJavaCache -or $RefreshRustCache) -and -not $PSBoundParameters.ContainsKey("WarmRuns")) {
    $effectiveWarmRuns = 0
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

    Write-Host ("requested_warm_runs={0}" -f $WarmRuns)
    Write-Host ("effective_warm_runs={0}" -f $effectiveWarmRuns)
    Invoke-TimedCargoTest -Label "cold-or-current-cache"
    for ($index = 1; $index -le $effectiveWarmRuns; $index++) {
        Invoke-TimedCargoTest -Label "warm-$index"
    }
} finally {
    Pop-Location
}
