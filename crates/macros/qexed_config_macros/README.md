# qexed_config_macros

[![Crates.io](https://img.shields.io/crates/v/qexed_config_macros.svg)](https://crates.io/crates/qexed_config_macros)
[![Documentation](https://docs.rs/qexed_config_macros/badge.svg)](https://docs.rs/qexed_config_macros)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

[`qexed_config::Config`](https://crates.io/crates/qexed_config) 的属性宏：一行注解生成完整 impl，并从 autodoc 标签自动收集机密字段规则。

```rust
#[qexed_config_macros::app_config("/", "log")]
struct LogConfig {
    /// 日志等级
    level: String,
    /// <Secret />
    token: String,   // 自动进入 SECRETS
}
```

等价于手写：

```rust
impl ::qexed_config::Config for LogConfig {
    const PATH: &'static str = "/";
    const NAME: &'static str = "log";
    const SECRETS: &'static [&'static str] = &["token"];
}
```

## 机密规则的两个来源

| 写法 | 效果 |
| --- | --- |
| 字段 doc 里写 `<Secret />` | 该字段（serde rename 后的键名优先）进入 `SECRETS` |
| 结构体 doc 里写 `<Secrets>a.b, list.*.key</Secrets>` | 逗号/换行分隔的嵌套与通配路径全部进入 `SECRETS` |

两处声明的模式取并集去重。没有 `secrets = [...]` 参数——机密规则只来自 autodoc 标签。

## 依赖要求

宏展开生成 `impl qexed_config::Config`，因此**使用方 crate 必须同时依赖 `qexed_config`**（非宏 re-export）：

```toml
[dependencies]
qexed_config = "0.1"
qexed_config_macros = "0.1"
```

## 参数

```rust
#[app_config(PATH, NAME)]   // 恰好两个位置参数，都是字符串字面量
```

- `PATH`：文件所在子目录（相对配置根目录，`"/"` 表示根）
- `NAME`：文件名（自动补 `.toml` 后缀，仅允许字母数字与 `_`）

## MSRV

Rust 1.85+（edition 2024）。

## License

MIT — 见 [LICENSE](./LICENSE)。
