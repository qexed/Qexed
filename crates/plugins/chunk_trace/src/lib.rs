//! chunk_trace：v4 示例移植 —— 区块加载/卸载日志。
//!
//! 注：v6 宿主尚未触发 ChunkLoadEvent/ChunkUnloadEvent，插件已就绪，
//! 宿主接线后自动生效。

use qexed_plugin_api::{ChunkLoadEvent, ChunkUnloadEvent, LogLevel};
use qexed_plugin_sdk::{InitFuture, PluginContext};

qexed_plugin_sdk::declare_simple_plugin!("chunk_trace", "Chunk Trace", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        ctx.register::<ChunkLoadEvent>(|api, ev, _| {
            Box::pin(async move {
                api.log(
                    LogLevel::Info,
                    &format!(
                        "chunk_load: dimension={}, chunk=({}, {})",
                        ev.chunk.dimension, ev.chunk.chunk_x, ev.chunk.chunk_z
                    ),
                );
            })
        });
        ctx.register::<ChunkUnloadEvent>(|api, ev, _| {
            Box::pin(async move {
                api.log(
                    LogLevel::Info,
                    &format!(
                        "chunk_unload: dimension={}, chunk=({}, {})",
                        ev.chunk.dimension, ev.chunk.chunk_x, ev.chunk.chunk_z
                    ),
                );
            })
        });
    })
});
