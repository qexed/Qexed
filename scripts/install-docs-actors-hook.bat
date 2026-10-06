@echo off
setlocal EnableExtensions
cd /d "%~dp0.."
if not exist ".git\\hooks" (
  echo not a git checkout: %CD%
  exit /b 1
)
copy /Y "scripts\\git-hooks\\post-commit" ".git\\hooks\\post-commit" >nul
echo installed .git\\hooks\\post-commit
echo next git commit will run local docs-auditor then docs-translator
echo logs: ..\\qexed-v6-doc\\.docs-actors\\
exit /b 0
