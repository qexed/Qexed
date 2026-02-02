# Qexed | 量子叠加态 (Quantum Existence State)
[![Rust](https://img.shields.io/badge/Rust-1.91+-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/Status-Developing-yellow)]()

[English](./readme.md)|[简体中文](./readme-cn.md)
一个使用 Rust 编写的低性能 Minecraft: Java Edition 服务端，致力于提供现代化、可扩展且稳定的游戏服务器体验。

> ⚠️ **项目处于积极开发阶段**，API 与功能可能发生变动。

## ✨ 特性

*   **高性能核心**: 利用 Rust 的安全性与并发特性，构建稳定高效的服务器核心。
*   **模块化架构**: 插件系统设计，便于功能扩展与社区贡献。
*   **内置国际化 (i18n)**: 服务端界面与日志支持多语言。
*   **现代化配置**: 采用 TOML 格式的清晰配置系统。

## 🌍 多语言支持 (Internationalization)

Qexed 服务端原生集成了国际化支持，目前正式支持以下语言：

| 语言 | Locale 代码 | 状态 |
| :--- | :--- | :--- |
| **简体中文** | `zh-CN` | ✅ 完全支持 |
| **英文 (English)** | `en` | ✅ 完全支持 |

我们使用 [`rust-i18n`](https://crates.io/crates/rust-i18n) 框架管理翻译。所有用户可见的字符串（如控制台输出、日志、配置说明）均已抽取并存储在 `locales/` 目录下的翻译文件中。

### 🤝 欢迎贡献更多语言！

我们非常欢迎并感谢社区为 Qexed 添加新的语言支持。

1.  **Fork** 本仓库。
2.  在 `locales/i18n.toml` 文件中，为您要添加的语言（例如 `ja-JP` 日语）创建新的翻译区块。
3.  使用 `cargo i18n` 工具检查并补全缺失的翻译键。
4.  提交 Pull Request (PR)。

您的贡献将使更多玩家能够使用母语运行和管理 Qexed 服务器！

## 🚀 快速开始

### 前提条件
*   [Rust 工具链](https://www.rust-lang.org/tools/install) (版本 1.91 或更高)
*   Java (用于运行 Minecraft 客户端)

### 构建与运行
```sh
cargo build --bin qexed
cargo run
```