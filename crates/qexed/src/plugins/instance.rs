use std::path::PathBuf;

use anyhow::{Context, Result};
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

use super::{PluginEvent, PluginState, host::host_log};

const MAX_EVENT_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_QUERY_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) struct PluginInstance {
    pub(super) name: String,
    pub(super) priority: i32,
    store: Store<PluginState>,
    instance: Instance,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    dealloc: Option<TypedFunc<(i32, i32), ()>>,
}

impl PluginInstance {
    pub(super) fn load(engine: &Engine, path: PathBuf) -> Result<Self> {
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin")
            .to_string();
        let module = Module::from_file(engine, &path)
            .with_context(|| format!("编译插件 {}", path.display()))?;
        let mut store = Store::new(engine, PluginState { name: name.clone() });
        let mut linker = Linker::new(engine);
        linker
            .func_wrap("qexed", "log", host_log)
            .context("注册插件宿主日志 API")?;
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

        Ok(Self {
            name,
            priority,
            store,
            instance,
            memory,
            alloc,
            dealloc,
        })
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
}
