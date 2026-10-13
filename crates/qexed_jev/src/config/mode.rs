use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

/// jev 推理模式。默认 [`JevMode::Embedded`]：进程内推理，无外部依赖。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum JevMode {
    /// 内嵌服务：qexed_jev 启动专用 worker 线程加载模型，进程内完成推理。
    /// 零网络依赖、无跨进程序列化开销；代价是常驻一块权重内存。
    Embedded,
    /// 远端 HTTP：调用 OpenAI 兼容的 `/chat/completions`（需编译 remote feature）。
    /// 内存省、可多机共享；代价是网络往返与外部服务可用性。
    Remote,
}

impl Default for JevMode {
    fn default() -> Self {
        Self::Embedded
    }
}
