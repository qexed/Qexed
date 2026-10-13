# qexed 构建包装：先把编译器临时目录指到 E 盘，规避 Windows Defender 对 C 盘 Temp 中
# 新生 .obj/.pdb 实时扫描导致的 cl.exe D8050（详见 .cargo/config.toml 头部注释）。
# 用法与 cargo 完全一致，例如：
#   .\build.ps1 build -p qexed_server
#   .\build.ps1 test -p qexed_jev -- --include-ignored
#   .\build.ps1 run --bin qexed
$t = Join-Path $PSScriptRoot '.tmp\test-temp'
New-Item -ItemType Directory -Force -Path $t | Out-Null
$env:TMP = $t
$env:TEMP = $t
& cargo @args
exit $LASTEXITCODE
