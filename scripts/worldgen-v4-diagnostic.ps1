param(
    [switch]$Full
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

function Invoke-CapturedProcess {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList
    )

    $stdoutPath = [System.IO.Path]::GetTempFileName()
    $stderrPath = [System.IO.Path]::GetTempFileName()
    try {
        $startedAt = Get-Date
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $process = Start-Process `
            -FilePath $FilePath `
            -ArgumentList $ArgumentList `
            -NoNewWindow `
            -Wait `
            -PassThru `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath
        $watch.Stop()

        @(Get-Content -LiteralPath $stdoutPath -ErrorAction SilentlyContinue) |
            ForEach-Object { Write-Host $_ }
        @(Get-Content -LiteralPath $stderrPath -ErrorAction SilentlyContinue) |
            ForEach-Object { Write-Host $_ }
        Write-Host ("diagnostic_started_at={0:o}" -f $startedAt)
        Write-Host ("diagnostic_elapsed_ms={0}" -f [int64]$watch.Elapsed.TotalMilliseconds)
        Write-Host ("diagnostic_exit_code={0}" -f $process.ExitCode)

        if ($process.ExitCode -ne 0) {
            throw "cargo diagnostic failed with exit code $($process.ExitCode)"
        }
    } finally {
        if ($watch -and $watch.IsRunning) {
            $watch.Stop()
        }
        Remove-Item -LiteralPath $stdoutPath, $stderrPath -ErrorAction SilentlyContinue
    }
}

Push-Location $repoRoot
try {
    $stale = Get-Process |
        Where-Object { $_.ProcessName -like "qexed_worldgen-*" } |
        Select-Object Id, ProcessName, Path

    if ($stale) {
        Write-Host "Found qexed_worldgen test processes that can keep test executables locked:"
        $stale | Format-Table -AutoSize
        Write-Host "Stop those processes before rebuilding if link.exe reports LNK1104."
    }

    if ($Full) {
        Invoke-CapturedProcess `
            -FilePath "cargo" `
            -ArgumentList @("test", "-p", "qexed_worldgen", "generator_v4::tests::v4_pipeline_full_chunk_manual_diagnostic", "--", "--ignored", "--test-threads=1", "--nocapture")
    } else {
        Invoke-CapturedProcess `
            -FilePath "cargo" `
            -ArgumentList @("test", "-p", "qexed_worldgen", "generator_v4::tests::v4_pipeline_base_and_carvers_smoke_is_bounded", "--", "--test-threads=1")
    }
} finally {
    Pop-Location
}
