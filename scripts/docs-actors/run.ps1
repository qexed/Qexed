$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$ActorDir = $PSScriptRoot
$Root = (Resolve-Path (Join-Path $ActorDir '..\..')).Path
$DocRoot = Join-Path (Split-Path -Parent $Root) 'qexed-v6-doc'
$OutDir = Join-Path $DocRoot '.docs-actors'

if (-not (Test-Path $DocRoot)) {
    throw "docs repo missing: $DocRoot"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Find-Dsh {
    $cmd = Get-Command dsh -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($p in @('D:\bin\dsh.ps1')) {
        if (Test-Path $p) { return $p }
    }
    return $null
}

$dsh = Find-Dsh
if (-not $dsh) { throw 'dsh not found on PATH' }

Push-Location $Root
try {
    $sha = (git rev-parse HEAD).Trim()
    $title = (git log -1 --pretty=%s).Trim()
    $files = @(git diff-tree --no-commit-id --name-only -r HEAD)
} finally {
    Pop-Location
}

$fileText = ($files -join "`n")
$touches = $false
foreach ($f in $files) {
    if ($f -like 'crates/*' -or $f -eq 'Cargo.toml' -or $f -eq 'Cargo.lock') { $touches = $true }
}
if (-not $touches) {
    $msg = "skip: $sha did not change crates"
    [IO.File]::WriteAllText((Join-Path $OutDir 'last-audit.md'), $msg + "`n")
    Write-Host $msg
    exit 0
}

function Expand-Prompt($src, $dst) {
    $text = [IO.File]::ReadAllText($src)
    $text = $text.Replace('{{SHA}}', $sha).Replace('{{TITLE}}', $title).Replace('{{FILES}}', $fileText)
    [IO.File]::WriteAllText($dst, $text)
}

$auditPrompt = Join-Path $OutDir 'audit.prompt.md'
$translatePrompt = Join-Path $OutDir 'translate.prompt.md'
Expand-Prompt (Join-Path $ActorDir 'docs-auditor.prompt.md') $auditPrompt
Expand-Prompt (Join-Path $ActorDir 'docs-translator.prompt.md') $translatePrompt
$i18nPrompt = Join-Path $OutDir 'log-i18n.prompt.md'
Expand-Prompt (Join-Path $ActorDir 'log-i18n.prompt.md') $i18nPrompt

function Find-DshBin {
    $ps1 = Find-Dsh
    $candidates = @(
        'C:\Program Files\nodejs\node_modules\@deepseek-ai\dsh\lib\bin.js',
        'D:\nvm\nvm\v22.23.1\node_modules\@deepseek-ai\dsh\lib\bin.js'
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    if ($ps1) {
        $txt = [IO.File]::ReadAllText($ps1)
        if ($txt -match '"([^"]+dsh\\lib\\bin\.js)"') { return $Matches[1] }
    }
    throw 'dsh bin.js not found'
}

function Invoke-Actor($name, $promptFile, $logName) {
    $actorLog = Join-Path $OutDir $logName
    $stamp = Get-Date -Format o
    Add-Content $actorLog "[$stamp] start $name sha=$sha"
    $bin = Find-DshBin
    $node = (Get-Command node).Source
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $node
    $patch = Join-Path $ActorDir 'dsh-grok-4.6.patch.yml'
    $psi.Arguments = '"' + $bin + '" --profile headless --patch "' + $patch + '" -'
    $psi.WorkingDirectory = $DocRoot
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $psi.StandardOutputEncoding = [Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [Text.Encoding]::UTF8
    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    [void]$proc.Start()
    $stdoutTask = $proc.StandardOutput.ReadToEndAsync()
    $stderrTask = $proc.StandardError.ReadToEndAsync()
    $proc.StandardInput.Write([IO.File]::ReadAllText($promptFile))
    $proc.StandardInput.Close()
    $proc.WaitForExit()
    $out = $stdoutTask.Result + [Environment]::NewLine + $stderrTask.Result
    $code = $proc.ExitCode
    Add-Content $actorLog $out
    Add-Content (Join-Path $OutDir 'last-run.log') "[$name] exit=$code"
    if ($code -ne 0) { throw "$name failed exit=$code log=$actorLog" }
}

[IO.File]::WriteAllText((Join-Path $OutDir 'running'), "audit $sha`n")
try {
    Invoke-Actor 'docs-auditor' $auditPrompt 'last-audit-run.log'
    Invoke-Actor 'docs-translator' $translatePrompt 'last-translate-run.log'
    Invoke-Actor 'log-i18n' $i18nPrompt 'last-log-i18n-run.log'
} finally {
    Remove-Item (Join-Path $OutDir 'running') -ErrorAction SilentlyContinue
}
Write-Host "docs actors finished for $sha"

