//! 集成测试：真实示例页的轻量校验 +（可选）docker 全链路复现。
//!
//! 轻量层随 `cargo test` 默认跑；完整层需要 docker，用：
//! ```bash
//! cargo test -p qexed_doc_verify -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};

use qexed_doc_verify::verify::{MATERIALIZE_ROOT, verify_manifest, verify_page_in_docker};

/// 仓库根（qexed_doc_verify 的上一级）。
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

/// 「日志初始化」示例页：doc verify 机制的首个真实样本。
fn log_init_page() -> PathBuf {
    workspace_root().join("verify-manifest.example.mdx")
}

#[test]
fn example_page_manifest_is_self_consistent() {
    let page = log_init_page();
    assert!(page.exists(), "示例页缺失: {}", page.display());
    match verify_manifest(&page) {
        Ok(outcome) => assert_eq!(outcome, qexed_doc_verify::VerifyOutcome::ManifestOk),
        Err(err) => panic!("示例页清单校验失败: {err:#}"),
    }
}

/// 完整复现：拉镜像 + 编译依赖首跑需数分钟，且要求本机 docker daemon。
#[tokio::test]
#[ignore = "需要 docker：cargo test -p qexed_doc_verify -- --ignored"]
async fn example_page_reproduces_in_docker() {
    let page = log_init_page();
    match verify_page_in_docker(&page).await {
        Ok(outcome) => assert_eq!(outcome, qexed_doc_verify::VerifyOutcome::ReproducedOk),
        Err(err) => panic!("容器内复现失败: {err:#}"),
    }
}

#[test]
fn materialize_root_is_under_target() {
    assert!(MATERIALIZE_ROOT.starts_with("target/"));
}

#[test]
fn doc_site_pages_manifests_are_self_consistent() {
    // 文档站仓库：默认取与 qexed-v6 同级的 qexed-v6-doc，CI 可用 DOC_SITE_DIR 覆盖
    let doc_site = std::env::var("DOC_SITE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().parent().unwrap().join("qexed-v6-doc"));
    if !doc_site.exists() {
        // CI 只 checkout 本仓库时没有文档站：跳过而非失败（本地开发照常严查）。
        eprintln!("skip: 文档站目录不存在（{}），如需校验请 checkout 或设 DOC_SITE_DIR", doc_site.display());
        return;
    }

    // 站点全部 MDX 页面里必须至少有 1 页带 verify 清单，且全部自洽
    let pages = qexed_doc_verify::collect_mdx_pages(&doc_site.join("app"))
        .expect("遍历文档站页面失败");
    assert!(
        pages.len() >= 30,
        "文档站页面数量异常: {}",
        pages.len()
    );

    let mut with_manifest = 0usize;
    let mut failures = Vec::new();
    for page in &pages {
        let text = std::fs::read_to_string(page).unwrap_or_default();
        if !text.contains("verify: |") {
            continue;
        }
        with_manifest += 1;
        if let Err(err) = qexed_doc_verify::verify_manifest(page) {
            failures.push(format!("{}: {err}", page.display()));
        }
    }
    assert!(
        with_manifest >= 1,
        "文档站没有任何页面带 verify 清单"
    );
    assert!(
        failures.is_empty(),
        "以下页面清单校验失败:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
#[ignore = "需要 docker，运行：cargo test -p qexed_doc_verify -- --ignored --nocapture"]
async fn doc_site_pages_reproduce_in_docker() {
    // 与轻量层相同的站点定位（CI 用 DOC_SITE_DIR 覆盖）
    let doc_site = std::env::var("DOC_SITE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().parent().unwrap().join("qexed-v6-doc"));
    if !doc_site.exists() {
        // CI 只 checkout 本仓库时没有文档站：跳过而非失败。
        eprintln!("skip: 文档站目录不存在（{}），如需校验请 checkout 或设 DOC_SITE_DIR", doc_site.display());
        return;
    }
    let pages = qexed_doc_verify::collect_mdx_pages(&doc_site.join("app"))
        .expect("遍历文档站页面失败");

    let mut checked = 0usize;
    for page in &pages {
        let text = std::fs::read_to_string(page).unwrap_or_default();
        if !text.contains("verify: |") {
            continue;
        }
        checked += 1;
        println!("docker 复现: {}", page.display());
        let outcome = qexed_doc_verify::verify_page_in_docker(page)
            .await
            .unwrap_or_else(|e| panic!("{}: {e:#}", page.display()));
        assert_eq!(outcome, qexed_doc_verify::VerifyOutcome::ReproducedOk);
    }
    assert!(checked >= 1, "文档站没有带 verify 清单的页面可复现");
}
