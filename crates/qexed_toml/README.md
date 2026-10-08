# qexed_toml

[![Crates.io](https://img.shields.io/crates/v/qexed_toml.svg)](https://crates.io/crates/qexed_toml)
[![Documentation](https://docs.rs/qexed_toml/badge.svg)](https://docs.rs/qexed_toml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

基于 `toml_edit` 的 TOML 工具库：**保格式深度合并**、文档级 serde、以及**机密字段拆分**（主配置进 git，token 落 `.secrets/`）。

从 [Qexed](https://github.com/qexed/Qexed) 项目拆分出的独立模块，可脱离 workspace 单独使用。

## 特性

- **保格式合并（`merge`）** — 递归合并两个 `DocumentMut`：表逐键合并、数组逐项合并，未变更的注释与排版原样保留，适合"配置文件升级不丢用户手写内容"的场景。
- **机密字段拆分（`split_secrets` / `merge_secrets`）** — 按点路径规则把机密字段从主文档抽到独立文档，主文档留 `<stored in .secrets>` 占位符。支持嵌套路径（`auth.password`）与数组通配符（`servers.*.api_key`）；配合旧 secrets 文档可实现"一旦机密，永远是机密"。
- **文档级 serde** — `to_document` / `from_document` 在 `DocumentMut` 与任意 serde 类型之间转换（`toml_edit::ser/de` 的直通封装）。
- **文件读写** — `create_file`（已存在则报错）/ `load_file`（不存在则报错）/ `save_file`（不存在则创建）/`has_file`，路径校验明确区分"是文件夹""不存在"等情况。
- 纯同步、无 `unsafe`，仅依赖 `toml_edit` + `serde` + `thiserror`。

## 安装

```toml
[dependencies]
qexed_toml = "0.1"
```

## 快速上手

### 深度合并（保留旧文件里"不认识"的字段与注释）

```rust
use qexed_toml::merge;

let old: DocumentMut = r#"
name = "server"

[net]
port = 25565  # 用户手写注释
"#.parse()?;

let new: DocumentMut = r#"
name = "server"

[net]
port = 25565
host = "0.0.0.0"   # 新版本新增字段
"#.parse()?;

let merged = merge(&old, &new)?;
// host 被补上；"用户手写注释" 保留
```

### 机密拆分：主文件可提交 git，token 落 .secrets/

```rust
use qexed_toml::{split_secrets, merge_secrets};

let doc: DocumentMut = r#"
name = "server"

[auth]
user = "alice"
password = "hunter2"
"#.parse()?;

// 规则：点路径嵌套，* 匹配数组每一项
let rules = ["auth.password", "servers.*.api_key"];

let (main, secrets) = split_secrets(&doc, &rules, None)?;
// main:     password = "<stored in .secrets>"
// secrets:  password = "hunter2"

// 保存：main 写 config.toml，secrets 写 .secrets/config.toml
// 读取：先 load main，再 merge_secrets(&main, &secrets) 回填真值
let restored = merge_secrets(&main, &secrets)?;
```

### serde 类型 ⇄ 文档

```rust
use qexed_toml::{to_document, from_document};

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Config { name: String, port: u16 }

let doc = to_document(&Config::default())?;
let cfg: Config = from_document(doc)?;
```

## API 一览

| 函数 | 说明 |
| --- | --- |
| `merge(old, new)` | 递归合并，new 覆盖 old，保注释保排版 |
| `split_secrets(doc, rules, old_secret)` | 拆为主文档（占位符）+ 机密文档；`old_secret` 里的历史路径继续视为机密 |
| `merge_secrets(main, secret)` | 把机密文档回填进主文档（当前是 `merge` 的别名） |
| `to_document(t)` / `from_document(doc)` | serde 类型与 `DocumentMut` 互转 |
| `create_file` / `load_file` / `save_file` / `has_file` | 带"已存在/不存在/是目录"明确错误的文件读写 |

## 与 qexed_config 的关系

[`qexed_config`](https://crates.io/crates/qexed_config) 是本库之上的配置管理框架（trait 模板方法 + 目录约定 + 自动 secrets 落盘）。只需要合并/拆分原语时用 `qexed_toml`；想要"实现个 trait 就自动拥有读写与机密管理"时用 `qexed_config`。

## MSRV

Rust 1.85+（edition 2024）。

## License

MIT — 见 [LICENSE](./LICENSE)。
