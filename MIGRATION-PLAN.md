# v4 → v6 全量迁移计划

## 总量
v4 qexed 本体 119,562 行 + 待迁独立 crate（entity 946 / player 998 / plugin_api 1537 / tcp_connect 904 / tools 960 / profiler 789 / mcp 2115）

## v6 目标架构（qexed 本体仅 main.rs 组装）
| crate | 吸收的 v4 内容 | 预估行数 |
| --- | --- | --- |
| qexed_world | world/（generator 38k + root 13.5k + chunk_nbt） | 53k |
| qexed_play | play/ + inventory + structures | 22k |
| qexed_entities | entities/ + qexed_entity | 14.5k |
| qexed_plugins | plugins/ + qexed_plugin_api（dll/so 动态库加载） | 5.5k |
| qexed_connection | connection/ + auth + secure_chat + proxy_forwarding + tcp_connect | 4k |
| qexed_player | qexed_player + player_data/ | 2.2k |
| qexed_server | server/bootstrap/console/commands/status/services/cluster/杂项 | 4k |

## 硬规则（所有 actor 必须遵守）
1. 禁止 anyhow —— 用各 crate 自己的 thiserror 错误类型
2. qexed 本体（crates/qexed）只允许 main.rs + 极薄组装层
3. 26.3 协议包名/路径以 v6 现有为准（如 move_player_pos → pos）
4. MC_VERSION/PROTOCOL_VERSION 已是 26.3/777
5. 插件加载：v6 用 libloading（dll/so），不用 v4 的机制（v4 机制待确认后对齐）
6. 未经用户要求不 git
7. 每个 crate：error/mod.rs 定义 XxxError；config 用 app_config 宏（如有配置）
8. i18n：log 文案用 qexed_language::t(key)，键写 locales JSON

## 依赖方向（禁止环）
qexed(本体) → server → play/entities/player/connection/plugins → world → protocol/packet/nbt → config/log/language
