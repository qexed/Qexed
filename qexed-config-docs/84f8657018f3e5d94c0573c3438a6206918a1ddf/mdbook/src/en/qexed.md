# qexed

- Commit: `84f8657018f3e5d94c0573c3438a6206918a1ddf`
- Language: `en`
- Config file: `config/qexed.toml`
- Field count: `80`

| Path | Type | Default | Description | Notice |
| --- | --- | --- | --- | --- |
| `language` | `string` | `"zh-CN"` | Runtime language | None |
| `plugin_download` | `object` | `{"download":"https://api.example.com/plugins/","download_token":"<random>","enable":false}` | Plugin download settings | None |
| `plugin_download.download` | `string` | `"https://api.example.com/plugins/"` | Plugin download URL | **Warning:** Use a trusted download server. This URL is used when fetching plugin metadata or plugin packages. |
| `plugin_download.download_token` | `string` | `"<random>"` | Plugin download authentication token | **Warning:** This token is sensitive. Do not share generated configuration files publicly without rotating it. |
| `plugin_download.enable` | `boolean` | `false` | Enable plugin downloads | **Migration:** The field name is kept compatible with the old qexed_config crate. |
| `server` | `object` | `{"code_of_conduct":"","display_players":true,"favicon":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6...","ip":"0.0.0.0:25565","lan_discovery":{"enable":true,"interval_ms":1500},"max_player":-1,"max_port_c...` | Server settings | None |
| `server.code_of_conduct` | `string` | `""` | Server entry code of conduct. Empty disables the code-of-conduct prompt. | None |
| `server.display_players` | `boolean` | `true` | Expose player count in status responses | None |
| `server.favicon` | `string` | `"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6..."` | Server icon data URI | None |
| `server.ip` | `string` | `"0.0.0.0:25565"` | Server listen address | None |
| `server.lan_discovery` | `object` | `{"enable":true,"interval_ms":1500}` | LAN discovery settings | None |
| `server.lan_discovery.enable` | `boolean` | `true` | Enable Minecraft LAN discovery broadcasts | None |
| `server.lan_discovery.interval_ms` | `integer` | `1500` | LAN discovery broadcast interval in milliseconds | None |
| `server.max_player` | `integer` | `-1` | Maximum player count. -1 means unlimited. | None |
| `server.max_port_connections` | `integer` | `65535` | Maximum concurrent TCP connections. 0 is treated as 65535. | None |
| `server.motd` | `array` | `["Welcome to Qexed","MC server based on Rust"]` | Random server description lines | None |
| `server.network_compression_threshold` | `integer` | `256` | Packet compression threshold in bytes | None |
| `server.online` | `boolean` | `false` | Enable public server mode | **Warning:** When disabled, clients may be accepted without the expected public-server checks. |
| `server.online_mode` | `boolean` | `true` | Enable Mojang online authentication | **Warning:** Disabling online mode allows offline usernames and increases impersonation risk. |
| `server.player_data` | `object` | `{"collection":"players","enable":true,"engine":"vanilla","mongodb":{"app_name":"qexed","auth_source":"admin","connect_timeout":"10s","database":"qexed","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":fals...` | Player save data settings | None |
| `server.player_data.collection` | `string` | `"players"` | MongoDB collection name for player data | None |
| `server.player_data.enable` | `boolean` | `true` | Enable player data saving and loading | None |
| `server.player_data.engine` | `string` | `"vanilla"` | Player data storage engine: vanilla, mongodb, or mysql | None |
| `server.player_data.mongodb` | `object` | `{"app_name":"qexed","auth_source":"admin","connect_timeout":"10s","database":"qexed","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":false,"username":"qexed"}` | MongoDB settings for player data | None |
| `server.player_data.mongodb.app_name` | `string` | `"qexed"` | MongoDB application name | None |
| `server.player_data.mongodb.auth_source` | `string` | `"admin"` | MongoDB authentication database | None |
| `server.player_data.mongodb.connect_timeout` | `string` | `"10s"` | MongoDB connection timeout | None |
| `server.player_data.mongodb.database` | `string` | `"qexed"` | MongoDB database name | None |
| `server.player_data.mongodb.host` | `string` | `"127.0.0.1"` | MongoDB server address | None |
| `server.player_data.mongodb.max_idle_time` | `string` | `"1m"` | MongoDB maximum idle connection time | None |
| `server.player_data.mongodb.max_pool_size` | `integer` | `100` | Maximum MongoDB connection pool size | None |
| `server.player_data.mongodb.min_pool_size` | `integer` | `0` | Minimum MongoDB connection pool size | None |
| `server.player_data.mongodb.password` | `string` | `"<random>"` | MongoDB password | None |
| `server.player_data.mongodb.port` | `integer` | `27017` | MongoDB server port | None |
| `server.player_data.mongodb.replica_set` | `null` | None | MongoDB replica set name | None |
| `server.player_data.mongodb.socket_timeout` | `string` | `"5s"` | MongoDB socket timeout | None |
| `server.player_data.mongodb.use_tls` | `boolean` | `false` | Enable TLS for MongoDB connections | None |
| `server.player_data.mongodb.username` | `string` | `"qexed"` | MongoDB username | None |
| `server.player_data.mysql` | `object` | `{"charset":"utf8mb4","connection_timeout":"30s","database":"qexed","idle_timeout":"5m","ip":"127.0.0.1","options":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":3306,"use_ssl":false,"username":"qexed"}` | MySQL settings for player data | None |
| `server.player_data.mysql.charset` | `string` | `"utf8mb4"` | MySQL connection charset | None |
| `server.player_data.mysql.connection_timeout` | `string` | `"30s"` | MySQL connection timeout | None |
| `server.player_data.mysql.database` | `string` | `"qexed"` | MySQL database name | None |
| `server.player_data.mysql.idle_timeout` | `string` | `"5m"` | MySQL idle connection timeout | None |
| `server.player_data.mysql.ip` | `string` | `"127.0.0.1"` | MySQL server address | None |
| `server.player_data.mysql.options` | `array` | `[]` | Additional MySQL connection options | None |
| `server.player_data.mysql.password` | `string` | `"<random>"` | MySQL password | None |
| `server.player_data.mysql.pool_max_size` | `integer` | `10` | Maximum MySQL connection pool size | None |
| `server.player_data.mysql.pool_min_idle` | `integer` | `2` | Minimum idle MySQL connections | None |
| `server.player_data.mysql.port` | `integer` | `3306` | MySQL server port | None |
| `server.player_data.mysql.use_ssl` | `boolean` | `false` | Enable SSL for MySQL connections | None |
| `server.player_data.mysql.username` | `string` | `"qexed"` | MySQL username | None |
| `server.player_data.table` | `string` | `"qexed_players"` | MySQL table name for player data | None |
| `server.proxy` | `boolean` | `false` | Enable proxy forwarding | None |
| `server.proxy_protocol` | `string` | `"QTunnel"` | Proxy forwarding protocol | None |
| `server.proxy_token` | `string` | `"<random>"` | Proxy authentication token | **Warning:** Keep this token private and rotate it if it is exposed. |
| `server.rate_limit_max_attempts` | `integer` | `6` | Maximum connection attempts allowed per IP within the rate limit window | None |
| `server.rate_limit_window_secs` | `integer` | `60` | Per-IP connection rate limit window in seconds | None |
| `server.world` | `object` | `{"chunk_load_parallelism":4,"dimension":"minecraft:overworld","dimension_type":"minecraft:overworld","game_mode":"survival","gpu":{"device":"discrete","enable":false},"light":"static","light_algorithm":"fast","path":"world","read_only":false,"simulation_distance":3,"spawn":{"pitch":0.0,"x":0.0,"y":0.0,"yaw":0.0,"z":0.0...` | World and spawn settings | None |
| `server.world.chunk_load_parallelism` | `integer` | `4` | Maximum number of chunk build tasks per player | None |
| `server.world.dimension` | `string` | `"minecraft:overworld"` | Default dimension resource key | None |
| `server.world.dimension_type` | `string` | `"minecraft:overworld"` | Default dimension type resource key | None |
| `server.world.game_mode` | `string` | `"survival"` | Default game mode: survival, creative, adventure, or spectator | None |
| `server.world.gpu` | `object` | `{"device":"discrete","enable":false}` | Experimental GPU calculation settings for world lighting. The fast algorithm is usually faster on CPU until GPU batching is implemented. | None |
| `server.world.gpu.device` | `string` | `"discrete"` | GPU selector: discrete, integrated, cpu, auto, or a numeric adapter index | None |
| `server.world.gpu.enable` | `boolean` | `false` | Enable experimental GPU lighting calculation. Disabled uses CPU; unavailable GPU falls back to CPU. | None |
| `server.world.light` | `string` | `"static"` | World lighting mode: static, dynamic, or a fixed brightness integer from 0 to 15 | None |
| `server.world.light_algorithm` | `string` | `"fast"` | World lighting calculation algorithm: fast or ray_trace | None |
| `server.world.path` | `string` | `"world"` | World save directory | None |
| `server.world.read_only` | `boolean` | `false` | Enable read-only world mode. Player world edits are rejected and not written. | None |
| `server.world.simulation_distance` | `integer` | `3` | Initial simulation distance | None |
| `server.world.spawn` | `object` | `{"pitch":0.0,"x":0.0,"y":0.0,"yaw":0.0,"z":0.0}` | Player spawn position and rotation | None |
| `server.world.spawn.pitch` | `number` | `0.0` | Spawn pitch | None |
| `server.world.spawn.x` | `number` | `0.0` | Spawn X coordinate | None |
| `server.world.spawn.y` | `number` | `0.0` | Spawn Y coordinate | None |
| `server.world.spawn.yaw` | `number` | `0.0` | Spawn yaw | None |
| `server.world.spawn.z` | `number` | `0.0` | Spawn Z coordinate | None |
| `server.world.spawn_protection_radius` | `integer` | `16` | Spawn protection radius in blocks. 0 disables spawn protection. | None |
| `server.world.view_distance` | `integer` | `3` | Initial chunk view distance | None |
| `update_check` | `boolean` | `true` | Enable update checks | None |
| `version` | `integer` | `0` | Configuration version | None |
