use qexed_plugin_sdk::*;

static META: PluginMeta = PluginMeta {
    id: "event_subscriber",
    name: "Event Subscriber Example",
    version: "0.1.0",
    dependencies: &[],
};

#[derive(Default)]
pub struct EventSubscriberPlugin;

#[async_trait]
impl Plugin for EventSubscriberPlugin {
    fn meta(&self) -> &PluginMeta {
        &META
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn on_enable(&mut self, api: &dyn HostApi) -> Result<(), PluginError> {
        let handle = api
            .get_plugin_handle(META.id)
            .ok_or_else(|| PluginError::new("event_subscriber handle 未注册"))?;

        api.register::<PluginLoadFinishEvent>(handle, Box::new(LoadFinishHandler));
        api.register::<PlayerMoveEvent>(handle, Box::new(PlayerMoveHandler));
        api.register::<PlayerBlockInteractEvent>(handle, Box::new(PlayerBlockInteractHandler));
        api.register::<ChunkSyncEvent>(handle, Box::new(ChunkSyncHandler));
        api.log(
            LogLevel::Info,
            "EventSubscriber 已订阅插件加载、玩家移动、方块交互、区块同步事件",
        );

        Ok(())
    }
}

struct LoadFinishHandler;

#[async_trait]
impl EventHandler<PluginLoadFinishEvent> for LoadFinishHandler {
    async fn handle(
        &self,
        api: &dyn HostApi,
        _event: &PluginLoadFinishEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(LogLevel::Info, "EventSubscriber 收到插件加载完成事件");
        Ok(())
    }
}

struct PlayerMoveHandler;

#[async_trait]
impl EventHandler<PlayerMoveEvent> for PlayerMoveHandler {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &PlayerMoveEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            LogLevel::Debug,
            &format!(
                "玩家 {} 从 ({:.2}, {:.2}, {:.2}) 移动到 ({:.2}, {:.2}, {:.2})",
                event.player.username,
                event.previous_position.x,
                event.previous_position.y,
                event.previous_position.z,
                event.position.x,
                event.position.y,
                event.position.z,
            ),
        );
        Ok(())
    }
}

struct PlayerBlockInteractHandler;

#[async_trait]
impl EventHandler<PlayerBlockInteractEvent> for PlayerBlockInteractHandler {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &PlayerBlockInteractEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            LogLevel::Debug,
            &format!(
                "玩家 {} 使用 {:?} 点击方块 ({}, {}, {}) 面 {:?}，sequence={}",
                event.player.username,
                event.hand,
                event.hit.position.x,
                event.hit.position.y,
                event.hit.position.z,
                event.hit.face,
                event.sequence,
            ),
        );
        Ok(())
    }
}

struct ChunkSyncHandler;

#[async_trait]
impl EventHandler<ChunkSyncEvent> for ChunkSyncHandler {
    async fn handle(
        &self,
        api: &dyn HostApi,
        event: &ChunkSyncEvent,
        _ctx: &EventContext,
    ) -> Result<(), PluginError> {
        api.log(
            LogLevel::Debug,
            &format!(
                "区块同步 {:?} center=({}, {}) loaded={} unloaded={} unloading={}",
                event.cause,
                event.center_chunk_x,
                event.center_chunk_z,
                event.loaded_count(),
                event.unloaded_count(),
                event.unloading_count(),
            ),
        );
        Ok(())
    }
}

declare_plugin!(EventSubscriberPlugin);
