//! 页面校验编排：轻量层（清单哈希自洽）与完整层（docker 复现）。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::docker::{CONTAINER_WORKSPACE, docker_run, ensure_image};
use crate::manifest::VerifyManifest;

/// snippet 工程的物化根：工作区 target 下，天然被 git 忽略、cargo clean 可清。
pub const MATERIALIZE_ROOT: &str = "target/doc-verify";

/// 单页校验结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// 轻量层：清单哈希全部自洽，未执行 snippet。
    ManifestOk,
    /// 完整层：docker 内构建 + 运行 + 退出码比对全部通过。
    ReproducedOk,
}

/// 轻量校验（毫秒级，不碰 docker）：
/// 1. verify_hash 对 verify 块重算一致（parse_page 内完成）；
/// 2. api.hash 对 symbols 重算一致；
/// 3. result.hash 对 result.value 重算一致；
/// 4. snippet_hash 对正文第一个 rust 代码块字节级一致。
pub fn verify_manifest(page_path: &Path) -> Result<VerifyOutcome> {
    let mdx = std::fs::read_to_string(page_path)
        .with_context(|| format!("读取页面失败: {}", page_path.display()))?;
    let manifest = VerifyManifest::parse_page(&mdx)?;

    let api_actual = crate::manifest::hash_value(&json!({ "symbols": manifest.api.symbols }));
    if manifest.api.hash != api_actual {
        bail!("api.hash 与 symbols 重算不符: 声明 {}，重算 {api_actual}", manifest.api.hash);
    }

    let result_actual = crate::manifest::hash_value(&json!({ "value": manifest.result.value }));
    if manifest.result.hash != result_actual {
        bail!("result.hash 与 result.value 重算不符: 声明 {}，重算 {result_actual}", manifest.result.hash);
    }

    let snippet = first_rust_fence(&mdx)
        .ok_or_else(|| anyhow::anyhow!("页面没有 rust 代码块，snippet_hash 无从核对"))?;
    let snippet_actual = crate::manifest::hash_bytes(snippet.as_bytes());
    if manifest.snippet_hash != snippet_actual {
        bail!(
            "snippet_hash 与正文代码块不符: 声明 {}，重算 {snippet_actual}（文档被手改或哈希未回填）",
            manifest.snippet_hash
        );
    }

    Ok(VerifyOutcome::ManifestOk)
}

/// 完整校验：把 snippet 工程物化到 target/doc-verify/<页名>，
/// 在钉死 toolchain 的 docker 镜像里 cargo run，比对退出码。
///
/// 挂载约定（见 docker.rs）：工作区只读挂到 /workspace，物化目录可写挂到 /doc-verify，
/// cargo_toml 的 {{WORKSPACE}} 指向 /workspace，路径依赖直接命中工作区内真实 crate。
/// 构建预览 / CLI 用的复现报告（JSON 可序列化）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReproduceReport {
    pub ok: bool,
    pub outcome: String,
    pub page: String,
    pub image: String,
    pub exit_code: i32,
    pub expected_exit_code: i32,
    pub snippet_hash: String,
    pub verify_hash: String,
    pub cache_key: String,
    pub output_tail: String,
    pub error: Option<String>,
}

fn cache_key(snippet_hash: &str, verify_hash: &str) -> String {
    crate::manifest::hash_bytes(format!("{snippet_hash}\n{verify_hash}").as_bytes())
}

/// 按清单在 docker 里复现一页。不要求 tokio。
pub fn reproduce_page(page_path: &Path) -> ReproduceReport {
    reproduce_page_opts(page_path, false)
}

/// `skip_hash_gate`：构建预览用。正文示例改了也先按当前代码复现，再用退出码判断；
/// 哈希是否回填只作为报告字段，不拦截 docker。
pub fn reproduce_page_opts(page_path: &Path, skip_hash_gate: bool) -> ReproduceReport {
    let page = page_path.display().to_string();
    match reproduce_page_inner(page_path, skip_hash_gate) {
        Ok(report) => report,
        Err(err) => ReproduceReport {
            ok: false,
            outcome: "error".into(),
            page,
            image: String::new(),
            exit_code: -1,
            expected_exit_code: 0,
            snippet_hash: String::new(),
            verify_hash: String::new(),
            cache_key: String::new(),
            output_tail: String::new(),
            error: Some(format!("{err:#}")),
        },
    }
}

fn reproduce_page_inner(page_path: &Path, skip_hash_gate: bool) -> Result<ReproduceReport> {
    let page = page_path.display().to_string();
    if !skip_hash_gate {
        verify_manifest(page_path)?;
    }
    let mdx = std::fs::read_to_string(page_path)?;
    let (frontmatter, _) = crate::manifest::split_frontmatter(&mdx)?;
    let verify_hash = crate::manifest::extract_plain_field(&frontmatter, "verify_hash")?;
    let manifest = VerifyManifest::parse_page(&mdx)?;
    let snippet = first_rust_fence(&mdx)
        .ok_or_else(|| anyhow::anyhow!("页面没有 rust 代码块，无法复现"))?;
    let snippet_hash = crate::manifest::hash_bytes(snippet.as_bytes());
    let cache_key = cache_key(&snippet_hash, &verify_hash);
    let image = manifest.docker_image();

    let page_stem = page_stem(page_path);
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let host_root = workspace.join(MATERIALIZE_ROOT);
    std::fs::create_dir_all(host_root.join(&page_stem).join("src"))?;
    let cargo_toml = manifest.cargo_toml.replace("{{WORKSPACE}}", CONTAINER_WORKSPACE);
    std::fs::write(host_root.join(&page_stem).join("Cargo.toml"), cargo_toml)?;
    std::fs::write(host_root.join(&page_stem).join("src/main.rs"), &snippet)?;

    ensure_image(&image)?;
    let command = "cargo run --quiet 2>stderr.log; code=$?; tail -n 50 stderr.log >&2; exit $code";
    let (code, output) = docker_run(&image, &host_root, &format!("/doc-verify/{page_stem}"), command)?;
    let output_tail = output
        .lines()
        .rev()
        .take(30)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");

    if code != manifest.exit_code {
        return Ok(ReproduceReport {
            ok: false,
            outcome: "drift".into(),
            page,
            image,
            exit_code: code,
            expected_exit_code: manifest.exit_code,
            snippet_hash,
            verify_hash,
            cache_key,
            output_tail,
            error: Some(format!(
                "退出码漂移: 清单 exit_code={}，本次容器内运行 {code}",
                manifest.exit_code
            )),
        });
    }

    Ok(ReproduceReport {
        ok: true,
        outcome: "reproduced".into(),
        page,
        image,
        exit_code: code,
        expected_exit_code: manifest.exit_code,
        snippet_hash,
        verify_hash,
        cache_key,
        output_tail,
        error: None,
    })
}

pub async fn verify_page_in_docker(page_path: &Path) -> Result<VerifyOutcome> {
    let report = reproduce_page(page_path);
    if report.ok {
        Ok(VerifyOutcome::ReproducedOk)
    } else {
        bail!(
            "{}{}",
            report.error.unwrap_or_else(|| "复现失败".into()),
            if report.output_tail.is_empty() {
                String::new()
            } else {
                format!("\n--- 容器输出尾部 ---\n{}", report.output_tail)
            }
        )
    }
}

/// 物化目录名：文件 stem（非法字符折叠为下划线）。
/// 页面普遍叫 page.mdx，因此追加父目录名前缀（log/page.mdx → log_page），
/// 同名页面不同路由不会互相覆盖物化目录。
fn page_stem(page_path: &Path) -> String {
    let fold = |s: &str| s.replace(['-', '.'], "_");
    let stem = page_path
        .file_stem()
        .map(|n| fold(&n.to_string_lossy()))
        .unwrap_or_else(|| "page".to_string());
    if stem == "page" {
        if let Some(parent) = page_path.parent().and_then(|p| p.file_name()) {
            return format!("{}_{}", fold(&parent.to_string_lossy()), stem);
        }
    }
    stem
}

/// 提取正文第一个 rust 围栏块内容（不含围栏行与结尾换行）。
pub fn first_rust_fence(mdx: &str) -> Option<String> {
    let marker = "```rust";
    let start = mdx.find(marker)? + marker.len();
    let rest = &mdx[start..];
    let nl = rest.find('\n')? + 1;
    let end = rest[nl..].find("\n```")? + nl;
    Some(rest[nl..end].to_string())
}

/// 遍历目录收集全部 page.mdx（排序保证输出稳定）。
pub fn collect_pages(pages_root: &Path) -> Result<Vec<PathBuf>> {
    if !pages_root.exists() {
        bail!("页面目录不存在: {}", pages_root.display());
    }
    let mut out = Vec::new();
    collect_recursive(pages_root, &mut out)?;
    out.sort();
    Ok(out)
}

/// 收集目录下全部 .mdx 页面（递归，排序保证输出稳定）。
/// 与 collect_pages 的区别：不限定文件名，适配文档站 app/ 路由树。
pub fn collect_mdx_pages(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        bail!("目录不存在: {}", root.display());
    }
    let mut out = Vec::new();
    collect_recursive_by_ext(root, "mdx", &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_recursive_by_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_recursive_by_ext(&path, ext, out)?;
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_recursive(&path, out)?;
        } else if path.file_name().is_some_and(|n| n == "page.mdx") {
            out.push(path);
        }
    }
    Ok(())
}

/// 轻量校验整个页面目录，返回 (通过数, 总数)；任何一页失败即整体报错。
pub fn run_pages(pages_root: &Path) -> Result<(usize, usize)> {
    let pages = collect_pages(pages_root)?;
    let total = pages.len();
    let mut ok = 0;
    for page in &pages {
        verify_manifest(page).map_err(|e| anyhow::anyhow!("{}: {e:#}", page.display()))?;
        ok += 1;
    }
    Ok((ok, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{canonical_hash, hash_bytes, hash_value};

    /// 构造一页自洽的合成页面。
    fn synthetic_page() -> String {
        let snippet = "println!(\"log ready\");".to_string();
        let symbols = vec![
            "qexed_log::init() -> Result<(), qexed_log::error::LogError>".to_string(),
        ];
        let api_hash = hash_value(&json!({ "symbols": symbols }));
        let result_value = json!({ "exit": 0, "stderr": [], "stdout": ["log ready"] });
        let result_hash = hash_value(&json!({ "value": result_value }));
        let verify = json!({
            "spec": "qexed.doc.verify/1",
            "shell": "tokio-rt@1",
            "toolchain": "1.98.1",
            "snippet_hash": hash_bytes(snippet.as_bytes()),
            "api": { "symbols": symbols, "hash": api_hash },
            "result": { "format": "json", "value": result_value, "hash": result_hash },
            "run_ok": true,
            "exit_code": 0,
            "cargo_toml": "[package]\nname = \"doc-snippet-test\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
            "deps_lock_hash": "",
            "commit": "test",
            "ci_run": "https://ci.example.com/run/1",
            "verified_at": "2026-10-05T00:00:00Z",
        });
        let verify_json = serde_json::to_string_pretty(&verify).unwrap();
        let verify_hash = canonical_hash(&verify_json).unwrap();
        let indented = verify_json
            .lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "---\ntitle: 测试页\nverify: |\n{indented}\nverify_hash: {verify_hash}\n---\n\n# 测试\n\n```rust\n{snippet}\n```\n"
        )
    }

    #[test]
    fn manifest_roundtrip_passes() {
        let page = synthetic_page();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.mdx");
        std::fs::write(&path, &page).unwrap();
        assert_eq!(verify_manifest(&path).unwrap(), VerifyOutcome::ManifestOk);
    }

    #[test]
    fn tampered_snippet_is_caught() {
        let page = synthetic_page().replace("println!(\"log ready\");", "println!(\"changed\");");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.mdx");
        std::fs::write(&path, &page).unwrap();
        let err = verify_manifest(&path).unwrap_err().to_string();
        assert!(err.contains("snippet_hash"), "unexpected: {err}");
    }

    #[test]
    fn tampered_verify_block_is_caught() {
        let page = synthetic_page().replace("\"run_ok\": true", "\"run_ok\": false");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.mdx");
        std::fs::write(&path, &page).unwrap();
        let err = verify_manifest(&path).unwrap_err().to_string();
        assert!(err.contains("verify_hash"), "unexpected: {err}");
    }

    #[test]
    fn fence_extraction_exact_bytes() {
        let mdx = "前文\n```rust\nfn main() {}\nlet x = 1;\n```\n后文";
        assert_eq!(first_rust_fence(mdx).unwrap(), "fn main() {}\nlet x = 1;");
    }

    #[test]
    fn collect_pages_finds_nested() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a/page.mdx");
        let b = dir.path().join("b/c/page.mdx");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, "x").unwrap();
        std::fs::write(&b, "y").unwrap();
        assert_eq!(collect_pages(dir.path()).unwrap().len(), 2);
    }
}
