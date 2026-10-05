use crate::export_item::ExportItem;
use crate::route::{route_segments, url_path};
use qexed_config::error::ConfigError;
use qexed_doc::DocSchema;
use qexed_language::Translations;
// import CommitToGithubHref from "@components/CommitToGithubHref";
// import ConfigAppList from "@components/ConfigAppList";

// export const AppTree = [
//   { id: "log", name: "日志", type: "file" },
//   { id: "language", name: "语言", type: "file" },
// ]

// # Qexed 配置文档 - 简体中文

// - 提交: <CommitToGithubHref commit="44c79aabfc6433b28502cbb327a5dc85ef2792bc"/>
// - 应用数量: `2`

// <ConfigAppList fileTree={AppTree}/>
/// 语言目录入口页：`app/<lang>/page.mdx`。
///
/// 按 `items` 的注册顺序列每个 config，保证文档站上的应用列表顺序稳定、
/// 与 `export_items()` 里登记的先后一致。
pub fn render_lang_index(items: &[ExportItem], table: &Translations) -> anyhow::Result<String> {
    let commit = crate::shadow::COMMIT_HASH;

    let mut out = String::new();

    out.push_str("import CommitToGithubHref from \"@components/CommitToGithubHref\";\n");
    out.push_str("import ConfigAppList from \"@components/ConfigAppList\";\n\n");
    out.push_str("export const AppTree = [\n");
    for item in items {
        let id = to_ts_path(&qexed_config::config_doc_path(item.path, item.name)?)?;
        // let schema: serde_json::Value = serde_json::from_str(&item.schema_json)?;
        // 译名未命中时回退路由短名（如 "log"），不让 i18n key 裸露在列表里。
        // name 落进 AppTree 的 TS 字符串字面量：引号/换行等按 TS 规则转义。
        let name = escape_ts(table.get(&item.schema.key).unwrap_or(item.name));

        out.push_str(&format!(
            "    {{id:\"{}\",name:\"{}\",type:\"file\"}},\n",
            id, name,
        ));
        // out.push_str(&format!(
        //     "- [{}](./{}): `{}` {}\n",
        //     item.name,
        //     rel,
        //     fields,
        //     table.t("qexed.doc.fields")
        // ));
    }
    out.push_str("]\n\n");

    out.push_str(&format!(
        "# {} - {}\n\n",
        table.t("qexed.doc.title"),
        table.t("language")
    ));
    out.push_str(&format!(
        "- {}: <CommitToGithubHref commit=\"{commit}\"/>\n",
        table.t("qexed.doc.commit")
    ));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        table.t("qexed.doc.apps"),
        items.len()
    ));
    out.push_str("<ConfigAppList fileTree={AppTree}/>\n");

    Ok(out)
}
fn to_ts_path(p: &std::path::Path) -> Result<String, ConfigError> {
    // 1) 拿到 &str
    let s = p
        .to_str()
        .ok_or_else(|| ConfigError::InvalidPath(format!("path is not valid UTF-8: {p:?}")))?;
    // 2) 统一成 /
    let s = s.replace('\\', "/");
    // 3) 转义 TS 字符串里的特殊字符
    Ok(escape_ts(&s))
}

fn escape_ts(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}
/// 数一个 schema 的字段个数；解析失败按 0 处理（骨架阶段不阻断生成）。
fn count_fields(schema_json: &str) -> usize {
    serde_json::from_str::<DocSchema>(schema_json)
        .map(|s| s.fields.len())
        .unwrap_or(0)
}
