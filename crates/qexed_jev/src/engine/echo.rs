//! 内置回声引擎：零权重的确定性实现，用于管线联调与测试。

use super::{JevCompletion, JevEngine, JevRequest};
use crate::error::JevError;
use std::time::Instant;

/// 确定性回声：取最后一条用户消息加工后原样返回。
///
/// 目的：
/// - 让内嵌服务/会话/背压管线在没有真实 jev 权重时就端到端可用；
/// - 测试可以断言确定性输出，不引入任何模型依赖。
pub struct EchoEngine {
    /// 模拟推理耗时（毫秒），用于验证超时/并发路径；0 = 立即返回。
    simulated_latency_ms: u64,
}

impl EchoEngine {
    pub fn new() -> Self {
        Self { simulated_latency_ms: 0 }
    }

    /// 测试辅助：设置模拟推理耗时。
    pub fn with_simulated_latency_ms(mut self, ms: u64) -> Self {
        self.simulated_latency_ms = ms;
        self
    }
}

impl Default for EchoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl JevEngine for EchoEngine {
    fn complete(&mut self, request: JevRequest) -> Result<JevCompletion, JevError> {
        let started = Instant::now();
        if self.simulated_latency_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(
                self.simulated_latency_ms,
            ));
        }
        let last_user = request
            .messages
            .iter()
            .rev()
            .find(|(role, _)| role == "user")
            .map(|(_, content)| content.as_str())
            .unwrap_or("");
        Ok(JevCompletion {
            text: format!("[jev:echo] {last_user}"),
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }
}
