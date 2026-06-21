param(
    [switch]$Full
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
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
        cargo test -p qexed_worldgen generator_v4::tests::v4_pipeline_full_chunk_manual_diagnostic -- --ignored --test-threads=1 --nocapture
    } else {
        cargo test -p qexed_worldgen generator_v4::tests::v4_pipeline_base_and_carvers_smoke_is_bounded -- --test-threads=1
    }
} finally {
    Pop-Location
}
