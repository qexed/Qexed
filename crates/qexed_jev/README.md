# qexed_jev

jev 模型推理网关：**默认内嵌服务**（进程内专用 worker 线程推理），可切远端
OpenAI 兼容 HTTP（`features = ["remote"]`）。

## 定位与依赖方向

```text
qexed_server → qexed_play → qexed_chat      （聊天：协议 / 签名 / 限速）
           └────────────→ qexed_jev         （AI 回复：内嵌 jev 推理 / 远端 HTTP）
```

叶子 crate：不依赖任何 qexed_* 运行时 crate。**qexed_chat 不反向依赖本 crate**，
聊天 crate 保持协议职责纯净；AI 回复由 qexed_play / qexed_server 层组合调用。

## 性能设计（默认内嵌模式）

| 旋钮 | 默认 | 作用 |
| --- | --- | --- |
| `embedded.queue_capacity` | 32 | 有界队列硬背压：满了立刻返回 `JevError::QueueFull`，内存有上界 |
| `embedded.micro_batch` | 4 | worker 把就绪请求凑批喂引擎，提升真实权重下的加速器吞吐 |
| `embedded.request_timeout_ms` | 10000 | 排队 + 推理总时长上限，超时快速失败 |
| `embedded.thread_priority_idle` | true | 推理线程让出调度优先级，避免 Minecraft tick 抖动 |
| `embedded.model_path` | "" | 真实 jev 权重路径；当前未接加载器时回退内置回声引擎 |

关键点：推理跑在**专用 OS 线程**（`qexed-jev-worker`），不阻塞 tokio runtime 的
worker 线程 —— 玩家网络包处理不受推理耗时影响；提交侧 `try_send` 永不阻塞。

## 真实 jev 权重接入点

`src/engine/mod.rs` 的 `build_embedded_engine` 是唯一需要改动的地方：按 `model_path`
扩展名分发到 GGUF / safetensors 加载器并实现 `JevEngine` trait 即可，
服务/会话/背压/微批管线全部复用。

## qexed_chat 接入步骤

1. `qexed_server` 启动时（`server::init`）加载服务：

```rust,ignore
let jev = qexed_jev::JevService::init()?;   // 读取 config/jev.toml（自动创建）
// ServerContext 增加 jev: Arc<qexed_jev::JevService>
```

2. `qexed_play` 的聊天分发处（`dispatch` 的 `ChatMessage::ID` 分支）：

```rust,ignore
if jev.enabled() {
    let mut conv = jev.conversation_with_system(
        player.uuid_string(), Some(NPC_SYSTEM_PROMPT.into()));
    match conv.ask(p.message.clone()).await {
        Ok(reply) => send_system(sink, reply.text).await?,
        Err(qexed_jev::JevError::QueueFull { .. }) => {
            send_system(sink, "jev is busy, try again shortly.").await?
        }
        Err(err) => { log::warn!("jev failed: {err}"); }
    }
}
```

3. 配置（`config/jev.toml`，默认生成即内嵌模式）：

```toml
enable = true
mode = "Embedded"          # 或 "Remote"（需 cargo build --features qexed_jev/remote）
max_history_turns = 8

[embedded]
model_path = ""
queue_capacity = 32
micro_batch = 4
request_timeout_ms = 10000
thread_priority_idle = true

[remote]
base_url = "http://127.0.0.1:8080/v1"
model = "jev"
api_key = "<stored in .secrets>"   # 自动落到 config/.secrets/jev.toml
timeout_ms = 10000
```

## 测试

```bash
cargo test -p qexed_jev
```
