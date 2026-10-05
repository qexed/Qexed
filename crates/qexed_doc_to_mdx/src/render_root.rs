/// 站点根入口页：`app/page.mdx`（Next.js 根路由 `/`）。
///
/// 提交 hash + 语言链接列表。语言链接用相对路径 `./<lang>`，从 `/` 出发解析正确。
pub fn render_root_page(langs: &[&str]) -> String {
    let commit = crate::shadow::COMMIT_HASH;
    let mut out = String::new();
    out.push_str("import CommitToGithubHref from \"@components/CommitToGithubHref\";\n\n");
    out.push_str("# Qexed 配置文档\n\n");
    out.push_str(&format!("- 提交: <CommitToGithubHref commit=\"{commit}\"/>\n\n"));
    for lang in langs {
        out.push_str(&format!("- [{lang}](./{lang})\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_page_lists_all_langs() {
        let page = render_root_page(&["zh-CN", "en-US"]);
        assert!(page.contains("- [zh-CN](./zh-CN)"), "{page}");
        assert!(page.contains("- [en-US](./en-US)"), "{page}");
    }
}
