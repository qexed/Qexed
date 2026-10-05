param(
    [Parameter(Position = 0)]
    [string]$Name
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($Name)) {
    Write-Error "usage: new-app-crate.bat <crate-stem>"
    exit 2
}

function ConvertTo-PascalCase([string]$Value) {
    $parts = $Value.Split("_") | Where-Object { $_ -ne "" }
    $out = ""
    foreach ($part in $parts) {
        $out += $part.Substring(0, 1).ToUpperInvariant() + $part.Substring(1).ToLowerInvariant()
    }
    return $out
}

$raw = $Name.Trim().ToLowerInvariant()
if ($raw.StartsWith("qexed_")) {
    $stem = $raw.Substring(6)
} else {
    $stem = $raw
}

if ($stem -notmatch "^[a-z][a-z0-9_]*$") {
    Write-Error "crate stem must be snake_case starting with a letter: $stem"
    exit 2
}

$crateName = "qexed_$stem"
$pascal = ConvertTo-PascalCase $stem
$configStruct = "${pascal}Config"
$errorEnum = "${pascal}Error"
$autodocName = "qexed.crates.$stem.config.$configStruct"
$memberPath = "crates/$crateName"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$crateDir = Join-Path $root $memberPath
$srcDir = Join-Path $crateDir "src"
$configDir = Join-Path $srcDir "config"
$errorDir = Join-Path $srcDir "error"

if (Test-Path $crateDir) {
    Write-Error "already exists: $crateDir"
    exit 1
}

New-Item -ItemType Directory -Path $configDir | Out-Null
New-Item -ItemType Directory -Path $errorDir | Out-Null

$nl = [Environment]::NewLine
$bt = [char]96

$cargoToml = @"
[package]
name = "$crateName"
version = "0.1.0"
edition = "2024"

[dependencies]
qexed_config.workspace = true
qexed_config_macros.workspace = true
qexed_language.workspace = true
qexed_doc.workspace = true
qexed_doc_macros.workspace = true

tokio.workspace = true
tklog.workspace = true
thiserror.workspace = true
serde.workspace = true

# 下面的库自己写
"@

$libRs = "pub mod config;" + $nl + "pub mod error;" + $nl

$errorRs = @"
#[derive(Debug, thiserror::Error)]
pub enum $errorEnum {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    // 下面更多的自定义错误
}
"@

$configRs = @"
use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

// autodoc <Name> 模板：crate 名去掉 qexed_ 前缀，结构体为 PascalCase + Config。
//   qexed_mojang_data -> qexed.crates.mojang_data.config.MojangDataConfig
//   qexed_a           -> qexed.crates.a.config.AConfig
//   qexed_log         -> qexed.crates.log.config.LogConfig
/// $($bt)$($bt)$($bt)autodoc
/// <Name>$autodocName</Name>
/// <Attr name="writable" />
/// $($bt)$($bt)$($bt)
#[qexed_config_macros::app_config("/", "$stem")]
#[derive(Debug, Default, Serialize, Deserialize, Doc)]
pub struct $configStruct {}
"@

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    $enc = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($Path, $Content.TrimEnd() + $nl, $enc)
}

Write-Utf8NoBom (Join-Path $crateDir "Cargo.toml") $cargoToml
Write-Utf8NoBom (Join-Path $srcDir "lib.rs") $libRs
Write-Utf8NoBom (Join-Path $errorDir "mod.rs") $errorRs
Write-Utf8NoBom (Join-Path $configDir "mod.rs") $configRs

$workspaceToml = Join-Path $root "Cargo.toml"
$text = [System.IO.File]::ReadAllText($workspaceToml)

if ($text -match [regex]::Escape("`"$memberPath`"")) {
    Write-Error "already a workspace member: $memberPath"
    exit 1
}

$memberLine = "    `"$memberPath`","
if ($text -notmatch '(?s)members\s*=\s*\[[^\]]*\]') {
    Write-Error "cannot find workspace members array in Cargo.toml"
    exit 1
}
$text = [regex]::Replace(
    $text,
    '(?s)(members\s*=\s*\[[^\]]*?)(\r?\n\])',
    "`$1`r`n$memberLine`$2",
    1
)

$depLine = "$crateName.path = `"$memberPath`""
if ($text -match [regex]::Escape("$crateName.path")) {
    Write-Error "already in workspace.dependencies: $crateName"
    exit 1
}
if ($text -notmatch '\[workspace\.dependencies\]') {
    Write-Error "cannot find [workspace.dependencies] in Cargo.toml"
    exit 1
}
$text = [regex]::Replace(
    $text,
    '(\[workspace\.dependencies\]\r?\n)',
    "`$1$depLine`r`n",
    1
)

Write-Utf8NoBom $workspaceToml $text

Write-Host "created $memberPath"
Write-Host "  crate   $crateName"
Write-Host "  config  $configStruct  ->  <config-path>/$stem.toml"
Write-Host "  autodoc $autodocName"
Write-Host "  error   $errorEnum"
Write-Host "registered in workspace members + dependencies"

