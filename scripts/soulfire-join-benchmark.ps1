param(
    [string]$SoulfireCommand = "",
    [string[]]$SoulfireArgs = @(),
    [string]$SoulfireJar = "",
    [int]$ServerReadyTimeoutSeconds = 30,
    [int]$ClientTimeoutSeconds = 90,
    [int]$ChunkLoadTimeoutSeconds = 20,
    [int]$PostClientWaitSeconds = 5,
    [int]$Port = 25565,
    [string]$HostName = "127.0.0.1",
    [string[]]$SoulfireTargetArgs = @(),
    [int]$ExpectedInitialChunks = 9,
    [int]$ChunkLoadComplianceMs = 5000,
    [switch]$SkipTemporaryDebugLog,
    [switch]$NoAutoDiscoverSoulfire,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$runDir = Join-Path $repoRoot "run/noise_raw"
$configDir = Join-Path $runDir "config"
$logDir = Join-Path $runDir "logs"
$serverLog = Join-Path $logDir "qexed.log"
$serverConfig = Join-Path $configDir "qexed.toml"
$logConfig = Join-Path $configDir "log.toml"
$serverBinary = Join-Path $repoRoot "target/debug/qexed.exe"
if (-not (Test-Path $serverBinary)) {
    $serverBinary = Join-Path $repoRoot "target/debug/qexed"
}

function Resolve-TemplateArgs {
    param([string[]]$Args)

    $resolved = New-Object System.Collections.Generic.List[string]
    foreach ($arg in $Args) {
        $resolved.Add(
            $arg.
                Replace("{host}", $HostName).
                Replace("{port}", [string]$Port).
                Replace("{address}", "$HostName`:$Port")
        )
    }

    return @($resolved)
}

function Join-LaunchArgs {
    param(
        [string[]]$Args,
        [string[]]$TargetArgs
    )

    if ($TargetArgs -and $TargetArgs.Count -gt 0) {
        return @($Args) + (Resolve-TemplateArgs -Args $TargetArgs)
    }

    return @($Args)
}

function Test-TcpPort {
    param(
        [string]$TargetHost,
        [int]$TargetPort
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $connect = $client.BeginConnect($TargetHost, $TargetPort, $null, $null)
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

function Get-FirstExistingDirectory {
    param([string[]]$Paths)

    foreach ($path in $Paths) {
        if ($path -and (Test-Path $path -PathType Container)) {
            $resolved = Resolve-Path $path -ErrorAction SilentlyContinue
            if ($resolved) {
                $resolved.Path
            }
        }
    }
}

function Get-ConfiguredPort {
    param([string]$Path)

    if (-not (Test-Path $Path -PathType Leaf)) {
        return $null
    }

    $text = Get-Content -Path $Path -Raw
    $match = [regex]::Match($text, '(?m)^\s*bind\s*=\s*"(?<bind>[^"]+)"')
    if (-not $match.Success) {
        return $null
    }

    $bind = $match.Groups["bind"].Value
    $lastColon = $bind.LastIndexOf(":")
    if ($lastColon -lt 0) {
        return $null
    }

    $parsedPort = 0
    if ([int]::TryParse($bind.Substring($lastColon + 1), [ref]$parsedPort)) {
        return $parsedPort
    }

    return $null
}

function Find-SoulfireFile {
    $repoRoots = Get-FirstExistingDirectory @(
        (Join-Path $repoRoot "tools"),
        (Join-Path $repoRoot "scripts"),
        (Join-Path $repoRoot "target/debug"),
        (Join-Path $repoRoot "target/release")
    )

    foreach ($root in $repoRoots) {
        $candidate = Get-ChildItem -Path $root -Recurse -Force -File -Include "*soulfire*.exe","*soulfire*.bat","*soulfire*.cmd","*soulfire*.ps1","*soulfire*.jar" -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notmatch "SoulFireBlock" -and $_.FullName -ne $PSCommandPath } |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($candidate) {
            return $candidate.FullName
        }
    }

    $userRoots = Get-FirstExistingDirectory @(
        (Join-Path $env:USERPROFILE "Downloads"),
        (Join-Path $env:USERPROFILE "Documents"),
        (Join-Path $env:USERPROFILE "Desktop")
    )

    foreach ($root in $userRoots) {
        $candidate = Get-ChildItem -Path $root -Force -File -Include "*soulfire*.exe","*soulfire*.bat","*soulfire*.cmd","*soulfire*.ps1","*soulfire*.jar" -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notmatch "SoulFireBlock" } |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($candidate) {
            return $candidate.FullName
        }
    }

    return ""
}

function Resolve-SoulfireLaunch {
    if ($SoulfireJar) {
        if (-not (Test-Path $SoulfireJar -PathType Leaf)) {
            throw "SoulfireJar not found: $SoulfireJar"
        }
        return @{
            Command = "java"
            Args = Join-LaunchArgs -Args (@("-jar", (Resolve-Path $SoulfireJar).Path) + $SoulfireArgs) -TargetArgs $SoulfireTargetArgs
            Source = (Resolve-Path $SoulfireJar).Path
        }
    }

    if ($SoulfireCommand) {
        return @{
            Command = $SoulfireCommand
            Args = Join-LaunchArgs -Args $SoulfireArgs -TargetArgs $SoulfireTargetArgs
            Source = $SoulfireCommand
        }
    }

    if ($NoAutoDiscoverSoulfire) {
        return $null
    }

    $pathCommand = Get-Command "soulfire" -ErrorAction SilentlyContinue
    if ($pathCommand) {
        return @{
            Command = $pathCommand.Source
            Args = Join-LaunchArgs -Args $SoulfireArgs -TargetArgs $SoulfireTargetArgs
            Source = $pathCommand.Source
        }
    }

    $file = Find-SoulfireFile
    if (-not $file) {
        return $null
    }

    $extension = [System.IO.Path]::GetExtension($file)
    if ($extension -ieq ".jar") {
        return @{
            Command = "java"
            Args = Join-LaunchArgs -Args (@("-jar", $file) + $SoulfireArgs) -TargetArgs $SoulfireTargetArgs
            Source = $file
        }
    }

    if ($extension -ieq ".ps1") {
        return @{
            Command = "powershell"
            Args = Join-LaunchArgs -Args (@("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $file) + $SoulfireArgs) -TargetArgs $SoulfireTargetArgs
            Source = $file
        }
    }

    return @{
        Command = $file
        Args = Join-LaunchArgs -Args $SoulfireArgs -TargetArgs $SoulfireTargetArgs
        Source = $file
    }
}

function Read-FileTail {
    param(
        [string]$Path,
        [int]$LineCount = 80
    )

    if (-not (Test-Path $Path)) {
        return ""
    }

    return (Get-Content -Path $Path -Tail $LineCount -ErrorAction SilentlyContinue) -join "`n"
}

function Read-FileTailFromOffset {
    param(
        [string]$Path,
        [int64]$Offset
    )

    if (-not (Test-Path $Path)) {
        return ""
    }

    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
    try {
        if ($Offset -gt $stream.Length) {
            $Offset = 0
        }
        $stream.Seek($Offset, [System.IO.SeekOrigin]::Begin) | Out-Null
        $reader = [System.IO.StreamReader]::new($stream)
        try {
            return $reader.ReadToEnd()
        } finally {
            $reader.Close()
        }
    } finally {
        $stream.Close()
    }
}

function Set-TemporaryDebugLog {
    param([string]$Path)

    if ($SkipTemporaryDebugLog -or -not (Test-Path $Path -PathType Leaf)) {
        return $null
    }

    $original = Get-Content -Path $Path -Raw
    if ($original -match '(?m)^\s*level\s*=\s*"Debug"\s*$') {
        return @{
            Path = $Path
            Original = $original
            Changed = $false
        }
    }

    $updated = if ($original -match '(?m)^\s*level\s*=\s*"[^"]*"\s*$') {
        [regex]::Replace($original, '(?m)^\s*level\s*=\s*"[^"]*"\s*$', 'level = "Debug"', 1)
    } else {
        "level = `"Debug`"`r`n$original"
    }

    Set-Content -Path $Path -Value $updated -NoNewline
    return @{
        Path = $Path
        Original = $original
        Changed = $true
    }
}

function Restore-TemporaryDebugLog {
    param([hashtable]$State)

    if (-not $State -or -not $State.Changed) {
        return
    }

    Set-Content -Path $State.Path -Value $State.Original -NoNewline
}

function Get-ProcessTreeSnapshot {
    param([int]$RootProcessId)

    $processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
    $byParent = @{}
    foreach ($process in $processes) {
        $parentId = [int]$process.ParentProcessId
        if (-not $byParent.ContainsKey($parentId)) {
            $byParent[$parentId] = New-Object System.Collections.Generic.List[object]
        }
        $byParent[$parentId].Add($process)
    }

    $owned = New-Object System.Collections.Generic.List[object]
    $queue = New-Object System.Collections.Queue
    $queue.Enqueue($RootProcessId)
    $seen = @{}
    while ($queue.Count -gt 0) {
        $parentId = [int]$queue.Dequeue()
        if ($seen.ContainsKey($parentId)) {
            continue
        }
        $seen[$parentId] = $true
        if (-not $byParent.ContainsKey($parentId)) {
            continue
        }
        foreach ($child in $byParent[$parentId]) {
            $owned.Add($child)
            $queue.Enqueue([int]$child.ProcessId)
        }
    }

    return $owned.ToArray()
}

function Stop-OwnedProcessTree {
    param(
        [System.Diagnostics.Process]$RootProcess,
        [string]$Label
    )

    if (-not $RootProcess) {
        return
    }

    $rootId = [int]$RootProcess.Id
    $descendants = @(Get-ProcessTreeSnapshot -RootProcessId $rootId | Sort-Object ProcessId -Descending)
    foreach ($child in $descendants) {
        try {
            Write-Host ("owned_process_cleanup={0}: child_pid={1} name={2}" -f $Label, $child.ProcessId, $child.Name)
            Stop-Process -Id ([int]$child.ProcessId) -Force -ErrorAction Stop
        } catch {
            Write-Host ("owned_process_cleanup_warning={0}: child_pid={1} error={2}" -f $Label, $child.ProcessId, $_.Exception.Message)
        }
    }

    try {
        $RootProcess.Refresh()
        if (-not $RootProcess.HasExited) {
            Write-Host ("owned_process_cleanup={0}: root_pid={1} name={2}" -f $Label, $rootId, $RootProcess.ProcessName)
            $RootProcess.Kill()
            $RootProcess.WaitForExit()
        }
    } catch {
        Write-Host ("owned_process_cleanup_warning={0}: root_pid={1} error={2}" -f $Label, $rootId, $_.Exception.Message)
    }
}

function Get-LogTimestamp {
    param([string]$Line)

    $match = [regex]::Match($Line, '^\[[A-Z]+\]\s+(?<stamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\b')
    if (-not $match.Success) {
        return $null
    }

    try {
        return [datetime]::ParseExact(
            $match.Groups["stamp"].Value,
            "yyyy-MM-dd HH:mm:ss",
            [System.Globalization.CultureInfo]::InvariantCulture
        )
    } catch {
        return $null
    }
}

function Get-FirstLogLineInfo {
    param(
        [string]$Text,
        [string]$Pattern
    )

    foreach ($line in ($Text -split "`r?`n")) {
        if ($line -match $Pattern) {
            return @{
                Line = $line
                Time = Get-LogTimestamp -Line $line
            }
        }
    }

    return $null
}

function Get-ElapsedMillisecondsOrNull {
    param(
        [Nullable[datetime]]$Start,
        [Nullable[datetime]]$End
    )

    if (-not $Start.HasValue -or -not $End.HasValue) {
        return $null
    }

    return [int64]($End.Value - $Start.Value).TotalMilliseconds
}

function Get-ObservedBenchmarkState {
    param(
        [datetime]$ClientStartedAt,
        [System.Diagnostics.Stopwatch]$ClientWatch,
        [string]$ServerLogOffsetPath,
        [int64]$ServerLogOffset,
        [string]$QexedStdoutPath,
        [string]$QexedStderrPath,
        [string]$SoulfireStdoutPath,
        [string]$SoulfireStderrPath
    )

    $newServerLog = Read-FileTailFromOffset -Path $ServerLogOffsetPath -Offset $ServerLogOffset
    $qexedOut = Read-FileTailFromOffset -Path $QexedStdoutPath -Offset 0
    $qexedErr = Read-FileTailFromOffset -Path $QexedStderrPath -Offset 0
    $clientOut = Read-FileTailFromOffset -Path $SoulfireStdoutPath -Offset 0
    $clientErr = Read-FileTailFromOffset -Path $SoulfireStderrPath -Offset 0
    $serverText = "$newServerLog`n$qexedOut`n$qexedErr"
    $clientText = "$clientOut`n$clientErr"
    $chunkMetrics = Get-ChunkSyncMetrics -LogText $serverText
    $serverJoin = Get-FirstLogLineInfo -Text $serverText -Pattern "joined qexed-v5"
    $clientJoin = Get-FirstLogLineInfo -Text $clientText -Pattern "(?i)\b(joined|spawned|logged in|login successful)\b"
    $joinSeen = [bool]($serverJoin -or $chunkMetrics -or $clientJoin)
    $connectionMs = $null
    if ($serverJoin -and $serverJoin.Time) {
        $connectionMs = Get-ElapsedMillisecondsOrNull -Start $ClientStartedAt -End $serverJoin.Time
    }
    if (-not $connectionMs -and $joinSeen) {
        $connectionMs = [int64]$ClientWatch.Elapsed.TotalMilliseconds
    }
    $firstChunkMs = $null
    if ($chunkMetrics -and $chunkMetrics.Time) {
        $firstChunkMs = Get-ElapsedMillisecondsOrNull -Start $ClientStartedAt -End $chunkMetrics.Time
    }

    return @{
        NewServerLog = $newServerLog
        QexedOut = $qexedOut
        QexedErr = $qexedErr
        ClientOut = $clientOut
        ClientErr = $clientErr
        ServerText = $serverText
        ClientText = $clientText
        ChunkMetrics = $chunkMetrics
        ServerJoin = $serverJoin
        ClientJoin = $clientJoin
        JoinSeen = $joinSeen
        ConnectionMs = $connectionMs
        FirstChunkMs = $firstChunkMs
    }
}

function Write-DiagnosticBlock {
    param(
        [string]$Reason,
        [string]$QexedStdout,
        [string]$QexedStderr,
        [string]$SoulfireStdout = "",
        [string]$SoulfireStderr = "",
        [string]$QexedLog = ""
    )

    Write-Host "benchmark_status=failed"
    Write-Host ("failure_reason={0}" -f $Reason)
    Write-Host ("target={0}:{1}" -f $HostName, $Port)
    Write-Host ("qexed_stdout={0}" -f $QexedStdout)
    Write-Host ("qexed_stderr={0}" -f $QexedStderr)
    if ($SoulfireStdout) {
        Write-Host ("soulfire_stdout={0}" -f $SoulfireStdout)
    }
    if ($SoulfireStderr) {
        Write-Host ("soulfire_stderr={0}" -f $SoulfireStderr)
    }
    Write-Host ("qexed_log={0}" -f $QexedLog)
    Write-Host "failure_logs=qexed_stdout,qexed_stderr,soulfire_stdout,soulfire_stderr,qexed_log"
    Write-Host "qexed_stdout_tail_begin"
    Write-Host (Read-FileTail -Path $QexedStdout)
    Write-Host "qexed_stdout_tail_end"
    Write-Host "qexed_stderr_tail_begin"
    Write-Host (Read-FileTail -Path $QexedStderr)
    Write-Host "qexed_stderr_tail_end"
    if ($SoulfireStdout) {
        Write-Host "soulfire_stdout_tail_begin"
        Write-Host (Read-FileTail -Path $SoulfireStdout)
        Write-Host "soulfire_stdout_tail_end"
    }
    if ($SoulfireStderr) {
        Write-Host "soulfire_stderr_tail_begin"
        Write-Host (Read-FileTail -Path $SoulfireStderr)
        Write-Host "soulfire_stderr_tail_end"
    }
    Write-Host "qexed_log_tail_begin"
    Write-Host (Read-FileTail -Path $QexedLog)
    Write-Host "qexed_log_tail_end"
}

function Get-ChunkSyncMetrics {
    param([string]$LogText)

    $pattern = "synced player chunks: cause=(?<cause>\w+), center=\((?<center>[^)]*)\), sent_new=(?<sent>\d+), unloading=(?<unloading>\d+), unloaded=(?<unloaded>\d+), first_loaded=(?<first>[^,]+(?:, [^)]+\))?|none), first_unloaded=(?<firstUnloaded>[^,]+(?:, [^)]+\))?|none), sources=saved=(?<saved>\d+), local_generated=(?<local>\d+), vanilla_generated=(?<vanilla>\d+), empty_fallback=(?<empty>\d+)"
    $matches = [regex]::Matches($LogText, $pattern)
    if ($matches.Count -eq 0) {
        return $null
    }

    $initial = $matches | Where-Object { $_.Groups["cause"].Value -eq "InitialLogin" } | Select-Object -First 1
    if (-not $initial) {
        $initial = $matches[0]
    }

    return @{
        Line = $initial.Value
        Time = Get-LogTimestamp -Line $initial.Value
        Cause = $initial.Groups["cause"].Value
        Center = $initial.Groups["center"].Value
        SentNew = [int]$initial.Groups["sent"].Value
        Unloading = [int]$initial.Groups["unloading"].Value
        Unloaded = [int]$initial.Groups["unloaded"].Value
        FirstLoaded = $initial.Groups["first"].Value
        Saved = [int]$initial.Groups["saved"].Value
        LocalGenerated = [int]$initial.Groups["local"].Value
        VanillaGenerated = [int]$initial.Groups["vanilla"].Value
        EmptyFallback = [int]$initial.Groups["empty"].Value
        TotalSyncEvents = $matches.Count
    }
}

function Write-OptimizationHint {
    param(
        [hashtable]$Metrics,
        [string]$ServerText
    )

    if ($Metrics) {
        if ($Metrics.LocalGenerated -gt 0) {
            Write-Host "optimization_focus=qexed_worldgen::generator_v4 local chunk generation"
            return
        }
        if ($Metrics.VanillaGenerated -gt 0) {
            Write-Host "optimization_focus=qexed_world Java worldgen RPC/cache path"
            return
        }
        if ($Metrics.EmptyFallback -gt 0) {
            Write-Host "optimization_focus=qexed bootstrap local_worldgen cache initialization; missing chunks fell back to empty chunks"
            return
        }
        Write-Host "optimization_focus=qexed connection/play login and saved chunk serialization path"
        return
    }

    if ($ServerText -match "local worldgen unavailable") {
        Write-Host "optimization_focus=qexed_worldgen cache/bootstrap availability; local worldgen unavailable before join"
    } else {
        Write-Host "optimization_focus=enable Debug log level to expose synced player chunks; otherwise inspect qexed connection login/configuration path and qexed_world chunk send path"
    }
}

function Write-ActorFeedback {
    param(
        [string]$ChunkLoadStatus,
        $ChunkLoadMs,
        [hashtable]$Metrics
    )

    if ($ChunkLoadStatus -eq "below_expected") {
        Write-Host ("actor_feedback=性能/区块同步 actor: initial_chunks_sent={0} below expected {1}; inspect qexed_world chunk view scheduling and first login send path." -f $Metrics.SentNew, $ExpectedInitialChunks)
        return
    }
    if ($ChunkLoadStatus -eq "degraded_empty_fallback") {
        Write-Host ("actor_feedback=性能/区块同步 actor: empty fallback chunks observed ({0}); inspect local worldgen bootstrap/cache availability before login." -f $Metrics.EmptyFallback)
        return
    }
    if ($null -ne $ChunkLoadMs -and [int64]$ChunkLoadMs -gt $ChunkLoadComplianceMs) {
        Write-Host ("actor_feedback=性能/区块同步 actor: first chunk sync took {0}ms over {1}ms budget; inspect worldgen/cache IO and chunk serialization/send path." -f ([int64]$ChunkLoadMs), $ChunkLoadComplianceMs)
        return
    }
    if ($ChunkLoadStatus -eq "not_observed") {
        Write-Host "actor_feedback=性能/区块同步 actor: initial chunk sync was not observable; ensure Debug log emits synced player chunks, then inspect qexed connection/play login to qexed_world chunk send path."
    }
}

function Get-ChunkLoadStatus {
    param([hashtable]$Metrics)

    if (-not $Metrics) {
        return "not_observed"
    }
    if ($Metrics.SentNew -lt $ExpectedInitialChunks) {
        return "below_expected"
    }
    if ($Metrics.EmptyFallback -gt 0) {
        return "degraded_empty_fallback"
    }
    return "ok"
}

function Get-ComplianceStatus {
    param(
        [string]$ChunkLoadStatus,
        $ChunkLoadMs
    )

    if ($ChunkLoadStatus -ne "ok") {
        return "failed"
    }
    if ($null -ne $ChunkLoadMs -and [int64]$ChunkLoadMs -gt $ChunkLoadComplianceMs) {
        return "failed"
    }
    if ($null -eq $ChunkLoadMs) {
        return "unknown"
    }
    return "ok"
}

function Invoke-SelfTest {
    $sampleLog = @"
[DEBUG] 2026-06-22 12:00:00 synced player chunks: cause=InitialLogin, center=(0, 0), sent_new=9, unloading=0, unloaded=0, first_loaded=(-1, -1), first_unloaded=none, sources=saved=0, local_generated=9, vanilla_generated=0, empty_fallback=0
"@
    $metrics = Get-ChunkSyncMetrics -LogText $sampleLog
    if (-not $metrics) {
        throw "self-test failed: chunk sync metrics were not parsed"
    }
    if ($metrics.Cause -ne "InitialLogin" -or $metrics.SentNew -ne 9 -or $metrics.LocalGenerated -ne 9) {
        throw "self-test failed: parsed metrics are incorrect"
    }
    $status = Get-ChunkLoadStatus -Metrics $metrics
    if ($status -ne "ok") {
        throw "self-test failed: expected ok chunk status, got $status"
    }
    $compliance = Get-ComplianceStatus -ChunkLoadStatus $status -ChunkLoadMs ([int64]100)
    if ($compliance -ne "ok") {
        throw "self-test failed: expected ok compliance, got $compliance"
    }
    Write-Host "self_test=ok"
    Write-Host "parsed_initial_chunks_sent=$($metrics.SentNew)"
    Write-Host "parsed_local_generated=$($metrics.LocalGenerated)"
    Write-Host "chunk_load_status=$status"
    Write-Host "chunk_load_compliance=$compliance"
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if (-not (Test-Path $serverBinary)) {
    throw "qexed binary not found. Run: cargo build -p qexed"
}
if (-not (Test-Path $runDir -PathType Container)) {
    throw "run directory not found: $runDir"
}
if (-not (Test-Path $serverConfig -PathType Leaf)) {
    throw "missing run/noise_raw/config/qexed.toml"
}
$configuredPort = Get-ConfiguredPort -Path $serverConfig
if ($configuredPort -and $configuredPort -ne $Port) {
    throw "Port mismatch: script target is $Port but run/noise_raw/config/qexed.toml binds $configuredPort. Pass -Port $configuredPort or update the config."
}
if (Test-TcpPort -TargetHost $HostName -TargetPort $Port) {
    throw "$HostName`:$Port is already open. This script will not stop or reuse non-owned processes; stop the existing server or choose a free configured port."
}

New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$qexedStdout = Join-Path $logDir "soulfire-qexed-$timestamp.stdout.log"
$qexedStderr = Join-Path $logDir "soulfire-qexed-$timestamp.stderr.log"
$soulfireStdout = Join-Path $logDir "soulfire-client-$timestamp.stdout.log"
$soulfireStderr = Join-Path $logDir "soulfire-client-$timestamp.stderr.log"
$serverLogStart = if (Test-Path $serverLog) { (Get-Item $serverLog).Length } else { 0 }
$logConfigState = $null
$serverProcess = $null

$resolvedSoulfire = Resolve-SoulfireLaunch
if (-not $resolvedSoulfire) {
    Write-Host "soulfire_found=false"
    Write-Host "qexed_server_started=false"
    Write-Host "benchmark_status=blocked"
    Write-Host "blocker=Soulfire executable/JAR was not found in PATH, repo tools/scripts/target, Desktop, Downloads, or Documents."
    Write-Host "usage=powershell -ExecutionPolicy Bypass -File `"scripts/soulfire-join-benchmark.ps1`" -SoulfireCommand <path-or-command> -SoulfireArgs <args>"
    Write-Host "optimization_focus=qexed connection/play login and qexed_world chunk send path cannot be measured until Soulfire launch is provided"
    exit 2
}

try {
    $logConfigState = Set-TemporaryDebugLog -Path $logConfig
    $serverStartedAt = Get-Date
    $serverWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $serverProcess = Start-Process -FilePath $serverBinary `
        -WorkingDirectory $runDir `
        -RedirectStandardOutput $qexedStdout `
        -RedirectStandardError $qexedStderr `
        -WindowStyle Hidden `
        -PassThru

    $deadline = $serverStartedAt.AddSeconds($ServerReadyTimeoutSeconds)
    do {
        if ($serverProcess.HasExited) {
            Write-DiagnosticBlock -Reason "qexed exited before opening the port" -QexedStdout $qexedStdout -QexedStderr $qexedStderr -QexedLog $serverLog
            throw "qexed exited before opening the port"
        }
        if (Test-TcpPort -TargetHost $HostName -TargetPort $Port) {
            break
        }
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)

    if (-not (Test-TcpPort -TargetHost $HostName -TargetPort $Port)) {
        Write-DiagnosticBlock -Reason "qexed did not open the target port before timeout" -QexedStdout $qexedStdout -QexedStderr $qexedStderr -QexedLog $serverLog
        throw "qexed did not open $HostName`:$Port within $ServerReadyTimeoutSeconds seconds"
    }

    $hasListenLog = $false
    $hasSaveLog = $false
    do {
        if ($serverProcess.HasExited) {
            Write-DiagnosticBlock -Reason "qexed exited after opening the port but before startup logs were complete" -QexedStdout $qexedStdout -QexedStderr $qexedStderr -QexedLog $serverLog
            throw "qexed exited before startup logs were complete"
        }
        if (Test-Path $serverLog) {
            $serverLogTail = Read-FileTail -Path $serverLog -LineCount 160
            $hasListenLog = $serverLogTail -match "qexed server listening on"
            $hasSaveLog = $serverLogTail -match "qexed save root initialized at"
        }
        if ($hasListenLog -and $hasSaveLog) {
            break
        }
        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)

    if (-not $hasListenLog -or -not $hasSaveLog) {
        Write-DiagnosticBlock -Reason "qexed opened the port but startup log markers were not observed" -QexedStdout $qexedStdout -QexedStderr $qexedStderr -QexedLog $serverLog
        throw "qexed opened $HostName`:$Port, but expected startup log markers were not observed"
    }

    $serverReadyMs = [int64]$serverWatch.Elapsed.TotalMilliseconds
    $clientStartedAt = Get-Date
    $clientWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $clientStartInfo = @{
        FilePath = $resolvedSoulfire.Command
        WorkingDirectory = $repoRoot
        RedirectStandardOutput = $soulfireStdout
        RedirectStandardError = $soulfireStderr
        WindowStyle = "Hidden"
        PassThru = $true
    }
    if ($resolvedSoulfire.Args -and $resolvedSoulfire.Args.Count -gt 0) {
        $clientStartInfo.ArgumentList = $resolvedSoulfire.Args
    }
    $clientProcess = Start-Process @clientStartInfo

    $clientDeadline = $clientStartedAt.AddSeconds($ClientTimeoutSeconds)
    $chunkDeadline = $clientStartedAt.AddSeconds($ClientTimeoutSeconds + $ChunkLoadTimeoutSeconds)
    $observedState = $null
    do {
        if ($serverProcess.HasExited) {
            Write-DiagnosticBlock -Reason "qexed exited while Soulfire was joining" -QexedStdout $qexedStdout -QexedStderr $qexedStderr -SoulfireStdout $soulfireStdout -SoulfireStderr $soulfireStderr -QexedLog $serverLog
            throw "qexed exited while Soulfire was joining"
        }

        $observedState = Get-ObservedBenchmarkState `
            -ClientStartedAt $clientStartedAt `
            -ClientWatch $clientWatch `
            -ServerLogOffsetPath $serverLog `
            -ServerLogOffset $serverLogStart `
            -QexedStdoutPath $qexedStdout `
            -QexedStderrPath $qexedStderr `
            -SoulfireStdoutPath $soulfireStdout `
            -SoulfireStderrPath $soulfireStderr

        if ($observedState.JoinSeen -and $observedState.ChunkMetrics) {
            break
        }

        $clientProcess.Refresh()
        if ($clientProcess.HasExited) {
            break
        }

        Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $chunkDeadline)

    $clientProcess.Refresh()
    if (-not $clientProcess.HasExited) {
        Stop-OwnedProcessTree -RootProcess $clientProcess -Label "soulfire_observed"
        $clientProcess.Refresh()
    }
    $clientWatch.Stop()

    if ($PostClientWaitSeconds -gt 0) {
        Start-Sleep -Seconds $PostClientWaitSeconds
    }

    $observedState = Get-ObservedBenchmarkState `
        -ClientStartedAt $clientStartedAt `
        -ClientWatch $clientWatch `
        -ServerLogOffsetPath $serverLog `
        -ServerLogOffset $serverLogStart `
        -QexedStdoutPath $qexedStdout `
        -QexedStderrPath $qexedStderr `
        -SoulfireStdoutPath $soulfireStdout `
        -SoulfireStderrPath $soulfireStderr

    $newServerLog = $observedState.NewServerLog
    $serverText = $observedState.ServerText
    $chunkMetrics = $observedState.ChunkMetrics
    $serverJoin = $observedState.ServerJoin
    $clientJoin = $observedState.ClientJoin
    $joinSeen = $observedState.JoinSeen
    $connectionMs = $observedState.ConnectionMs
    $firstChunkMs = $observedState.FirstChunkMs
    $chunkLoadStatus = Get-ChunkLoadStatus -Metrics $chunkMetrics
    $complianceStatus = Get-ComplianceStatus -ChunkLoadStatus $chunkLoadStatus -ChunkLoadMs $firstChunkMs
    $clientExitCode = if ($clientProcess.HasExited) { $clientProcess.ExitCode } else { "stopped_after_observation" }

    Write-Host "benchmark_status=completed"
    Write-Host "soulfire_found=true"
    Write-Host ("soulfire_source={0}" -f $resolvedSoulfire.Source)
    Write-Host ("target={0}:{1}" -f $HostName, $Port)
    Write-Host ("started_at={0:o}" -f $serverStartedAt)
    Write-Host "qexed_ready_ms=$serverReadyMs"
    Write-Host "soulfire_exit_code=$clientExitCode"
    Write-Host ("soulfire_elapsed_ms={0}" -f [int64]$clientWatch.Elapsed.TotalMilliseconds)
    if ($connectionMs) {
        Write-Host ("connection_elapsed_ms={0}" -f $connectionMs)
    } else {
        Write-Host "connection_elapsed_ms=not_observed"
    }
    Write-Host ("join_observed={0}" -f $joinSeen.ToString().ToLowerInvariant())
    if ($serverJoin) {
        Write-Host ("join_log={0}" -f $serverJoin.Line)
    } elseif ($clientJoin) {
        Write-Host ("join_log={0}" -f $clientJoin.Line)
    } else {
        Write-Host "join_log=not_observed"
    }
    if ($chunkMetrics) {
        Write-Host ("chunk_sync_events={0}" -f $chunkMetrics.TotalSyncEvents)
        Write-Host ("initial_chunk_cause={0}" -f $chunkMetrics.Cause)
        Write-Host ("initial_chunk_center={0}" -f $chunkMetrics.Center)
        Write-Host ("initial_chunks_sent={0}" -f $chunkMetrics.SentNew)
        Write-Host ("initial_first_loaded={0}" -f $chunkMetrics.FirstLoaded)
        Write-Host ("initial_chunks_saved={0}" -f $chunkMetrics.Saved)
        Write-Host ("initial_chunks_local_generated={0}" -f $chunkMetrics.LocalGenerated)
        Write-Host ("initial_chunks_vanilla_generated={0}" -f $chunkMetrics.VanillaGenerated)
        Write-Host ("initial_chunks_empty_fallback={0}" -f $chunkMetrics.EmptyFallback)
        Write-Host ("initial_chunk_log={0}" -f $chunkMetrics.Line)
        if ($firstChunkMs) {
            Write-Host ("first_chunk_elapsed_ms={0}" -f $firstChunkMs)
        } else {
            Write-Host "first_chunk_elapsed_ms=timestamp_unavailable"
        }
    } else {
        Write-Host "chunk_sync_events=0"
        Write-Host "chunk_load_speed=not_observed"
        Write-Host "chunk_load_note=run/noise_raw/config/log.toml likely needs Debug to emit synced player chunks"
        Write-Host "first_chunk_elapsed_ms=not_observed"
    }
    Write-Host ("chunk_load_status={0}" -f $chunkLoadStatus)
    Write-Host ("chunk_load_compliance_ms={0}" -f $ChunkLoadComplianceMs)
    Write-Host ("chunk_load_compliance={0}" -f $complianceStatus)
    Write-OptimizationHint -Metrics $chunkMetrics -ServerText $serverText
    if ($complianceStatus -eq "failed") {
        Write-ActorFeedback -ChunkLoadStatus $chunkLoadStatus -ChunkLoadMs $firstChunkMs -Metrics $chunkMetrics
    } elseif ($complianceStatus -eq "unknown") {
        Write-ActorFeedback -ChunkLoadStatus $chunkLoadStatus -ChunkLoadMs $firstChunkMs -Metrics $chunkMetrics
    }
    Write-Host "qexed_stdout=$qexedStdout"
    Write-Host "qexed_stderr=$qexedStderr"
    Write-Host "soulfire_stdout=$soulfireStdout"
    Write-Host "soulfire_stderr=$soulfireStderr"
    if ($logConfigState) {
        Write-Host ("temporary_debug_log={0}" -f $logConfigState.Changed.ToString().ToLowerInvariant())
    }
    Write-Host "qexed_new_log_begin"
    Write-Host $newServerLog
    Write-Host "qexed_new_log_end"

    if (-not $joinSeen) {
        Write-DiagnosticBlock -Reason "Soulfire did not reach join state before timeout" -QexedStdout $qexedStdout -QexedStderr $qexedStderr -SoulfireStdout $soulfireStdout -SoulfireStderr $soulfireStderr -QexedLog $serverLog
        throw "Soulfire did not reach join state within $ClientTimeoutSeconds seconds. Logs: $soulfireStdout / $soulfireStderr"
    }
    if ($clientProcess.HasExited -and $clientProcess.ExitCode -ne 0) {
        throw "Soulfire exited with code $($clientProcess.ExitCode). Logs: $soulfireStdout / $soulfireStderr"
    }
} finally {
    Stop-OwnedProcessTree -RootProcess $serverProcess -Label "qexed_server"
    Restore-TemporaryDebugLog -State $logConfigState
}
