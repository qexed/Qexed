//! 文档示例校验器：把 MDX 页面 frontmatter 里的 `verify:` 清单
//! 交给 docker 复现「构建 + 运行 + 结果比对」，入口就是 `cargo test`。
//!
//! 三层证据（详见 `verify-manifest.example.mdx`）：
//! 1. 定义漂移 —— snippet 编译即裁判：qexed 签名变了，容器内 cargo build 先炸；
//! 2. 结构漂移 —— 运行产出的类型化结果哈希对比 `result.hash`；
//! 3. 清单自身完整性 —— `verify_hash` / `api.hash` / `result.hash` 全部可重算比对。
//!
//! 快速校验（不碰 docker，毫秒级，随常规 cargo test 跑）：
//! ```bash
//! cargo test -p qexed_doc_verify
//! ```
//! 完整复现（需要本机 docker，首跑会拉镜像和依赖）：
//! ```bash
//! cargo test -p qexed_doc_verify -- --ignored --nocapture
//! ```

pub mod docker;
pub mod manifest;
pub mod verify;

pub use manifest::VerifyManifest;
pub use verify::{ReproduceReport, VerifyOutcome, collect_mdx_pages, reproduce_page, reproduce_page_opts, run_pages, verify_manifest, verify_page_in_docker};
