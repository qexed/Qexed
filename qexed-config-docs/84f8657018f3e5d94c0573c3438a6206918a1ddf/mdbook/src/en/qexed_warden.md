# qexed_warden

- Commit: `84f8657018f3e5d94c0573c3438a6206918a1ddf`
- Language: `en`
- Config file: `config/qexed_warden.toml`
- Field count: `49`

| Path | Type | Default | Description | Notice |
| --- | --- | --- | --- | --- |
| `data` | `object` | `{"mongodb":{"app_name":"my_rust_app","auth_source":null,"connect_timeout":"10s","database":"","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":false,"username":"qexed"},"mysql":{"charset":"utf8mb4","connec...` | Warden data storage settings | None |
| `data.mongodb` | `object` | `{"app_name":"my_rust_app","auth_source":null,"connect_timeout":"10s","database":"","host":"127.0.0.1","max_idle_time":"1m","max_pool_size":100,"min_pool_size":0,"password":"<random>","port":27017,"replica_set":null,"socket_timeout":"5s","use_tls":false,"username":"qexed"}` | Warden MongoDB storage | None |
| `data.mongodb.app_name` | `string` | `"my_rust_app"` | MongoDB application name | None |
| `data.mongodb.auth_source` | `null` | None | MongoDB authentication database | None |
| `data.mongodb.connect_timeout` | `string` | `"10s"` | MongoDB connection timeout | None |
| `data.mongodb.database` | `string` | `""` | MongoDB database name | None |
| `data.mongodb.host` | `string` | `"127.0.0.1"` | MongoDB server address | None |
| `data.mongodb.max_idle_time` | `string` | `"1m"` | MongoDB maximum idle connection time | None |
| `data.mongodb.max_pool_size` | `integer` | `100` | Maximum MongoDB connection pool size | None |
| `data.mongodb.min_pool_size` | `integer` | `0` | Minimum MongoDB connection pool size | None |
| `data.mongodb.password` | `string` | `"<random>"` | MongoDB password | None |
| `data.mongodb.port` | `integer` | `27017` | MongoDB server port | None |
| `data.mongodb.replica_set` | `null` | None | MongoDB replica set name | None |
| `data.mongodb.socket_timeout` | `string` | `"5s"` | MongoDB socket timeout | None |
| `data.mongodb.use_tls` | `boolean` | `false` | Enable TLS for MongoDB connections | None |
| `data.mongodb.username` | `string` | `"qexed"` | MongoDB username | None |
| `data.mysql` | `object` | `{"charset":"utf8mb4","connection_timeout":"30s","database":"","idle_timeout":"5m","ip":"127.0.0.1","options":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":3306,"table_prefix":"wardon","use_ssl":false,"username":""}` | Warden MySQL storage | None |
| `data.mysql.charset` | `string` | `"utf8mb4"` | MySQL connection charset | None |
| `data.mysql.connection_timeout` | `string` | `"30s"` | MySQL connection timeout | None |
| `data.mysql.database` | `string` | `""` | MySQL database name | None |
| `data.mysql.idle_timeout` | `string` | `"5m"` | MySQL idle connection timeout | None |
| `data.mysql.ip` | `string` | `"127.0.0.1"` | MySQL server address | None |
| `data.mysql.options` | `array` | `[]` | Additional MySQL connection options | None |
| `data.mysql.password` | `string` | `"<random>"` | MySQL password | None |
| `data.mysql.pool_max_size` | `integer` | `10` | Maximum MySQL connection pool size | None |
| `data.mysql.pool_min_idle` | `integer` | `2` | Minimum idle MySQL connections | None |
| `data.mysql.port` | `integer` | `3306` | MySQL server port | None |
| `data.mysql.table_prefix` | `string` | `"wardon"` | Table prefix for Warden MySQL tables | None |
| `data.mysql.use_ssl` | `boolean` | `false` | Enable SSL for MySQL connections | None |
| `data.mysql.username` | `string` | `""` | MySQL username | None |
| `data.pika` | `object` | `{"connection_timeout":"1s","database":0,"host":"127.0.0.1","key_prefix":"wardon","master_name":null,"mode":"Standalone","nodes":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":9221,"timeout":"5s","use_tls":false}` | Warden Pika storage | None |
| `data.pika.connection_timeout` | `string` | `"1s"` | Pika connection timeout | None |
| `data.pika.database` | `integer` | `0` | Pika database index | None |
| `data.pika.host` | `string` | `"127.0.0.1"` | Pika server address | None |
| `data.pika.key_prefix` | `string` | `"wardon"` | Key prefix for Warden Pika records | None |
| `data.pika.master_name` | `null` | None | Pika sentinel master name | None |
| `data.pika.mode` | `string` | `"Standalone"` | Pika connection mode | None |
| `data.pika.nodes` | `array` | `[]` | Pika sentinel or cluster nodes | None |
| `data.pika.password` | `string` | `"<random>"` | Pika password | None |
| `data.pika.pool_max_size` | `integer` | `10` | Maximum Pika connection pool size | None |
| `data.pika.pool_min_idle` | `integer` | `2` | Minimum idle Pika connections | None |
| `data.pika.port` | `integer` | `9221` | Pika server port | None |
| `data.pika.timeout` | `string` | `"5s"` | Pika operation timeout | None |
| `data.pika.use_tls` | `boolean` | `false` | Enable TLS for Pika connections | None |
| `data.simple` | `object` | `{"player_list":[]}` | In-memory Warden storage | None |
| `data.simple.player_list` | `array` | `[]` | Banned player UUID list | None |
| `data.storage_engine` | `string` | `"Simple"` | Storage engine used by Warden | None |
| `data.version` | `integer` | `0` | Warden data configuration version | None |
| `version` | `integer` | `0` | Warden configuration version | None |
