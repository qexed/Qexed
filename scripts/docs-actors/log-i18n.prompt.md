你是本地 actor log-i18n。工作目录：E:\\code\\qexed-v6。必须使用 grok-4.6（dsh overlay 已指定），不要 DeepSeek。

# 任务
除 crates/qexed_log 外，把 log::info/warn/error/debug/trace! 和 tklog 调用改成 qexed_language 翻译。

规则：
- 不要改 crates/qexed_log
- 用户可见字符串不要写死中英文。键名：qexed.<crate>.<topic>
- 使用 qexed_language::t(key)，动态值用 %{name} 再 replace
- 同步写入 crates/qexed_language/src/locales/zh-CN.json 与 en.json
- 缺依赖就补 qexed_language
- 禁止 anyhow
- 未经用户要求不要 git commit / push
- 没有剩余硬编码日志则在 E:\\code\\qexed-v6-doc\\.docs-actors\\last-log-i18n.md 写「无需改动」
