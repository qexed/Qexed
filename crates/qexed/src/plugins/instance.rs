use std::{collections::HashSet, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

use super::{
    PluginEvent, PluginState,
    host::{
        PluginHostServices, host_config_exists, host_config_read, host_config_write,
        host_economy_async_forget, host_economy_async_poll, host_economy_balance,
        host_economy_balance_async, host_economy_currency_info, host_economy_deposit,
        host_economy_deposit_async, host_economy_register_currency, host_economy_set_balance,
        host_economy_set_balance_async, host_economy_storage, host_economy_withdraw,
        host_economy_withdraw_async, host_entity_move, host_entity_remove, host_entity_upsert,
        host_log, host_lottery_roll, host_pathfinding_find, host_plugin_call,
        host_plugin_service_exists, host_random_pool_roll, host_storage_delete,
        host_storage_exists, host_storage_get, host_storage_set,
        host_structured_storage_async_forget, host_structured_storage_async_poll,
        host_structured_storage_delete, host_structured_storage_delete_async,
        host_structured_storage_exists, host_structured_storage_exists_async,
        host_structured_storage_get, host_structured_storage_get_async,
        host_structured_storage_set, host_structured_storage_set_async, host_time_millis,
        host_world_break_block, host_world_register_edit_region, host_world_set_block,
        host_world_set_blocks,
    },
};
use qexed_plugin_api::PluginManifest;

const MAX_EVENT_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_QUERY_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) struct PluginInstance {
    pub(super) name: String,
    pub(super) manifest: PluginManifest,
    pub(super) priority: i32,
    supported_events: HashSet<PluginEvent>,
    store: Store<PluginState>,
    instance: Instance,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    dealloc: Option<TypedFunc<(i32, i32), ()>>,
}

fn register_host_apis(linker: &mut Linker<PluginState>) -> Result<()> {
    linker
        .func_wrap("qexed", "config_exists", host_config_exists)
        .context("register plugin config_exists API")?;
    linker
        .func_wrap("qexed", "config_read", host_config_read)
        .context("register plugin config_read API")?;
    linker
        .func_wrap("qexed", "config_write", host_config_write)
        .context("register plugin config_write API")?;
    linker
        .func_wrap("qexed", "storage_exists", host_storage_exists)
        .context("register plugin storage_exists API")?;
    linker
        .func_wrap("qexed", "storage_get", host_storage_get)
        .context("register plugin storage_get API")?;
    linker
        .func_wrap("qexed", "storage_set", host_storage_set)
        .context("register plugin storage_set API")?;
    linker
        .func_wrap("qexed", "storage_delete", host_storage_delete)
        .context("register plugin storage_delete API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_exists",
            host_structured_storage_exists,
        )
        .context("register plugin structured_storage_exists API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_get",
            host_structured_storage_get,
        )
        .context("register plugin structured_storage_get API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_set",
            host_structured_storage_set,
        )
        .context("register plugin structured_storage_set API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_delete",
            host_structured_storage_delete,
        )
        .context("register plugin structured_storage_delete API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_exists_async",
            host_structured_storage_exists_async,
        )
        .context("register plugin structured_storage_exists_async API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_get_async",
            host_structured_storage_get_async,
        )
        .context("register plugin structured_storage_get_async API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_set_async",
            host_structured_storage_set_async,
        )
        .context("register plugin structured_storage_set_async API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_delete_async",
            host_structured_storage_delete_async,
        )
        .context("register plugin structured_storage_delete_async API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_async_poll",
            host_structured_storage_async_poll,
        )
        .context("register plugin structured_storage_async_poll API")?;
    linker
        .func_wrap(
            "qexed",
            "structured_storage_async_forget",
            host_structured_storage_async_forget,
        )
        .context("register plugin structured_storage_async_forget API")?;
    linker
        .func_wrap(
            "qexed",
            "economy_register_currency",
            host_economy_register_currency,
        )
        .context("register plugin economy_register_currency API")?;
    linker
        .func_wrap("qexed", "economy_currency_info", host_economy_currency_info)
        .context("register plugin economy_currency_info API")?;
    linker
        .func_wrap("qexed", "economy_storage", host_economy_storage)
        .context("register plugin economy_storage API")?;
    linker
        .func_wrap("qexed", "economy_balance", host_economy_balance)
        .context("register plugin economy_balance API")?;
    linker
        .func_wrap("qexed", "economy_set_balance", host_economy_set_balance)
        .context("register plugin economy_set_balance API")?;
    linker
        .func_wrap("qexed", "economy_deposit", host_economy_deposit)
        .context("register plugin economy_deposit API")?;
    linker
        .func_wrap("qexed", "economy_withdraw", host_economy_withdraw)
        .context("register plugin economy_withdraw API")?;
    linker
        .func_wrap("qexed", "economy_balance_async", host_economy_balance_async)
        .context("register plugin economy_balance_async API")?;
    linker
        .func_wrap(
            "qexed",
            "economy_set_balance_async",
            host_economy_set_balance_async,
        )
        .context("register plugin economy_set_balance_async API")?;
    linker
        .func_wrap("qexed", "economy_deposit_async", host_economy_deposit_async)
        .context("register plugin economy_deposit_async API")?;
    linker
        .func_wrap(
            "qexed",
            "economy_withdraw_async",
            host_economy_withdraw_async,
        )
        .context("register plugin economy_withdraw_async API")?;
    linker
        .func_wrap("qexed", "economy_async_poll", host_economy_async_poll)
        .context("register plugin economy_async_poll API")?;
    linker
        .func_wrap("qexed", "economy_async_forget", host_economy_async_forget)
        .context("register plugin economy_async_forget API")?;
    linker
        .func_wrap("qexed", "lottery_roll", host_lottery_roll)
        .context("register plugin lottery_roll API")?;
    linker
        .func_wrap("qexed", "pathfinding_find", host_pathfinding_find)
        .context("register plugin pathfinding_find API")?;
    linker
        .func_wrap("qexed", "world_set_block", host_world_set_block)
        .context("register plugin world_set_block API")?;
    linker
        .func_wrap("qexed", "world_set_blocks", host_world_set_blocks)
        .context("register plugin world_set_blocks API")?;
    linker
        .func_wrap("qexed", "world_break_block", host_world_break_block)
        .context("register plugin world_break_block API")?;
    linker
        .func_wrap(
            "qexed",
            "world_register_edit_region",
            host_world_register_edit_region,
        )
        .context("register plugin world_register_edit_region API")?;
    linker
        .func_wrap("qexed", "random_pool_roll", host_random_pool_roll)
        .context("register plugin random_pool_roll API")?;
    linker
        .func_wrap("qexed", "entity_upsert", host_entity_upsert)
        .context("register plugin entity_upsert API")?;
    linker
        .func_wrap("qexed", "entity_move", host_entity_move)
        .context("register plugin entity_move API")?;
    linker
        .func_wrap("qexed", "entity_remove", host_entity_remove)
        .context("register plugin entity_remove API")?;
    linker
        .func_wrap("qexed", "time_millis", host_time_millis)
        .context("register plugin time_millis API")?;
    linker
        .func_wrap("qexed", "plugin_service_exists", host_plugin_service_exists)
        .context("register plugin_service_exists API")?;
    linker
        .func_wrap("qexed", "plugin_call", host_plugin_call)
        .context("register plugin_call API")?;
    Ok(())
}

impl PluginInstance {
    pub(super) fn load(
        engine: &Engine,
        path: PathBuf,
        services: Arc<PluginHostServices>,
    ) -> Result<Self> {
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin")
            .to_string();
        let module = Module::from_file(engine, &path)
            .with_context(|| format!("编译插件 {}", path.display()))?;
        let exported_names = module
            .exports()
            .map(|export| export.name().to_string())
            .collect::<HashSet<_>>();
        let supported_events = PluginEvent::ALL
            .iter()
            .copied()
            .filter(|event| exported_names.contains(event.export_name()))
            .collect::<HashSet<_>>();
        let mut store = Store::new(
            engine,
            PluginState {
                name: name.clone(),
                services,
            },
        );
        let mut linker = Linker::new(engine);
        linker
            .func_wrap("qexed", "log", host_log)
            .context("注册插件宿主日志 API")?;
        register_host_apis(&mut linker)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .with_context(|| format!("实例化插件 {}", path.display()))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .with_context(|| format!("插件 {name} 缺少导出 memory"))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "qexed_plugin_alloc")
            .with_context(|| format!("插件 {name} 缺少导出 qexed_plugin_alloc"))?;
        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "qexed_plugin_dealloc")
            .ok();
        let priority = instance
            .get_typed_func::<(), i32>(&mut store, "qexed_plugin_priority")
            .ok()
            .map(|priority| priority.call(&mut store, ()))
            .transpose()
            .with_context(|| format!("读取插件 {name} 优先级失败"))?
            .unwrap_or(0);
        let manifest = read_manifest(
            &instance,
            &mut store,
            &memory,
            &alloc,
            dealloc.as_ref(),
            &name,
        )
        .unwrap_or_else(|err| {
            log::warn!("WASM plugin manifest read failed: plugin={name}, error={err:#}");
            PluginManifest {
                id: name.clone(),
                version: String::new(),
                depends: Vec::new(),
                optional_depends: Vec::new(),
                services: Vec::new(),
            }
        });

        Ok(Self {
            name,
            manifest,
            priority,
            supported_events,
            store,
            instance,
            memory,
            alloc,
            dealloc,
        })
    }

    pub(super) fn supports_event(&self, event: PluginEvent) -> bool {
        self.supported_events.contains(&event)
    }

    pub(super) fn call_event(&mut self, event: PluginEvent, payload: &[u8]) -> Result<()> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            anyhow::bail!(
                "插件事件 payload 超过限制: {} > {}",
                payload.len(),
                MAX_EVENT_PAYLOAD_BYTES
            );
        }

        match event {
            PluginEvent::Init => {
                let Some(func) = self
                    .instance
                    .get_typed_func::<(), ()>(&mut self.store, event.export_name())
                    .ok()
                else {
                    return Ok(());
                };
                func.call(&mut self.store, ())?;
            }
            _ => {
                let Some(func) = self
                    .instance
                    .get_typed_func::<(i32, i32), ()>(&mut self.store, event.export_name())
                    .ok()
                else {
                    return Ok(());
                };

                let len = i32::try_from(payload.len()).context("插件事件 payload 长度溢出")?;
                let ptr = self.alloc.call(&mut self.store, len)?;
                let offset = usize::try_from(ptr).context("插件分配器返回负地址")?;
                self.memory.write(&mut self.store, offset, payload)?;
                func.call(&mut self.store, (ptr, len))?;

                if let Some(dealloc) = &self.dealloc {
                    dealloc.call(&mut self.store, (ptr, len))?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn call_query(
        &mut self,
        event: PluginEvent,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            anyhow::bail!(
                "鎻掍欢鏌ヨ payload 瓒呰繃闄愬埗: {} > {}",
                payload.len(),
                MAX_EVENT_PAYLOAD_BYTES
            );
        }

        let Some(func) = self
            .instance
            .get_typed_func::<(i32, i32), i64>(&mut self.store, event.export_name())
            .ok()
        else {
            return Ok(None);
        };

        let len = i32::try_from(payload.len()).context("鎻掍欢鏌ヨ payload 闀垮害婧㈠嚭")?;
        let ptr = self.alloc.call(&mut self.store, len)?;
        let offset = usize::try_from(ptr).context("鎻掍欢鍒嗛厤鍣ㄨ繑鍥炶礋鍦板潃")?;
        self.memory.write(&mut self.store, offset, payload)?;
        let response = func.call(&mut self.store, (ptr, len))?;
        if let Some(dealloc) = &self.dealloc {
            dealloc.call(&mut self.store, (ptr, len))?;
        }

        let response_ptr = (response >> 32) as i32;
        let response_len = response as i32;
        if response_ptr <= 0 || response_len <= 0 {
            return Ok(None);
        }
        let response_len_usize =
            usize::try_from(response_len).context("鎻掍欢鏌ヨ杩斿洖浜嗚礋闀垮害")?;
        if response_len_usize > MAX_QUERY_RESPONSE_BYTES {
            anyhow::bail!(
                "鎻掍欢鏌ヨ response 瓒呰繃闄愬埗: {} > {}",
                response_len_usize,
                MAX_QUERY_RESPONSE_BYTES
            );
        }
        let response_offset =
            usize::try_from(response_ptr).context("鎻掍欢鏌ヨ杩斿洖浜嗚礋鍦板潃")?;
        let mut bytes = vec![0; response_len_usize];
        self.memory
            .read(&mut self.store, response_offset, &mut bytes)
            .context("璇诲彇鎻掍欢鏌ヨ response")?;
        if let Some(dealloc) = &self.dealloc {
            dealloc.call(&mut self.store, (response_ptr, response_len))?;
        }
        Ok(Some(bytes))
    }

    pub(super) fn call_event_or_query(
        &mut self,
        event: PluginEvent,
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        if payload.len() > MAX_EVENT_PAYLOAD_BYTES {
            anyhow::bail!(
                "鎻掍欢浜嬩欢 payload 瓒呰繃闄愬埗: {} > {}",
                payload.len(),
                MAX_EVENT_PAYLOAD_BYTES
            );
        }

        if let Ok(func) = self
            .instance
            .get_typed_func::<(i32, i32), i64>(&mut self.store, event.export_name())
        {
            let len = i32::try_from(payload.len()).context("鎻掍欢 query payload 闀垮害婧㈠嚭")?;
            let ptr = self.alloc.call(&mut self.store, len)?;
            let offset = usize::try_from(ptr).context("鎻掍欢鍒嗛厤鍣ㄨ繑鍥炶礋鍦板潃")?;
            self.memory.write(&mut self.store, offset, payload)?;
            let response = func.call(&mut self.store, (ptr, len))?;
            if let Some(dealloc) = &self.dealloc {
                dealloc.call(&mut self.store, (ptr, len))?;
            }

            let response_ptr = (response >> 32) as i32;
            let response_len = response as i32;
            if response_ptr <= 0 || response_len <= 0 {
                return Ok(None);
            }
            let response_len_usize =
                usize::try_from(response_len).context("鎻掍欢 query response 闀垮害鏃犳晥")?;
            if response_len_usize > MAX_QUERY_RESPONSE_BYTES {
                anyhow::bail!(
                    "鎻掍欢 query response 瓒呰繃闄愬埗: {} > {}",
                    response_len_usize,
                    MAX_QUERY_RESPONSE_BYTES
                );
            }
            let response_offset =
                usize::try_from(response_ptr).context("鎻掍欢 query response 鍦板潃鏃犳晥")?;
            let mut bytes = vec![0; response_len_usize];
            self.memory
                .read(&mut self.store, response_offset, &mut bytes)
                .context("璇诲彇鎻掍欢 query response 澶辫触")?;
            if let Some(dealloc) = &self.dealloc {
                dealloc.call(&mut self.store, (response_ptr, response_len))?;
            }
            return Ok(Some(bytes));
        }

        if let Ok(func) = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, event.export_name())
        {
            let len = i32::try_from(payload.len()).context("鎻掍欢浜嬩欢 payload 闀垮害婧㈠嚭")?;
            let ptr = self.alloc.call(&mut self.store, len)?;
            let offset = usize::try_from(ptr).context("鎻掍欢鍒嗛厤鍣ㄨ繑鍥炶礋鍦板潃")?;
            self.memory.write(&mut self.store, offset, payload)?;
            func.call(&mut self.store, (ptr, len))?;
            if let Some(dealloc) = &self.dealloc {
                dealloc.call(&mut self.store, (ptr, len))?;
            }
        }

        Ok(None)
    }
}

fn read_manifest(
    instance: &Instance,
    store: &mut Store<PluginState>,
    memory: &Memory,
    alloc: &TypedFunc<i32, i32>,
    dealloc: Option<&TypedFunc<(i32, i32), ()>>,
    name: &str,
) -> Result<PluginManifest> {
    let Ok(func) = instance.get_typed_func::<(i32, i32), i64>(&mut *store, "qexed_plugin_manifest")
    else {
        return Ok(default_manifest(name));
    };

    let ptr = alloc.call(&mut *store, 0)?;
    let response = func.call(&mut *store, (ptr, 0))?;
    if let Some(dealloc) = dealloc {
        dealloc.call(&mut *store, (ptr, 0))?;
    }
    let response_ptr = (response >> 32) as i32;
    let response_len = response as i32;
    if response_ptr <= 0 || response_len <= 0 {
        return Ok(default_manifest(name));
    }

    let response_len = usize::try_from(response_len).context("plugin manifest length")?;
    if response_len > MAX_QUERY_RESPONSE_BYTES {
        anyhow::bail!("plugin manifest response too large: {response_len}");
    }
    let response_offset = usize::try_from(response_ptr).context("plugin manifest pointer")?;
    let mut bytes = vec![0; response_len];
    memory.read(&mut *store, response_offset, &mut bytes)?;
    if let Some(dealloc) = dealloc {
        dealloc.call(&mut *store, (response_ptr, response_len as i32))?;
    }

    let mut manifest: PluginManifest = postcard::from_bytes(&bytes)?;
    if manifest.id.trim().is_empty() {
        manifest.id = name.to_string();
    }
    Ok(manifest)
}

fn default_manifest(name: &str) -> PluginManifest {
    PluginManifest {
        id: name.to_string(),
        version: String::new(),
        depends: Vec::new(),
        optional_depends: Vec::new(),
        services: Vec::new(),
    }
}
