# qexed_nbt

[![Crates.io](https://img.shields.io/crates/v/qexed_nbt.svg)](https://crates.io/crates/qexed_nbt)
[![Documentation](https://docs.rs/qexed_nbt/badge.svg)](https://docs.rs/qexed_nbt)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Minecraft Java Edition NBT（Named Binary Tag）读写库：树模型 + Java 大端序编解码 + Serde 支持。

`qexed_nbt` 是 [Qexed](https://github.com/qexed/Qexed) 项目拆分出的独立 NBT 模块，可脱离 workspace 单独使用。

## 特性

- **完整 NBT 树模型** — `Tag` 枚举覆盖 Java 版全部 13 种标签类型（Byte/Short/Int/Long/Float/Double/String/ByteArray/IntArray/LongArray/List/Compound/End），Compound 与 List 内部使用 `Arc` 共享缓冲，克隆开销极低。
- **磁盘格式（named NBT）** — 根节点带类型 ID 与名称（如 `level.dat`、`playerdata/*.dat`），读取时自动检测并解压 gzip；写入时可选 gzip 压缩。
- **网络格式（network NBT）** — 根节点无名称（Java 版协议包中使用），支持空值（`Tag::End`）。
- **Serde 支持** — 任意 `Serialize`/`Deserialize` 类型与 `Tag` 之间互转，NBT 结构可直接映射到 Rust 结构体。
- **零 panic 解码** — 所有解析错误通过 `NbtError`（基于 `thiserror`）返回，包含类型不匹配、非法 UTF-8、未知标签 ID 等具体信息。
- **无 unsafe**。

## 安装

```toml
[dependencies]
qexed_nbt = "0.1"
```

## 快速上手

### 构建 NBT 树

```rust
use qexed_nbt::{Tag, tag_id};
use std::collections::HashMap;
use std::sync::Arc;

let mut player = HashMap::new();
player.insert("Health".to_string(), Tag::Float(20.0));
player.insert("Name".to_string(), Tag::String(Arc::from("Steve")));
player.insert(
    "Pos".to_string(),
    Tag::new_list(
        tag_id::DOUBLE,
        vec![Tag::Double(100.5), Tag::Double(64.0), Tag::Double(-200.0)],
    )?,
);
let root = Tag::Compound(Arc::new(player));
```

### 读写 .dat 文件（named 格式，自动 gzip 检测）

```rust
use qexed_nbt::{from_file, to_file};

// 读取：gzip 与未压缩文件自动识别
let (root_name, root_tag) = from_file("level.dat")?;

// 写入：第四个参数控制是否 gzip 压缩
to_file("level.dat", &root_name, &root_tag, true)?;
```

### 内存中的 named 编解码

```rust
use qexed_nbt::{from_slice, to_vec};

let bytes = to_vec("root", &tag)?;
let (name, tag) = from_slice(&bytes)?;
```

### 网络 NBT（Java 协议，根无名称）

```rust
use qexed_nbt::net::NetNbtIo;

// 空值编码为单个 0x00 字节
let mut buf = Vec::new();
NetNbtIo::to_writer(&mut buf, &tag, false)?;

let mut cursor = std::io::Cursor::new(buf);
let tag = NetNbtIo::from_reader(&mut cursor, false)?;
```

### Serde：结构体 ⇄ NBT

```rust
use qexed_nbt::nbt_serde::nbt_serde::{to_tag, from_tag};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Item {
    id: String,
    count: i32,
}

let item = Item { id: "minecraft:diamond".into(), count: 64 };
let tag = to_tag(&item)?;          // Rust 值 -> Tag
let item2: Item = from_tag(&tag)?; // Tag -> Rust 值
```

## 示例

仓库 `examples/` 目录包含完整可运行示例：

| 示例 | 说明 |
| --- | --- |
| `basic` | 构建 Tag 树并用 serde_json 打印调试视图 |
| `read_dat` | 读取真实 `level.dat`，打印树结构与统计信息 |
| `rewrite_test` | 读取 .dat 后重新写回并验证一致性 |

```bash
cargo run --example read_dat -- path/to/level.dat
cargo run --example rewrite_test -- path/to/player.dat --verify
```

## 格式差异：named vs network

| | named（磁盘） | network（协议） |
| --- | --- | --- |
| 根节点 | 类型 ID + 名称字符串 + Compound | 类型 ID + Compound（无名称） |
| 空值 | 不支持 | `Tag::End` → 单字节 `0x00` |
| 典型场景 | `level.dat`、`playerdata/*.dat` | Java 版协议包中的 NBT 载荷 |
| 压缩 | 可选 gzip（读取时自动检测） | 无 |

## MSRV

Rust 1.85+（edition 2024）。

## License

MIT — 见 [LICENSE](./LICENSE)。
