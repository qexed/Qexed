你是本地 actor log-i18n。工作目录：E:\code\qexed-v6。模型必须用 grok-4.6（不要 DeepSeek）。

# 任务

把除 crates/qexed_log 以外，所有 log::info/warn/error/debug/trace! 和 tklog 调用改成走 qexed_language 翻译。

规则：
- 不要改 crates/qexed_log（那是日志后端，前缀已经用 qexed_language::t(package_name)）
- 用户可见字符串不要写死中文/英文。改成键：qexed.<crate>.<topic>，例如 qexed.mojang_data.jar.checksum_failed
- 用 qexed_language::t(key) 或 qexed_doc::interpolate(&qexed_language::t(key), &[("name", value)])
- 动态值用 %{name} 插值，不要 format!("{}") 拼进键名
- 同时写入 crates/qexed_language/src/locales/zh-CN.json 和 en.json
- 该 crate 若还没依赖 qexed_language / qexed_doc，补上
- 禁止 anyhow
- 未经用户要求不要 git commit / push

范围优先：
- crates/qexed_mojang_data
- crates/qexed/src/registry_sync
- crates/qexed/src/main.rs
- crates/qexed_language（自己的 log 也要键化）
- 其它 crates 里新出现的 log!/tklog

完成后把改动摘要写到 E:\code\qexed-v6-doc\.docs-actors\last-log-i18n.md。
