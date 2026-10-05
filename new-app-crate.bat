@echo off
setlocal EnableExtensions
cd /d "%~dp0"

if "%~1"=="" (
  echo usage: %~nx0 ^<crate-stem^>
  echo   %~nx0 a              -^> crates/qexed_a , AConfig , qexed.crates.a.config.AConfig
  echo   %~nx0 mojang_data    -^> crates/qexed_mojang_data
  echo   %~nx0 qexed_foo_bar  -^> crates/qexed_foo_bar
  exit /b 2
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0new-app-crate.ps1" %*
exit /b %ERRORLEVEL%
