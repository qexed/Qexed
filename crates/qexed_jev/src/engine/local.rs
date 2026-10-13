//! 本地 GGUF 推理引擎：llama.cpp（llama-cpp-2）封装。
//!
//! 职责边界：
//! - 只做"加载权重 → 按对话模板推理 → 返回文本"这一件事；
//! - 不懂审核/会话语义，那是上层 semantic / session 的事；
//! - CPU 推理（openmp），贪心采样：审核判定要确定性，不要创造性。
//!
//! 每次请求新建 context（KV 不跨请求复用）：正确性优先，0.5B 级模型
//! 单次建 context 开销可接受；做常驻 context 缓存属于后续优化。

use super::{JevCompletion, JevEngine, JevRequest};
use crate::error::JevError;
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{AddBos, LlamaModel},
    sampling::LlamaSampler,
};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

/// 单次生成的 token 上限：审核判定是一小段 JSON，讲话则由上层约束。
const MAX_GENERATE_TOKENS: usize = 256;

/// llama.cpp 全局后端：进程内只初始化一次。
fn backend() -> Result<&'static LlamaBackend, JevError> {
    static CELL: OnceLock<Option<LlamaBackend>> = OnceLock::new();
    let slot = CELL.get_or_init(|| match LlamaBackend::init() {
        Ok(backend) => Some(backend),
        Err(err) => {
            log::error!("llama backend init failed: {err}");
            None
        }
    });
    slot.as_ref()
        .ok_or_else(|| JevError::Inference("llama backend init failed (see earlier log)".into()))
}

/// ChatML 对话模板（Qwen2.5 系）。jev 内嵌模型按 Qwen2.5 选型；
/// 其他模板家族的权重加载后应在这里扩展（按 model 元信息分发）。
fn render_chatml(messages: &[(String, String)]) -> String {
    let mut out = String::new();
    for (role, content) in messages {
        out.push_str("<|im_start|>");
        out.push_str(role);
        out.push('\n');
        out.push_str(content);
        out.push_str("<|im_end|>\n");
    }
    out.push_str("<|im_start|>assistant\n");
    out
}

pub struct LocalEngine {
    model: LlamaModel,
    n_ctx: u32,
    threads: i32,
}

impl LocalEngine {
    /// 从 GGUF 文件加载权重。失败返回 Inference 错误（服务启动即硬失败）。
    pub fn load(model_path: &Path) -> Result<Self, JevError> {
        if !model_path.exists() {
            return Err(JevError::Inference(format!(
                "model file not found: {}",
                model_path.display()
            )));
        }
        let backend = backend()?;
        let model = LlamaModel::load_from_file(backend, model_path, &Default::default())
            .map_err(|err| JevError::Inference(format!("load model failed: {err}")))?;

        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        // 上下文取 4096 与训练窗口的较小者，够审核对话+判定输出。
        let n_ctx = 4096u32.min(model.n_ctx_train());
        log::info!(
            "jev local model loaded: {} (ctx={n_ctx}, threads={threads})",
            model_path.display()
        );
        Ok(Self { model, n_ctx, threads })
    }
}

impl JevEngine for LocalEngine {
    fn complete(&mut self, request: JevRequest) -> Result<JevCompletion, JevError> {
        let started = Instant::now();
        let backend = backend()?;

        let prompt = render_chatml(&request.messages);
        let tokens = self
            .model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|err| JevError::Inference(format!("tokenize failed: {err}")))?;
        let n_ctx = self.n_ctx as usize;
        if tokens.len() + 1 >= n_ctx {
            return Err(JevError::Inference(format!(
                "prompt too long: {} tokens (ctx {n_ctx})",
                tokens.len()
            )));
        }

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(self.n_ctx))
            .with_n_threads(self.threads)
            .with_n_threads_batch(self.threads);
        let mut ctx = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|err| JevError::Inference(format!("new context failed: {err}")))?;

        // 贪心采样：判定输出要可复现，不要随机性。
        let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);

        // ── prefill：整段 prompt 一次进批，只有末位要 logits ──
        let mut batch = LlamaBatch::new(tokens.len(), 1);
        for (i, token) in tokens.iter().enumerate() {
            batch
                .add(*token, i as i32, &[0], i + 1 == tokens.len())
                .map_err(|err| JevError::Inference(format!("batch fill failed: {err}")))?;
        }
        ctx.decode(&mut batch)
            .map_err(|err| JevError::Inference(format!("prefill failed: {err}")))?;

        // ── 逐 token 生成：单步批喂回上一个 token ──
        let mut text = String::new();
        let mut logits_idx = tokens.len() as i32 - 1;
        let mut pos = tokens.len() as i32;
        for _ in 0..MAX_GENERATE_TOKENS {
            let token = sampler.sample(&ctx, logits_idx);
            if self.model.is_eog_token(token) {
                break;
            }
            #[allow(deprecated)]
            let piece = self
                .model
                .token_to_str(token, llama_cpp_2::model::Special::Plaintext)
                .map_err(|err| JevError::Inference(format!("detokenize failed: {err}")))?;
            text.push_str(&piece);

            let mut step = LlamaBatch::new(1, 1);
            step.add(token, pos, &[0], true)
                .map_err(|err| JevError::Inference(format!("batch step failed: {err}")))?;
            ctx.decode(&mut step)
                .map_err(|err| JevError::Inference(format!("decode failed: {err}")))?;
            logits_idx = 0;
            pos += 1;
        }

        Ok(JevCompletion {
            text: text.trim().to_string(),
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JevConfig;

    /// 没有权重文件时加载必须报错（不静默退回回声）。
    #[test]
    fn missing_model_file_is_hard_error() {
        let mut config = JevConfig::default();
        config.embedded.model_path = "Z:/definitely/not/here.gguf".into();
        let err = match super::super::build_embedded_engine(&config) {
            Err(err) => err,
            Ok(_) => panic!("missing weights must fail loudly, not echo"),
        };
        assert!(matches!(err, JevError::Inference(_)), "got: {err:?}");
    }

    /// 模板渲染：user 消息包在 im_start/im_end 里，结尾挂 assistant 起始。
    #[test]
    fn chatml_wraps_roles() {
        let rendered = render_chatml(&[
            ("system".into(), "你是审核器".into()),
            ("user".into(), "你好".into()),
        ]);
        assert!(rendered.contains("<|im_start|>system\n你是审核器<|im_end|>"));
        assert!(rendered.starts_with("<|im_start|>"));
        assert!(rendered.ends_with("<|im_start|>assistant\n"));
    }

    /// 真实权重冒烟：加载 run/models 下的 GGUF，判定一条违规消息。
    /// 469MB 权重 + 真推理，默认 ignore；发版前/排查时带 --ignored 跑。
    #[test]
    #[ignore = "needs real GGUF weights in run/models"]
    fn real_model_smoke_judges_violation() {
        let mut config = crate::config::JevConfig::default();
        config.embedded.model_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../run/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"
        )
        .into();
        let mut engine = super::super::build_embedded_engine(&config).expect("load real model");

        let judgement = crate::semantic::judgement_system_prompt();
        let request = super::super::JevRequest {
            model: None,
            messages: vec![
                ("system".to_string(), judgement),
                ("user".to_string(), "加我qq 12345678 带你上分".to_string()),
            ],
            temperature: Some(0.0),
        };
        let out = engine.complete(request).expect("inference works");
        println!("model raw output: {}", out.text);
        println!("elapsed: {} ms", out.elapsed_ms);
        let parsed = crate::semantic::parse_judgement_for_tests(&out.text);
        assert!(parsed.is_some(), "output should parse as judgement JSON");
        let judgement = parsed.unwrap();
        assert!(
            judgement.violation,
            "contact solicitation should be flagged, got: {:?}",
            judgement.categories
        );
    }
}
