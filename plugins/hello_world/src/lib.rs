use qexed_plugin_sdk::*;

static META: PluginMeta = PluginMeta {
    id: "hello_world",
    name: "Hello World",
    version: "0.1.0",
    dependencies: &[],
};

#[derive(Default)]
pub struct HelloWorldPlugin;

// -- Plugin --

#[async_trait]
impl Plugin for HelloWorldPlugin {
    fn meta(&self) -> &PluginMeta {
        &META
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn on_load(&mut self, api: &dyn HostApi) -> Result<(), PluginError> {
        api.log(LogLevel::Info, "HelloWorld 插件已加载");
        Ok(())
    }

    async fn on_enable(&mut self, api: &dyn HostApi) -> Result<(), PluginError> {
        api.log(LogLevel::Info, "Hello, World!");

        // 订阅 PluginLoadFinish 事件
        let my_handle = api
            .get_plugin_handle("hello_world")
            .expect("找不到自己的 handle");
        api.register::<PluginLoadFinishEvent>(my_handle, Box::new(LoadFinishHandler));

        Ok(())
    }

    fn on_disable(&mut self) -> Result<(), PluginError> {
        eprintln!("HelloWorld 插件已卸载");
        Ok(())
    }
}

// -- 事件处理器 --

struct LoadFinishHandler;

#[async_trait]
impl EventHandler<PluginLoadFinishEvent> for LoadFinishHandler {
    async fn handle(
        &self,
        api: &dyn HostApi,
        _event: &PluginLoadFinishEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            LogLevel::Info,
            "HelloWorld 收到 PluginLoadFinishEvent —— 所有插件加载完成！",
        );
        Ok(())
    }
}

declare_plugin!(HelloWorldPlugin);
