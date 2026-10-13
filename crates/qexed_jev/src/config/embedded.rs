//! 内嵌模式配置：worker 线程、请求队列与微批参数。
//!
//! 性能设计（为什么是这几个旋钮）：
//! - [`JevEmbeddedConfig::queue_capacity`] 是硬背压上限。聊天是突发低流量场景，
//!   与其把请求无界堆在内存里让尾部延迟爆炸，不如满了立刻拒绝，让上层提示"稍后再试"。
//! - [`JevEmbeddedConfig::micro_batch`]：真实 jev 权重接入后，加速器一次吃
//!   `micro_batch` 条请求的吞吐远高于逐条串行；worker 会把队列里同时就绪的请求
//!   凑满一批再喂给引擎。
//! - [`JevEmbeddedConfig::thread_priority_idle`]：Minecraft 服务器主循环对抖动
//!   极其敏感，推理线程默认让出调度优先级，避免 tick 卡顿。

use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

/// ```autodoc
/// <Name>qexed.crates.jev.config.JevEmbeddedConfig</Name>
/// <Attr name="writable" />
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Doc)]
pub struct JevEmbeddedConfig {
    /// 模型权重文件路径；留空 = 内置回声引擎（真实加载器接入前的兜底）。
    /// 绝对路径原样使用；相对路径锚定「服务器主目录」（配置根目录的上一级，
    /// dev 下为 `run/`），与进程工作目录无关。生产环境按部署布局摆放权重即可。
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevEmbeddedConfig.model_path</Name>
    /// <Default>""</Default>
    /// <Attr name="writable" />
    /// ```
    pub model_path: String,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevEmbeddedConfig.queue_capacity</Name>
    /// <Default>32</Default>
    /// <Attr name="writable" />
    /// ```
    pub queue_capacity: usize,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevEmbeddedConfig.micro_batch</Name>
    /// <Default>4</Default>
    /// <Attr name="writable" />
    /// ```
    pub micro_batch: usize,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevEmbeddedConfig.request_timeout_ms</Name>
    /// <Default>10000</Default>
    /// <Attr name="writable" />
    /// ```
    pub request_timeout_ms: u64,
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.JevEmbeddedConfig.thread_priority_idle</Name>
    /// <Default>true</Default>
    /// <Attr name="writable" />
    /// ```
    pub thread_priority_idle: bool,
}

impl Default for JevEmbeddedConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            queue_capacity: 32,
            micro_batch: 4,
            request_timeout_ms: 10_000,
            thread_priority_idle: true,
        }
    }
}

impl JevEmbeddedConfig {
    /// 内嵌配置合法性检查（启动时调用，配置坏了直接暴露而不是推理时才炸）。
    pub fn validate(&self) -> Result<(), crate::error::JevError> {
        if self.queue_capacity == 0 {
            return Err(crate::error::JevError::InvalidConfig(
                "embedded.queue_capacity must be > 0".into(),
            ));
        }
        if self.micro_batch == 0 {
            return Err(crate::error::JevError::InvalidConfig(
                "embedded.micro_batch must be > 0".into(),
            ));
        }
        if self.request_timeout_ms == 0 {
            return Err(crate::error::JevError::InvalidConfig(
                "embedded.request_timeout_ms must be > 0".into(),
            ));
        }
        Ok(())
    }
}
