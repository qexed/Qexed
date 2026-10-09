# qexed_forge

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

让 Minecraft **Forge 客户端**能连上 qexed（或任意 vanilla 兼容服务端）的线协议支持：
FML 登录握手（`forge:handshake` 登录插件通道）、mod 列表协商、CONFIGURATION 阶段的 forge 通道帧。

## 许可证立场（重要）

本 crate 是 **MIT**。Minecraft Forge 本体是 **LGPLv2.1**（部分组件 GPLv3 生态）：

- 本实现是**干净房间（clean-room）实现**：只实现客户端与服务端之间线上传输的字节格式（协议本身），
  不包含、不引用、不链接任何 Forge 代码或资源
- 网络协议的互操作不构成衍生作品——正如 MIT 的 HTTP 服务器可以和 GPL 的浏览器对话
- qexed 主仓与全部 crate 保持 MIT，不被污染

## FML 登录握手

Forge 客户端在 LOGIN 阶段等待服务器发起 `forge:handshake` 登录插件请求，否则会断开或按 vanilla 流程走。
握手帧装在 vanilla `Login Plugin Request/Response` 的数据字段里，格式为 `discriminator(u8) + 消息体`：

| Discriminator | 消息 | 内容 |
| --- | --- | --- |
| `0x00` | ServerHello | FML 网络版本 varint |
| `0x01` | ClientModList | count varint + (modid string + version string)* |
| `0x02` | ServerModList | 同上 |
| `0x03` | Ack | 空 |

### 服务器侧流程

```rust,ignore
use qexed_forge::handshake::{self, ModList, ServerHandshake};

// 1. LOGIN 阶段：login_start 之后发 login_plugin_request
//    channel = "forge:handshake", data = handshake::encode_server_hello(3)

// 2. 收到 login_plugin_response 后交给状态机：
let mut hs = ServerHandshake::new(ModList::default());
match hs.on_client_response(&response_data)? {
    Some(next_frame) => { /* 装进新的 login_plugin_request 发回 */ }
    None => { /* 握手完成，发 login_success */ }
}

// 可选：校验客户端 mod 与服务器需求的兼容性
hs.client_mods().unwrap().check_compatibility(&required_mods)?;
```

### 兼容性语义

`ModList::check_compatibility`：客户端必须包含服务器要求的全部 mod；客户端多出的 mod（纯客户端 mod，如 JEI/minimap）放行。

## CONFIGURATION 阶段通道

登录完成后 Forge 在 CONFIGURATION 阶段还有 `forge:login`（registry 协商载体）与 `forge:channeldata`（通道转发）：

```rust
use qexed_forge::channels::ForgeLoginMessage;

let msg = ForgeLoginMessage::decode(&payload)?;
match msg {
    ForgeLoginMessage::Ready { channels } => { /* 客户端就绪的通道表 */ }
    ForgeLoginMessage::RegistryData { payload } => { /* registry 同步数据 */ }
    ForgeLoginMessage::Ack => { /* 完成 */ }
    _ => {}
}
```

## 未实现（诚实边界）

- **registry 同步内容**：Forge 会同步方块/物品注册表差异（NBT 大对象）。对“接受 Forge 客户端进入 vanilla 语义世界”的场景不需要；要做完整 mod 服务器时需按 Forge 的 registry 格式实现
- **需要真实 Forge 客户端联调**：当前测试覆盖编解码往返与状态机；字节格式来自协议文档的干净房间实现，首次接真实客户端时可能需要微调（欢迎提 issue 附抓包）

## MSRV

Rust 1.85+（edition 2024）。

## License

MIT — 见 [LICENSE](./LICENSE)。
