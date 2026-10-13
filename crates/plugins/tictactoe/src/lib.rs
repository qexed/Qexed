//! tictactoe：v4 移植（骨架，待填充）。

use qexed_plugin_sdk::{InitFuture, PluginContext};

qexed_plugin_sdk::declare_simple_plugin!("tictactoe", "tictactoe", "0.1.0", |ctx: PluginContext| -> InitFuture {
    Box::pin(async move {
        ctx.log(qexed_plugin_api::LogLevel::Info, "tictactoe skeleton");
    })
});
