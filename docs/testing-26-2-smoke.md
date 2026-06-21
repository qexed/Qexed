# 26.2 进服 smoke 准备

## run/noise_raw

`run/noise_raw/config` 当前可用于低成本进服 smoke：

- `qexed.toml` 绑定 `0.0.0.0:25565`，`view_distance = 1`，`simulation_distance = 1`。
- `log.toml` 使用 `Info` 级别，日志写入 `run/noise_raw/logs/qexed.log`。
- `save.toml` 的根目录为当前运行目录，世界数据写入 `run/noise_raw/world`。

先构建服务端：

```powershell
cargo build -p qexed
```

然后执行只检查启动、端口和日志的 smoke：

```powershell
powershell -ExecutionPolicy Bypass -File "scripts/smoke-noise-raw.ps1"
```

该脚本会在 `run/noise_raw` 启动 `target/debug/qexed`，确认 `127.0.0.1:25565` 可连接，并检查日志中存在监听和存档初始化记录。脚本结束时会关闭它启动的服务端。

## 26.2 客户端进服

仓库内没有 SoulFire 可执行工具；如果本机另有 SoulFire，可把目标地址指向 `127.0.0.1:25565` 做离线登录 smoke。

现有 `tools/mc-vanilla-protocol` 已配置 Minecraft `26.2` vanilla client quickPlay。需要人工观察客户端时，先启动服务端：

```powershell
cargo build -p qexed
Push-Location "run/noise_raw"
..\..\target\debug\qexed.exe
```

再从另一个终端启动 26.2 客户端：

```powershell
Push-Location "tools/mc-vanilla-protocol"
gradle vanillaClient -PqexedQuickPlay=127.0.0.1:25565
```

本次准备不使用 `assets/reports`。`protocolIntegrationTest` 仍可作为协议目录对齐测试，但它读取 `assets/reports/packets.json`，不作为本次“无 assets”进服 smoke 的主路径。
