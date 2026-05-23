# qexed

- 提交: `84f8657018f3e5d94c0573c3438a6206918a1ddf`
- 语言: `zh-CN`
- 配置文件: `config/qexed.toml`
- 字段数量: `80`

| 配置项 | 类型 | 默认值 | 说明 | 提示 |
| --- | --- | --- | --- | --- |
| `language` | `string` | `"zh-CN"` | 运行时语言 | 无 |
| `plugin_download` | `object` | `{"download":"https://api.example.com/plugins/","download_token":"<random>","enable":false}` | 插件下载设置 | 无 |
| `plugin_download.download` | `string` | `"https://api.example.com/plugins/"` | 插件下载地址 | **警告:** 请使用可信的下载服务器。该地址会用于获取插件元数据或插件包。 |
| `plugin_download.download_token` | `string` | `"<random>"` | 插件下载认证 token | **警告:** 该 token 属于敏感信息。公开配置文件前请先更换或移除。 |
| `plugin_download.enable` | `boolean` | `false` | 启用插件下载功能 | **迁移:** 字段名保持与旧 qexed_config crate 兼容。 |
| `server` | `object` | `{"code_of_conduct":"","display_players":true,"favicon":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6...","ip":"0.0.0.0:25565","lan_discovery":{"enable":true,"interval_ms":1500},"max_player":-1,"max_port_c...` | 服务器设置 | 无 |
| `server.code_of_conduct` | `string` | `""` | 入服准则内容。为空时不显示入服准则。 | 无 |
| `server.display_players` | `boolean` | `true` | 在状态响应中显示玩家数 | 无 |
| `server.favicon` | `string` | `"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6..."` | 服务器图标 data URI | 无 |
| `server.ip` | `string` | `"0.0.0.0:25565"` | 服务器监听地址 | 无 |
| `server.lan_discovery` | `object` | `{"enable":true,"interval_ms":1500}` | 局域网发现设置 | 无 |
| `server.lan_discovery.enable` | `boolean` | `true` | 启用 Minecraft 局域网发现广播 | 无 |
| `server.lan_discovery.interval_ms` | `integer` | `1500` | 局域网发现广播间隔，单位为毫秒 | 无 |
| `server.max_player` | `integer` | `-1` | 最大玩家数，-1 表示不限制。 | 无 |
| `server.max_port_connections` | `integer` | `65535` | 总 TCP 连接同时在线限制数，0 表示 65535。 | 无 |
| `server.motd` | `array` | `["Welcome to Qexed","MC server based on Rust"]` | 服务器描述随机内容 | 无 |
| `server.network_compression_threshold` | `integer` | `256` | 网络数据包压缩阈值，单位为字节 | 无 |
| `server.online` | `boolean` | `false` | 启用公网服务模式 | **警告:** 关闭后，客户端可能不会经过预期的公网服务检查。 |
| `server.online_mode` | `boolean` | `true` | 启用 Mojang 正版验证 | **警告:** 关闭正版验证会允许离线用户名，并增加身份冒用风险。 |
| `server.player_data` | `object` | `{"collection":"players","enable":true,"engine":"vanilla","mongodb":{"app_name":"qexed","auth_source":"admin","connect_timeout":"10s","database":"qexed","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":fals...` | 玩家存档设置 | 无 |
| `server.player_data.collection` | `string` | `"players"` | MongoDB 玩家数据集合名 | 无 |
| `server.player_data.enable` | `boolean` | `true` | 启用玩家数据保存与加载 | 无 |
| `server.player_data.engine` | `string` | `"vanilla"` | 玩家数据存储引擎：vanilla、mongodb 或 mysql | 无 |
| `server.player_data.mongodb` | `object` | `{"app_name":"qexed","auth_source":"admin","connect_timeout":"10s","database":"qexed","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":false,"username":"qexed"}` | 玩家数据 MongoDB 设置 | 无 |
| `server.player_data.mongodb.app_name` | `string` | `"qexed"` | MongoDB 应用名称 | 无 |
| `server.player_data.mongodb.auth_source` | `string` | `"admin"` | MongoDB 认证数据库 | 无 |
| `server.player_data.mongodb.connect_timeout` | `string` | `"10s"` | MongoDB 连接超时时间 | 无 |
| `server.player_data.mongodb.database` | `string` | `"qexed"` | MongoDB 数据库名 | 无 |
| `server.player_data.mongodb.host` | `string` | `"127.0.0.1"` | MongoDB 服务器地址 | 无 |
| `server.player_data.mongodb.max_idle_time` | `string` | `"1m"` | MongoDB 最大空闲连接时间 | 无 |
| `server.player_data.mongodb.max_pool_size` | `integer` | `100` | MongoDB 连接池最大连接数 | 无 |
| `server.player_data.mongodb.min_pool_size` | `integer` | `0` | MongoDB 连接池最小连接数 | 无 |
| `server.player_data.mongodb.password` | `string` | `"<random>"` | MongoDB 密码 | 无 |
| `server.player_data.mongodb.port` | `integer` | `27017` | MongoDB 服务器端口 | 无 |
| `server.player_data.mongodb.replica_set` | `null` | 无 | MongoDB 副本集名称 | 无 |
| `server.player_data.mongodb.socket_timeout` | `string` | `"5s"` | MongoDB Socket 超时时间 | 无 |
| `server.player_data.mongodb.use_tls` | `boolean` | `false` | 启用 MongoDB TLS 连接 | 无 |
| `server.player_data.mongodb.username` | `string` | `"qexed"` | MongoDB 用户名 | 无 |
| `server.player_data.mysql` | `object` | `{"charset":"utf8mb4","connection_timeout":"30s","database":"qexed","idle_timeout":"5m","ip":"127.0.0.1","options":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":3306,"use_ssl":false,"username":"qexed"}` | 玩家数据 MySQL 设置 | 无 |
| `server.player_data.mysql.charset` | `string` | `"utf8mb4"` | MySQL 连接字符集 | 无 |
| `server.player_data.mysql.connection_timeout` | `string` | `"30s"` | MySQL 连接超时时间 | 无 |
| `server.player_data.mysql.database` | `string` | `"qexed"` | MySQL 数据库名 | 无 |
| `server.player_data.mysql.idle_timeout` | `string` | `"5m"` | MySQL 空闲连接超时时间 | 无 |
| `server.player_data.mysql.ip` | `string` | `"127.0.0.1"` | MySQL 服务器地址 | 无 |
| `server.player_data.mysql.options` | `array` | `[]` | 额外 MySQL 连接选项 | 无 |
| `server.player_data.mysql.password` | `string` | `"<random>"` | MySQL 密码 | 无 |
| `server.player_data.mysql.pool_max_size` | `integer` | `10` | MySQL 连接池最大连接数 | 无 |
| `server.player_data.mysql.pool_min_idle` | `integer` | `2` | MySQL 连接池最小空闲连接数 | 无 |
| `server.player_data.mysql.port` | `integer` | `3306` | MySQL 服务器端口 | 无 |
| `server.player_data.mysql.use_ssl` | `boolean` | `false` | 启用 MySQL SSL 连接 | 无 |
| `server.player_data.mysql.username` | `string` | `"qexed"` | MySQL 用户名 | 无 |
| `server.player_data.table` | `string` | `"qexed_players"` | MySQL 玩家数据表名 | 无 |
| `server.proxy` | `boolean` | `false` | 启用代理转发 | 无 |
| `server.proxy_protocol` | `string` | `"QTunnel"` | 代理端协议 | 无 |
| `server.proxy_token` | `string` | `"<random>"` | 代理认证密钥 | **警告:** 请妥善保管该密钥；一旦泄露请立即更换。 |
| `server.rate_limit_max_attempts` | `integer` | `6` | 同 IP 在时间窗口内允许的最大连接次数 | 无 |
| `server.rate_limit_window_secs` | `integer` | `60` | 同 IP 连接频率限制的时间窗口，单位为秒 | 无 |
| `server.world` | `object` | `{"chunk_load_parallelism":4,"dimension":"minecraft:overworld","dimension_type":"minecraft:overworld","game_mode":"survival","gpu":{"device":"discrete","enable":false},"light":"static","light_algorithm":"fast","path":"world","read_only":false,"simulation_distance":3,"spawn":{"pitch":0.0,"x":0.0,"y":0.0,"yaw":0.0,"z":0.0...` | 世界与出生点设置 | 无 |
| `server.world.chunk_load_parallelism` | `integer` | `4` | 每名玩家同时构建区块的最大任务数 | 无 |
| `server.world.dimension` | `string` | `"minecraft:overworld"` | 默认维度资源键 | 无 |
| `server.world.dimension_type` | `string` | `"minecraft:overworld"` | 默认维度类型资源键 | 无 |
| `server.world.game_mode` | `string` | `"survival"` | 默认游戏模式：survival、creative、adventure 或 spectator | 无 |
| `server.world.gpu` | `object` | `{"device":"discrete","enable":false}` | 世界光照 GPU 计算设置（实验性）。在 GPU 批量化完成前，fast 算法通常 CPU 更快。 | 无 |
| `server.world.gpu.device` | `string` | `"discrete"` | GPU 选择器：discrete、integrated、cpu、auto，或数字适配器索引 | 无 |
| `server.world.gpu.enable` | `boolean` | `false` | 启用实验性 GPU 光照计算。关闭时使用 CPU；GPU 不可用时回退 CPU。 | 无 |
| `server.world.light` | `string` | `"static"` | 世界光照模式：static、dynamic，或 0 到 15 的固定亮度整数 | 无 |
| `server.world.light_algorithm` | `string` | `"fast"` | 世界光照计算算法：fast 或 ray_trace | 无 |
| `server.world.path` | `string` | `"world"` | 世界存档目录 | 无 |
| `server.world.read_only` | `boolean` | `false` | 启用只读世界模式。玩家世界修改会被拒绝，不会写入世界。 | 无 |
| `server.world.simulation_distance` | `integer` | `3` | 初始模拟距离 | 无 |
| `server.world.spawn` | `object` | `{"pitch":0.0,"x":0.0,"y":0.0,"yaw":0.0,"z":0.0}` | 玩家出生坐标与朝向 | 无 |
| `server.world.spawn.pitch` | `number` | `0.0` | 出生点俯仰角 | 无 |
| `server.world.spawn.x` | `number` | `0.0` | 出生点 X 坐标 | 无 |
| `server.world.spawn.y` | `number` | `0.0` | 出生点 Y 坐标 | 无 |
| `server.world.spawn.yaw` | `number` | `0.0` | 出生点水平朝向 | 无 |
| `server.world.spawn.z` | `number` | `0.0` | 出生点 Z 坐标 | 无 |
| `server.world.spawn_protection_radius` | `integer` | `16` | 出生点保护半径，单位为方块。0 表示关闭出生点保护。 | 无 |
| `server.world.view_distance` | `integer` | `3` | 初始区块视距 | 无 |
| `update_check` | `boolean` | `true` | 启用更新检查 | 无 |
| `version` | `integer` | `0` | 配置版本 | 无 |
