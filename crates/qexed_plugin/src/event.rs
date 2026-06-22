use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use qexed_plugin_api::{
    ErasedHandler, Event, EventBusApi, EventContext, EventEmitResult, EventRegistry, HostApi,
    PluginHandle,
};

use crate::manager::PluginManager;

pub(crate) type HandlerMap = HashMap<&'static str, Vec<(PluginHandle, ErasedHandler)>>;

impl EventRegistry for PluginManager {
    fn register_event(
        &self,
        event_name: &'static str,
        handle: PluginHandle,
        handler: ErasedHandler,
    ) {
        let mut map = self.event_handlers.write().unwrap();
        map.entry(event_name).or_default().push((handle, handler));
    }
}

impl EventBusApi for PluginManager {
    fn emit_event<'a>(
        &'a self,
        event_name: &'static str,
        event: &'a (dyn std::any::Any + Send + Sync),
    ) -> Pin<Box<dyn Future<Output = EventEmitResult> + Send + 'a>> {
        Box::pin(async move { self.emit_erased(event_name, event).await })
    }
}

impl PluginManager {
    /// 触发事件，并返回取消状态和处理器错误。
    pub async fn fire<E: Event>(&self, event: &E) -> EventEmitResult {
        self.emit_erased(std::any::type_name::<E>(), event).await
    }

    async fn emit_erased(
        &self,
        event_name: &'static str,
        event: &(dyn std::any::Any + Send + Sync),
    ) -> EventEmitResult {
        let ctx = EventContext::new();
        let futures: Vec<_> = {
            let map = self.event_handlers.read().unwrap();
            let host: &dyn HostApi = self;
            map.get(event_name)
                .map(|handlers| {
                    handlers
                        .iter()
                        .map(|(_, handler)| handler(host, event_name, event, &ctx))
                        .collect()
                })
                .unwrap_or_default()
        };

        let errors = futures::future::join_all(futures)
            .await
            .into_iter()
            .filter_map(Result::err)
            .collect();

        EventEmitResult::new(ctx.is_cancelled(), errors)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use qexed_plugin_api::{
        EventContext, EventHandler, EventRegistryExt, PluginError, PluginLoadFinishEvent,
        async_trait,
    };

    use super::*;

    struct CountingHandler {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventHandler<PluginLoadFinishEvent> for CountingHandler {
        async fn handle(
            &self,
            _api: &dyn HostApi,
            _event: &PluginLoadFinishEvent,
            _ctx: &EventContext,
        ) -> Result<(), PluginError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    #[tokio::test]
    async fn fire_dispatches_registered_handler() {
        let manager = PluginManager::new();
        let calls = Arc::new(AtomicUsize::new(0));

        manager.register::<PluginLoadFinishEvent>(
            PluginHandle(0),
            Box::new(CountingHandler {
                calls: calls.clone(),
            }),
        );

        let result = manager.fire(&PluginLoadFinishEvent).await;

        assert!(result.is_ok());
        assert!(!result.is_cancelled());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
