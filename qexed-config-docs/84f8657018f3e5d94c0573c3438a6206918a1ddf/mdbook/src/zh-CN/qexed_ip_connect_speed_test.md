# qexed_ip_connect_speed_test

- 提交: `84f8657018f3e5d94c0573c3438a6206918a1ddf`
- 语言: `zh-CN`
- 配置文件: `config/qexed_ip_connect_speed_test.toml`
- 字段数量: `24`

| 配置项 | 类型 | 默认值 | 说明 | 提示 |
| --- | --- | --- | --- | --- |
| `data` | `object` | `{"pika":{"connection_timeout":"1s","database":0,"host":"127.0.0.1","key_prefix":"ip_connection_speed_test","master_name":null,"mode":"Standalone","nodes":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":9221,"timeout":"5s","use_tls":false},"simple":{"player_list":[]},"storage_engine":"Simple","versi...` | IP 连接速率检测数据存储设置 | 无 |
| `data.pika` | `object` | `{"connection_timeout":"1s","database":0,"host":"127.0.0.1","key_prefix":"ip_connection_speed_test","master_name":null,"mode":"Standalone","nodes":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":9221,"timeout":"5s","use_tls":false}` | IP 连接速率检测 Pika 存储 | 无 |
| `data.pika.connection_timeout` | `string` | `"1s"` | Pika 连接超时时间 | 无 |
| `data.pika.database` | `integer` | `0` | Pika 数据库索引 | 无 |
| `data.pika.host` | `string` | `"127.0.0.1"` | Pika 服务器地址 | 无 |
| `data.pika.key_prefix` | `string` | `"ip_connection_speed_test"` | IP 连接速率检测 Pika 记录键前缀 | 无 |
| `data.pika.master_name` | `null` | 无 | Pika 哨兵主节点名称 | 无 |
| `data.pika.mode` | `string` | `"Standalone"` | Pika 连接模式 | 无 |
| `data.pika.nodes` | `array` | `[]` | Pika 哨兵或集群节点列表 | 无 |
| `data.pika.password` | `string` | `"<random>"` | Pika 密码 | 无 |
| `data.pika.pool_max_size` | `integer` | `10` | Pika 连接池最大连接数 | 无 |
| `data.pika.pool_min_idle` | `integer` | `2` | Pika 连接池最小空闲连接数 | 无 |
| `data.pika.port` | `integer` | `9221` | Pika 服务器端口 | 无 |
| `data.pika.timeout` | `string` | `"5s"` | Pika 操作超时时间 | 无 |
| `data.pika.use_tls` | `boolean` | `false` | 启用 Pika TLS 连接 | 无 |
| `data.simple` | `object` | `{"player_list":[]}` | IP 连接速率检测内存存储 | 无 |
| `data.simple.player_list` | `array` | `[]` | 简易存储中的 UUID 跟踪列表 | 无 |
| `data.storage_engine` | `string` | `"Simple"` | IP 连接速率检测使用的存储引擎 | 无 |
| `data.version` | `integer` | `0` | IP 连接速率检测数据配置版本 | 无 |
| `enable` | `boolean` | `true` | 启用 IP 连接速率检测 | 无 |
| `ttl` | `integer` | `30` | 连接速率统计 TTL，单位为秒 | 无 |
| `version` | `integer` | `0` | IP 连接速率检测配置版本 | 无 |
| `violation_count` | `integer` | `5` | TTL 时间内允许的最大违规次数 | 无 |
| `whitelist_ips` | `array` | `["127.0.0.1","::1"]` | 不参与连接速率检测的 IP 白名单 | 无 |
