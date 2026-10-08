# qexed_config

[![Crates.io](https://img.shields.io/crates/v/qexed_config.svg)](https://crates.io/crates/qexed_config)
[![Documentation](https://docs.rs/qexed_config/badge.svg)](https://docs.rs/qexed_config)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

基于 trait 模板方法的 TOML 配置管理框架：实现 `Config` trait，自动获得 **创建/加载/保存** + **默认值合并** + **机密字段自动拆分**。

从 [Qexed](https://github.com/qexed/Qexed) 项目拆分出的独立模块。底层原语来自 [`qexed_toml`](https://crates.io/crates/qexed_toml)。

## ⚠️ 历史设计说明（请先读）

**本包最初是 Qexed 的内部包**，因此沿用了"进程级全局配置根目录"的设计：启动时调用一次 `init_config_path`，之后所有 `Config` 实现共享这个根目录。这对单应用很方便，但对通用库而言并不是好设计（全局状态、单测不便、多实例冲突）。

开源时我们把全局根目录做成了**可选特征 `global-root`（默认关闭）**：不启用时包内没有任何全局状态，`init_config_path`/`config_path` API 不存在，未覆写 `ROOT` 的实现调用无根方法会明确返回 `ConfigError::NotInitialized`。需要兼容旧用法时显式启用特征即可。**新项目建议保持默认（不开特征），直接使用 `ROOT` 覆写或 `*_at` 方法**。

## 特性

- **声明式** — 一个 trait、三个关联常量，模板方法全部自带默认实现
- **默认值合并** — 加载时先与 `Default` 合并，配置文件缺字段自动补齐（升级不炸）
- **未知字段保留** — 保存时与磁盘旧文件合并，用户手写的字段和注释不丢
- **机密自动拆分** — `SECRETS` 声明的字段写入 `.secrets/<NAME>.toml`，主文件留占位符；支持 `auth.password` 嵌套与 `servers.*.api_key` 通配
- **路径安全** — 目录穿越（`../`）与非法文件名直接报错
- 纯同步、无 `unsafe`

## 安装

```toml
[dependencies]
qexed_config = "0.1"

# 如需兼容历史的进程级全局根目录（init_config_path / config_path）：
# qexed_config = { version = "0.1", features = ["global-root"] }
```

## 快速上手

### 根目录的三种方式

**方式一（遗留，需 `global-root` 特征）**：全局初始化一次，所有 Config 共享

```rust
qexed_config::init_config_path("./config".into())?;
```

**方式二（推荐）**：每个实现自带根目录常量（任何特征下可用）

```rust
impl Config for LogConfig {
    const ROOT: Option<&'static str> = Some("./my_config");  // 不再依赖全局初始化
    const PATH: &'static str = "log";
    const NAME: &'static str = "log";
    const SECRETS: &'static [&'static str] = &[];
}
```

**方式三（运行期动态）**：`*_at` 方法族，传任意根目录

```rust
let cfg = LogConfig::load_and_create_default_at(&dynamic_dir, false)?;
```

### 完整示例

```rust
use qexed_config::Config;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    /// 机密字段：值会进 .secrets/database.toml
    password: String,
}

impl Config for DatabaseConfig {
    const PATH: &'static str = "database";       // → <root>/database/database.toml
    const NAME: &'static str = "database";       // 不用写 .toml 后缀
    const SECRETS: &'static [&'static str] = &["password"];
}

fn main() -> Result<(), qexed_config::error::ConfigError> {
    // 遗留方式需要；若所有实现都覆写了 ROOT 或用 *_at，可跳过
    qexed_config::init_config_path("./config".into())?;

    // 不存在则用默认值创建；存在则加载（缺字段补默认，未知字段保留）
    let mut cfg = DatabaseConfig::load_and_create_default(true)?;

    cfg.port = 5432;
    DatabaseConfig::save_file(&cfg)?;   // password 自动拆到 .secrets/

    Ok(())
}
```

### 磁盘布局

```
<root>/
└── database/
    ├── database.toml          # host/port + password = "<stored in .secrets>"
    └── .secrets/
        └── database.toml      # password = "真实值"（记得加 .gitignore）
```

## API 一览

| 项 | 说明 |
| --- | --- |
| `Config` trait | `ROOT`/`PATH`/`NAME`/`SECRETS` 四个常量 + 模板方法 |
| `load_and_create_default(save)` | 不存在则建默认文件，加载后可选回存 |
| `load_file` / `save_file` / `create_file` | 读/写/建，自动处理 secrets 拆合 |
| `*_at(base, ...)` 系列 | 上述方法的显式根目录版本（任何特征下可用） |
| `to_toml_string` | 序列化为 TOML 字符串（调试用） |
| `init_config_path` / `config_path` | 遗留全局根目录（需 `global-root` 特征） |
| `config_file_at` | 任意根目录下的安全路径推导 |

## 机密规则语法

```text
"token"                 # 顶层字段
"auth.password"         # 嵌套表
"servers.*.api_key"     # 数组每项的同名字段
```

"一旦机密，永远是机密"：保存时若 `.secrets/` 里已存在某字段的历史值，该字段继续视为机密（即使 `SECRETS` 里删掉了）。

## MSRV

Rust 1.85+（edition 2024）。

## License

MIT — 见 [LICENSE](./LICENSE)。
