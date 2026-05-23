# qexed_warden

- 提交: `84f8657018f3e5d94c0573c3438a6206918a1ddf`
- 语言: `zh-CN`
- 配置文件: `config/qexed_warden.toml`
- 字段数量: `49`

| 配置项 | 类型 | 默认值 | 说明 | 提示 |
| --- | --- | --- | --- | --- |
| `data` | `object` | `{"mongodb":{"app_name":"my_rust_app","auth_source":null,"connect_timeout":"10s","database":"","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":false,"username":"qexed"},"mysql":{"charset":"utf8mb4","connec...` | 典狱长数据存储设置 | 无 |
| `data.mongodb` | `object` | `{"app_name":"my_rust_app","auth_source":null,"connect_timeout":"10s","database":"","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":false,"username":"qexed"}` | 典狱长 MongoDB 存储 | 无 |
| `data.mongodb.app_name` | `string` | `"my_rust_app"` | MongoDB 应用名称 | 无 |
| `data.mongodb.auth_source` | `null` | 无 | MongoDB 认证数据库 | 无 |
| `data.mongodb.connect_timeout` | `string` | `"10s"` | MongoDB 连接超时时间 | 无 |
| `data.mongodb.database` | `string` | `""` | MongoDB 数据库名 | 无 |
| `data.mongodb.host` | `string` | `"127.0.0.1"` | MongoDB 服务器地址 | 无 |
| `data.mongodb.max_idle_time` | `string` | `"1m"` | MongoDB 最大空闲连接时间 | 无 |
| `data.mongodb.max_pool_size` | `integer` | `100` | MongoDB 连接池最大连接数 | 无 |
| `data.mongodb.min_pool_size` | `integer` | `0` | MongoDB 连接池最小连接数 | 无 |
| `data.mongodb.password` | `string` | `"<random>"` | MongoDB 密码 | 无 |
| `data.mongodb.port` | `integer` | `27017` | MongoDB 服务器端口 | 无 |
| `data.mongodb.replica_set` | `null` | 无 | MongoDB 副本集名称 | 无 |
| `data.mongodb.socket_timeout` | `string` | `"5s"` | MongoDB Socket 超时时间 | 无 |
| `data.mongodb.use_tls` | `boolean` | `false` | 启用 MongoDB TLS 连接 | 无 |
| `data.mongodb.username` | `string` | `"qexed"` | MongoDB 用户名 | 无 |
| `data.mysql` | `object` | `{"charset":"utf8mb4","connection_timeout":"30s","database":"","idle_timeout":"5m","ip":"127.0.0.1","options":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":3306,"table_prefix":"wardon","use_ssl":false,"username":""}` | 典狱长 MySQL 存储 | 无 |
| `data.mysql.charset` | `string` | `"utf8mb4"` | MySQL 连接字符集 | 无 |
| `data.mysql.connection_timeout` | `string` | `"30s"` | MySQL 连接超时时间 | 无 |
| `data.mysql.database` | `string` | `""` | MySQL 数据库名 | 无 |
| `data.mysql.idle_timeout` | `string` | `"5m"` | MySQL 空闲连接超时时间 | 无 |
| `data.mysql.ip` | `string` | `"127.0.0.1"` | MySQL 服务器地址 | 无 |
| `data.mysql.options` | `array` | `[]` | 额外 MySQL 连接选项 | 无 |
| `data.mysql.password` | `string` | `"<random>"` | MySQL 密码 | 无 |
| `data.mysql.pool_max_size` | `integer` | `10` | MySQL 连接池最大连接数 | 无 |
| `data.mysql.pool_min_idle` | `integer` | `2` | MySQL 连接池最小空闲连接数 | 无 |
| `data.mysql.port` | `integer` | `3306` | MySQL 服务器端口 | 无 |
| `data.mysql.table_prefix` | `string` | `"wardon"` | 典狱长 MySQL 表名前缀 | 无 |
| `data.mysql.use_ssl` | `boolean` | `false` | 启用 MySQL SSL 连接 | 无 |
| `data.mysql.username` | `string` | `""` | MySQL 用户名 | 无 |
| `data.pika` | `object` | `{"connection_timeout":"1s","database":0,"host":"127.0.0.1","key_prefix":"wardon","master_name":null,"mode":"Standalone","nodes":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":9221,"timeout":"5s","use_tls":false}` | 典狱长 Pika 存储 | 无 |
| `data.pika.connection_timeout` | `string` | `"1s"` | Pika 连接超时时间 | 无 |
| `data.pika.database` | `integer` | `0` | Pika 数据库索引 | 无 |
| `data.pika.host` | `string` | `"127.0.0.1"` | Pika 服务器地址 | 无 |
| `data.pika.key_prefix` | `string` | `"wardon"` | 典狱长 Pika 记录键前缀 | 无 |
| `data.pika.master_name` | `null` | 无 | Pika 哨兵主节点名称 | 无 |
| `data.pika.mode` | `string` | `"Standalone"` | Pika 连接模式 | 无 |
| `data.pika.nodes` | `array` | `[]` | Pika 哨兵或集群节点列表 | 无 |
| `data.pika.password` | `string` | `"<random>"` | Pika 密码 | 无 |
| `data.pika.pool_max_size` | `integer` | `10` | Pika 连接池最大连接数 | 无 |
| `data.pika.pool_min_idle` | `integer` | `2` | Pika 连接池最小空闲连接数 | 无 |
| `data.pika.port` | `integer` | `9221` | Pika 服务器端口 | 无 |
| `data.pika.timeout` | `string` | `"5s"` | Pika 操作超时时间 | 无 |
| `data.pika.use_tls` | `boolean` | `false` | 启用 Pika TLS 连接 | 无 |
| `data.simple` | `object` | `{"player_list":[]}` | 典狱长内存存储 | 无 |
| `data.simple.player_list` | `array` | `[]` | 被封禁玩家 UUID 列表 | 无 |
| `data.storage_engine` | `string` | `"Simple"` | 典狱长使用的存储引擎 | 无 |
| `data.version` | `integer` | `0` | 典狱长数据配置版本 | 无 |
| `version` | `integer` | `0` | 典狱长配置版本 | 无 |
