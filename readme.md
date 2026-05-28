# Qexed | Quantum Existence State
[![Rust](https://img.shields.io/badge/Rust-1.91+-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/Status-Developing-yellow)]()

[English](./readme.md)|[简体中文](./readme-cn.md)

A low-performance Minecraft: Java Edition server written in Rust, dedicated to providing a modern, extensible, and stable game server experience.

> ⚠️ **Project is under active development**, APIs and features are subject to change.

## ✨ Features

*   **High-performance core**: Leverage Rust's safety and concurrency features to build a stable and efficient server core.
*   **Modular architecture**: Plugin system design for easy feature extension and community contribution.
*   **Built-in Internationalization (i18n)**: Server interface and logs support multiple languages.
*   **Modern configuration**: Clear configuration system using TOML format.

## 🌍 Internationalization Support

The Qexed server natively integrates internationalization support. The following languages are currently officially supported:

| Language | Locale Code | Status |
| :--- | :--- | :--- |
| **Simplified Chinese(简体中文)** | `zh-CN` | ✅ Fully supported |
| **English** | `en` | ✅ Fully supported |

We use the [`rust-i18n`](https://crates.io/crates/rust-i18n) framework to manage translations. All user-visible strings (such as console output, logs, configuration descriptions) are extracted and stored in translation files in the `locales/` directory.
## Plugin Support
Although Qexed does not support nms, you can still write plugins using alternative methods. Qexed uses a plugin system based on WASM for its operation, which means you can develop plugins using the language you prefer (if your language is not supported by WASM, just ignore this note)

| Language | Status | Remarks | 
| --- | --- | --- |
| Rust | ❌ Under development | |
| C++ | ❌ Not supported | The Rust SDK has not been completed yet |
| Golang | ❌ Not supported | The Rust SDK has not been completed yet |
| Python | ❌ Not supported | I will support it if you compile it into wasm for me |
### 🤝 Welcome to contribute more languages!

We warmly welcome and appreciate the community to add new language support for Qexed.

1.  **Fork** this repository.
2.  Create a new translation block in the `locales/i18n.toml` file for the language you want to add (e.g., `ja-JP` for Japanese).
3.  Use the `cargo i18n` tool to check and complete missing translation keys.
4.  Submit a Pull Request (PR).

Your contribution will allow more players to run and manage Qexed servers in their native language!

## 🚀 Quick Start

### Prerequisites
*   [Rust toolchain](https://www.rust-lang.org/tools/install) (version 1.91 or higher)
*   Java (for running Minecraft client)

### Build and Run
```sh
cargo build --bin qexed
cargo run
```

### Cross Compile (cross)
Qexed depends on GPU (`wgpu`/Vulkan backend) and OpenSSL. The repository provides a `Cross.toml` for common Linux targets.

```sh
cargo install cross --git https://github.com/cross-rs/cross
cross build -p qexed --target x86_64-unknown-linux-gnu
cross build -p qexed --target aarch64-unknown-linux-gnu
```

If the target machine may not have GPU support, keep CPU-only build (default):

```sh
cross build -p qexed --target x86_64-unknown-linux-musl
```

Enable GPU acceleration only when needed:

```sh
cross build -p qexed --target x86_64-unknown-linux-gnu --features gpu
```
