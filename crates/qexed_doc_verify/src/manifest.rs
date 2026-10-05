//! `verify:` 清单的解析、frontmatter 切分与哈希工具。
//!
//! 刻意不引入完整 YAML 解析器：生成器产出的 frontmatter 布局是固定的
//! （`verify: |` 字面块 + 平铺的 `verify_hash:` 行），按行切分即可，
//! 解析失败时给出明确错误而不是猜。

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// 页面 frontmatter 中 `verify: |` 块反序列化出的完整清单。
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyManifest {
    /// 规范化规则版本，当前固定 "qexed.doc.verify/1"。
    pub spec: String,
    /// 运行壳版本，如 "tokio-rt@1"。壳变化但不改变用户代码语义时递增。
    pub shell: String,
    /// 上游 rustc 版本，如 "1.98.1"。
    pub toolchain: String,
    /// 校验用 docker 镜像；缺省为 `rust:<major.minor>`。
    #[serde(default)]
    pub image: Option<String>,
    /// 用户可见代码块（字节级）的哈希。
    pub snippet_hash: String,
    /// 参与校验的 API 签名清单及其完整性哈希。
    pub api: ApiSection,
    /// 类型化运行结果及其哈希。
    pub result: ResultSection,
    /// 最近一次校验是否通过（纯展示缓存，判定以本次运行为准）。
    pub run_ok: bool,
    pub exit_code: i32,
    /// snippet 工程 Cargo.toml 原文；`{{WORKSPACE}}` 为宿主工作区占位符。
    pub cargo_toml: String,
    /// 依赖锁定（物化出的 Cargo.lock）的哈希。
    pub deps_lock_hash: String,
    /// 上游源码 commit。
    pub commit: String,
    /// 产生本清单的 CI 运行链接。
    pub ci_run: String,
    /// 校验时间（RFC 3339）。
    pub verified_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiSection {
    /// 规范化后的函数签名。签名漂移的最终裁判是 snippet 编译本身；
    /// 此清单 + 哈希保证「声明的接口面」没有被悄悄改过。
    pub symbols: Vec<String>,
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultSection {
    /// 结果载荷格式，当前固定 "json"。
    pub format: String,
    /// 规范化后的类型化运行结果（退出码 + stdout/stderr 行）。
    pub value: serde_json::Value,
    pub hash: String,
}

impl VerifyManifest {
    /// 解析 MDX 页面：切 frontmatter -> 取 verify 块 -> 校验顶层 verify_hash。
    pub fn parse_page(mdx: &str) -> Result<Self> {
        let (frontmatter, _body) = split_frontmatter(mdx)?;
        let verify_raw = extract_block_scalar(&frontmatter, "verify")?;
        let manifest: VerifyManifest = serde_json::from_str(&verify_raw)
            .with_context(|| "verify 块不是合法 JSON 或缺少必填字段".to_string())?;
        let declared = extract_plain_field(&frontmatter, "verify_hash")?;
        let actual = canonical_hash(&verify_raw)?;
        if declared != actual {
            bail!("verify_hash 不匹配: 声明 {declared}，按内容重算为 {actual}（清单被手改或哈希未回填）");
        }
        Ok(manifest)
    }

    /// 校验用镜像名。
    pub fn docker_image(&self) -> String {
        self.image.clone().unwrap_or_else(|| {
            let minor = self.toolchain.split('.').take(2).collect::<Vec<_>>().join(".");
            format!("rust:{minor}")
        })
    }
}

/// 切出 MDX frontmatter。返回 owned (frontmatter 文本, 正文)，
/// 统一换行到 \n，避免借用局部规范化副本。
pub fn split_frontmatter(mdx: &str) -> Result<(String, String)> {
    let normalized = mdx.replace("\r\n", "\n");
    let rest = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow::anyhow!("页面缺少 frontmatter（应以 --- 开头）"))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("frontmatter 未闭合（缺少结尾 ---）"))?;
    Ok((rest[..end].to_string(), rest[end + 4..].to_string()))
}

/// 提取 `key: |` 字面块标量（按块缩进聚合行）。
pub fn extract_block_scalar(frontmatter: &str, key: &str) -> Result<String> {
    let opener = format!("{key}: |");
    let mut out = String::new();
    let mut in_block = false;
    let mut block_indent: Option<usize> = None;
    for line in frontmatter.lines() {
        if !in_block {
            if line.trim_end() == opener {
                in_block = true;
            }
            continue;
        }
        if line.trim().is_empty() {
            out.push('\n');
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        match block_indent {
            Some(width) if indent < width => break, // 块结束（回到外层键）
            None => block_indent = Some(indent),
            _ => {}
        }
        let width = block_indent.unwrap();
        out.push_str(&line[width..]);
        out.push('\n');
    }
    if !in_block {
        bail!("frontmatter 中未找到 `{key}: |` 块");
    }
    Ok(out)
}

/// 提取平铺标量字段，如 `verify_hash: sha256:...`。
pub fn extract_plain_field(frontmatter: &str, key: &str) -> Result<String> {
    let prefix = format!("{key}:");
    for line in frontmatter.lines() {
        if let Some(value) = line.strip_prefix(&prefix) {
            return Ok(value.trim().to_string());
        }
    }
    bail!("frontmatter 中未找到 `{key}:`");
}

/// 规范化序列化（键排序、无空白）后取 sha256，输出 `sha256:<hex>`。
pub fn canonical_hash(json: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(json).with_context(|| "JSON 解析失败".to_string())?;
    Ok(hash_value(&value))
}

/// 对 JSON Value 取规范化哈希。serde_json 的对象是 BTreeMap：
/// to_string 天然键排序且无空白，即规范形式。
pub fn hash_value(value: &serde_json::Value) -> String {
    let canonical = serde_json::to_string(value).expect("Value 序列化不会失败");
    hash_bytes(canonical.as_bytes())
}

/// sha256 摘要，带 `sha256:` 前缀（与清单中存储格式一致）。
pub fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", bytes_to_hex(&digest))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_scalar_extracts_indented_lines() {
        let fm = "title: 日志\nverify: |\n  {\n    \"a\": 1\n  }\nverify_hash: sha256:ff\n";
        let block = extract_block_scalar(fm, "verify").unwrap();
        assert_eq!(block, "{\n  \"a\": 1\n}\n");
        assert_eq!(extract_plain_field(fm, "verify_hash").unwrap(), "sha256:ff");
    }

    #[test]
    fn canonical_hash_is_key_sorted() {
        let h1 = canonical_hash("{\"b\": 1, \"a\": [1, 2]}").unwrap();
        let h2 = canonical_hash("{\"a\": [1, 2], \"b\": 1}").unwrap();
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    #[test]
    fn split_frontmatter_handles_crlf() {
        let mdx = "---\r\ntitle: x\nverify: |\n  {}\n---\n\n# 正文\n";
        let (fm, body) = split_frontmatter(mdx).unwrap();
        assert!(fm.contains("verify: |"));
        assert!(body.contains("# 正文"));
    }
}
