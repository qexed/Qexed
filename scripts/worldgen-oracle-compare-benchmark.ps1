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

function Invoke-CapturedProcess {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList
    )

    $stdoutPath = [System.IO.Path]::GetTempFileName()
    $stderrPath = [System.IO.Path]::GetTempFileName()
    try {
        $process = Start-Process `
            -FilePath $FilePath `
            -ArgumentList $ArgumentList `
            -NoNewWindow `
            -Wait `
            -PassThru `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath

        [pscustomobject]@{
            ExitCode = $process.ExitCode
            Stdout = @(Get-Content -LiteralPath $stdoutPath -ErrorAction SilentlyContinue)
            Stderr = @(Get-Content -LiteralPath $stderrPath -ErrorAction SilentlyContinue)
        }
    } finally {
        Remove-Item -LiteralPath $stdoutPath, $stderrPath -ErrorAction SilentlyContinue
    }
}

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

function Get-FirstDiffSummary {
    param(
        [object[]]$Lines
    )

    $line = @(
        $Lines |
            Where-Object {
                $_ -match "first(_oracle)?_diff=" -or
                $_ -match "cached semantic digest differs"
            } |
            Select-Object -First 1
    )
    if ($line.Count -eq 0) {
        return "none"
    }

    return [string]$line[0]
}

function Get-OracleCacheVersionRoot {
    $root = Join-Path $repoRoot "target/worldgen-oracle"
    if (-not (Test-Path -LiteralPath $root)) {
        return $null
    }

    $versionRoots = @(
        Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc -Descending
    )
    if ($versionRoots.Count -eq 0) {
        return $null
    }

    return $versionRoots[0].FullName
}

function Get-DigestSummary {
    param(
        [int64[]]$Seeds
    )

    $versionRoot = Get-OracleCacheVersionRoot
    if (-not $versionRoot) {
        return @("semantic_digest_status=missing_oracle_cache_root")
    }

    $lines = @("semantic_digest_cache_root=$versionRoot")
    foreach ($seed in $Seeds) {
        $chunkRoot = Join-Path $versionRoot "seed-$seed/minecraft/overworld/chunks/x.0.z.0"
        $javaDigestPath = Join-Path $chunkRoot "semantic-digest.txt"
        $rustDigestPath = Join-Path $versionRoot "seed-$seed/minecraft/overworld/rust/chunks/x.0.z.0/semantic-digest.txt"
        $javaDigest = if (Test-Path -LiteralPath $javaDigestPath) {
            (Get-Content -LiteralPath $javaDigestPath -Raw).Trim()
        } else {
            "missing"
        }
        $rustDigest = if (Test-Path -LiteralPath $rustDigestPath) {
            (Get-Content -LiteralPath $rustDigestPath -Raw).Trim()
        } else {
            "missing"
        }

        $lines += "semantic_digest seed=$seed java=$javaDigest rust=$rustDigest match=$($javaDigest -eq $rustDigest)"
    }

    return $lines
}

function Get-CompareSeeds {
    if ($env:QEXED_WORLDGEN_COMPARE_JAVA_SEEDS) {
        return @(
            $env:QEXED_WORLDGEN_COMPARE_JAVA_SEEDS.Split(",") |
                ForEach-Object { $_.Trim() } |
                Where-Object { $_ } |
                ForEach-Object { [int64]$_ }
        )
    }

    return @(0)
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
    try {
        $result = Invoke-CapturedProcess `
            -FilePath "cargo" `
            -ArgumentList @("test", "-p", "qexed", $testName, "--", "--nocapture", "--test-threads=1")
    } finally {
        $watch.Stop()
    }
    $stdout = @($result.Stdout)
    $stderr = @($result.Stderr)
    $exitCode = $result.ExitCode
    $combined = @($stdout) + @($stderr)
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
    Write-Host ("last_worldgen_stage={0}" -f (Get-LastWorldgenStage -Lines $combined))
    Write-Host ("first_diff={0}" -f (Get-FirstDiffSummary -Lines $combined))
    Get-DigestSummary -Seeds (Get-CompareSeeds) | ForEach-Object { Write-Host $_ }

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
        Write-Host "benchmark_status=completed"
    } catch {
        Write-Host "benchmark_status=failed"
        Write-Host ("benchmark_error={0}" -f $_.Exception.Message)
        exit 1
    }
} finally {
    Pop-Location
}
