# qexed_ip_connect_speed_test

- Commit: `84f8657018f3e5d94c0573c3438a6206918a1ddf`
- Language: `en`
- Config file: `config/qexed_ip_connect_speed_test.toml`
- Field count: `24`

| Path | Type | Default | Description | Notice |
| --- | --- | --- | --- | --- |
| `data` | `object` | `{"pika":{"connection_timeout":"1s","database":0,"host":"127.0.0.1","key_prefix":"ip_connection_speed_test","master_name":null,"mode":"Standalone","nodes":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":9221,"timeout":"5s","use_tls":false},"simple":{"player_list":[]},"storage_engine":"Simple","versi...` | IP connection speed test storage settings | None |
| `data.pika` | `object` | `{"connection_timeout":"1s","database":0,"host":"127.0.0.1","key_prefix":"ip_connection_speed_test","master_name":null,"mode":"Standalone","nodes":[],"password":"<random>","pool_max_size":10,"pool_min_idle":2,"port":9221,"timeout":"5s","use_tls":false}` | IP connection speed test Pika storage | None |
| `data.pika.connection_timeout` | `string` | `"1s"` | Pika connection timeout | None |
| `data.pika.database` | `integer` | `0` | Pika database index | None |
| `data.pika.host` | `string` | `"127.0.0.1"` | Pika server address | None |
| `data.pika.key_prefix` | `string` | `"ip_connection_speed_test"` | Key prefix for IP connection speed test records | None |
| `data.pika.master_name` | `null` | None | Pika sentinel master name | None |
| `data.pika.mode` | `string` | `"Standalone"` | Pika connection mode | None |
| `data.pika.nodes` | `array` | `[]` | Pika sentinel or cluster nodes | None |
| `data.pika.password` | `string` | `"<random>"` | Pika password | None |
| `data.pika.pool_max_size` | `integer` | `10` | Maximum Pika connection pool size | None |
| `data.pika.pool_min_idle` | `integer` | `2` | Minimum idle Pika connections | None |
| `data.pika.port` | `integer` | `9221` | Pika server port | None |
| `data.pika.timeout` | `string` | `"5s"` | Pika operation timeout | None |
| `data.pika.use_tls` | `boolean` | `false` | Enable TLS for Pika connections | None |
| `data.simple` | `object` | `{"player_list":[]}` | In-memory IP connection speed storage | None |
| `data.simple.player_list` | `array` | `[]` | Tracked UUID list for simple storage | None |
| `data.storage_engine` | `string` | `"Simple"` | Storage engine used by IP connection speed detection | None |
| `data.version` | `integer` | `0` | IP connection speed test data configuration version | None |
| `enable` | `boolean` | `true` | Enable IP connection speed detection | None |
| `ttl` | `integer` | `30` | Connection rate tracking TTL in seconds | None |
| `version` | `integer` | `0` | IP connection speed test configuration version | None |
| `violation_count` | `integer` | `5` | Maximum allowed violations within the TTL | None |
| `whitelist_ips` | `array` | `["127.0.0.1","::1"]` | IP addresses excluded from connection speed detection | None |
