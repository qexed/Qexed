param(
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$runDir = Join-Path $repoRoot "run/noise_raw"
$configDir = Join-Path $runDir "config"
$logFile = Join-Path $runDir "logs/qexed.log"
$binary = Join-Path $repoRoot "target/debug/qexed.exe"
if (-not (Test-Path $binary)) {
    $binary = Join-Path $repoRoot "target/debug/qexed"
}

function Test-TcpPort {
    param(
        [string]$HostName,
        [int]$Port
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $connect = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $connect.AsyncWaitHandle.WaitOne(250)) {
            return $false
        }
        $client.EndConnect($connect)
        return $true
    } catch {
        return $false
    } finally {
        $client.Close()
    }
}

if (-not (Test-Path $binary)) {
    throw "qexed binary not found. Run: cargo build -p qexed"
}
if (-not (Test-Path (Join-Path $configDir "qexed.toml"))) {
    throw "missing run/noise_raw/config/qexed.toml"
}
if (-not (Test-Path (Join-Path $configDir "log.toml"))) {
    throw "missing run/noise_raw/config/log.toml"
}
if (Test-TcpPort -HostName "127.0.0.1" -Port 25565) {
    throw "127.0.0.1:25565 is already open; stop the existing server or change run/noise_raw/config/qexed.toml"
}

$stdout = Join-Path $runDir "logs/smoke-noise-raw.stdout.log"
$stderr = Join-Path $runDir "logs/smoke-noise-raw.stderr.log"
$startedAt = Get-Date
$process = Start-Process -FilePath $binary `
    -WorkingDirectory $runDir `
    -RedirectStandardOutput $stdout `
    -RedirectStandardError $stderr `
    -WindowStyle Hidden `
    -PassThru

try {
    $deadline = $startedAt.AddSeconds($TimeoutSeconds)
    do {
        if ($process.HasExited) {
            $out = if (Test-Path $stdout) { Get-Content $stdout -Raw } else { "" }
            $err = if (Test-Path $stderr) { Get-Content $stderr -Raw } else { "" }
            throw "qexed exited before opening port. stdout:`n$out`nstderr:`n$err"
        }
        if (Test-TcpPort -HostName "127.0.0.1" -Port 25565) {
            break
        }
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)

    if (-not (Test-TcpPort -HostName "127.0.0.1" -Port 25565)) {
        throw "qexed did not open 127.0.0.1:25565 within $TimeoutSeconds seconds"
    }

    $hasListenLog = $false
    $hasSaveLog = $false
    $logTail = @()
    do {
        if (Test-Path $logFile) {
            $logTail = Get-Content $logFile -Tail 120
            $hasListenLog = [bool]($logTail | Select-String -SimpleMatch "qexed server listening on")
            $hasSaveLog = [bool]($logTail | Select-String -SimpleMatch "qexed save root initialized at")
        }
        if ($hasListenLog -and $hasSaveLog) {
            break
        }
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)

    if (-not $hasListenLog -or -not $hasSaveLog) {
        $out = if (Test-Path $stdout) { Get-Content $stdout -Raw } else { "" }
        $err = if (Test-Path $stderr) { Get-Content $stderr -Raw } else { "" }
        throw "qexed port opened, but expected startup log lines were not found in $logFile. stdout:`n$out`nstderr:`n$err`nlog tail:`n$($logTail -join "`n")"
    }

    Write-Host "noise_raw smoke ok: qexed opened 127.0.0.1:25565 and wrote startup logs."
} finally {
    if (-not $process.HasExited) {
        $process.Kill()
        $process.WaitForExit()
    }
}
