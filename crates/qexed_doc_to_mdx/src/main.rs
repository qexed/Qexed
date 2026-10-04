use shadow_rs::shadow;
shadow!(shadow);

use qexed_doc::{interpolate, DocSchema, I18nTextKind};
use qexed_language::{key_layout, Translations};
use std::path::PathBuf;

/// 一个要导出的配置 schema 条目。
struct ExportItem {
    /// 文件名（不含扩展名），如 "log"。
    slug: &'static str,
    /// schema JSON 文本（运行时从各配置 crate 拿）。
    schema_json: String,
}

/// 当前参与导出的配置 schema 清单：新增配置 crate 时在此登记。
fn export_items() -> anyhow::Result<Vec<ExportItem>> {
    Ok(vec![ExportItem {
        slug: "log",
        schema_json: qexed_log::config::LogConfig::schema_json(),
    }])
}

/// 多语言文档生成器：把配置 schema 渲染成每语言一份的 MDX。
#[derive(Debug, clap::Parser)]
struct Args {
    /// 目标语言列表，逗号分隔（翻译表按语言加载；缺失键回退 key 并标注 TODO）。
    #[arg(long, default_value = "zh-CN,en-US")]
    langs: String,
    /// 输出根目录：生成 <out>/<lang>/<slug>.mdx。
    #[arg(long, default_value = "./docs-generated")]
    out: PathBuf,
}

/// 用翻译表解析 DocI18nText：i18n 键查表，直接文本原样返回。
fn resolve_i18n(text: &qexed_doc::DocI18nText, table: &Translations) -> String {
    match text.kind {
        I18nTextKind::I18n => table.t(&text.value),
        I18nTextKind::Text => text.value.clone(),
    }
}

/// 插值专用：default + values 合并成插值表。
fn interp_values(
    field: &qexed_doc::DocField,
    table: &Translations,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut map = field.values.clone();
    if let Some(d) = &field.default {
        // JSON 字符串值去掉引号;其它类型保持 JSON 形态
        let text = match d {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        map.insert("default".to_string(), serde_json::Value::String(text));
    }
    // variants:值显示名序列(经翻译),供 desc 模板 {variants} 插值
    let variants: Vec<&String> = if field.variants.is_empty() {
        field.select.iter().collect()
    } else {
        field.variants.iter().collect()
    };
    if !variants.is_empty() {
        let names: Vec<String> = variants
            .iter()
            .map(|v| table.t(&key_layout::variant(&field.key, v)))
            .collect();
        map.insert(
            "variants".to_string(),
            serde_json::Value::String(names.join("/")),
        );
    }
    map
}

/// 渲染一个字段的 MDX 段落。
fn render_field(field: &qexed_doc::DocField, table: &Translations, lang: &str) -> String {
    let mut out = String::new();
    let name = table.t(&field.key);
    out.push_str(&format!("## {name}\n\n"));
    // 元信息行
    out.push_str(&format!("- 类型：`{}`\n", field.value_type));
    out.push_str(&format!("- 配置键：`{}`\n", field.path));
    out.push_str(&format!("- 可写：{}\n", if field.writable { "是" } else { "否" }));
    // 描述：desc 模板插值，缺失时跳过
    let desc_key = key_layout::desc(&field.key);
    if let Some(template) = table.get(&desc_key) {
        let rendered = interpolate(template, &interp_values(field, table));
        out.push_str(&format!("\n{rendered}\n"));
    }
    // 默认值
    if let Some(d) = &field.default {
        out.push_str(&format!("\n- 默认值：`{d}`\n"));
    }
    // 候选值表：变体显示名走 key_layout::variant
    let variants: Vec<&String> = if field.variants.is_empty() {
        field.select.iter().collect()
    } else {
        field.variants.iter().collect()
    };
    if !variants.is_empty() {
        out.push_str("\n| 值 | 显示名 |\n| --- | --- |\n");
        for v in variants {
            let display = table.t(&key_layout::variant(&field.key, v));
            let untranslated = display == *v && lang != "zh-CN";
            let mark = if untranslated { " `<!-- TODO: translate -->`" } else { "" };
            out.push_str(&format!("| `{v}` | {display}{mark} |\n"));
        }
    }
    // 警告 / 提示
    if let Some(warn) = &field.warn {
        out.push_str(&format!("\n> [!WARNING]\n> {}\n", resolve_i18n(warn, table)));
    }
    if let Some(tip) = &field.tip {
        out.push_str(&format!("\n> [!TIP]\n> {}\n", resolve_i18n(tip, table)));
    }
    // 校验失败提示
    let ce = key_layout::check_error(&field.key);
    if let Some(msg) = table.get(&ce) {
        out.push_str(&format!("\n> 校验失败提示：{msg}\n"));
    }
    out
}

/// 渲染整个 schema 为一份 MDX 文本。
fn render_mdx(slug: &str, schema: &DocSchema, table: &Translations, lang: &str) -> String {
    let title = match table.get(&schema.key) {
        Some(t) => t.to_string(),
        None => schema.key.clone(),
    };
    let mut out = format!("# {title}\n\n");
    out.push_str(&format!("<!-- generator: qexed_doc_to_mdx | lang: {lang} | schema: {slug} -->\n\n"));
    for field in &schema.fields {
        out.push_str(&render_field(field, table, lang));
        out.push_str("\n");
    }
    out
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Args = clap::Parser::parse();
    qexed_config::init_config_path("./config".into())?;
    let langs: Vec<&str> = args.langs.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let items = export_items()?;
    let mut generated = 0;
    for lang in &langs {
        let table = qexed_language::load_translations(shadow::SHORT_COMMIT, lang).await?;
        let lang_dir = args.out.join(lang);
        tokio::fs::create_dir_all(&lang_dir).await?;
        for item in &items {
            let schema: DocSchema = serde_json::from_str(&item.schema_json)?;
            let mdx = render_mdx(item.slug, &schema, &table, lang);
            let file = lang_dir.join(format!("{}.mdx", item.slug));
            tokio::fs::write(&file, mdx).await?;
            generated += 1;
            println!("generated {}", file.display());
        }
    }
    println!("done: {generated} files across {} languages", langs.len());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interp_fills_default_and_variants() {
        let schema: DocSchema =
            serde_json::from_str(&qexed_log::config::LogConfig::schema_json()).unwrap();
        let level = schema.fields.iter().find(|f| f.path == "level").unwrap();
        let table = Translations::from_json(qexed_language::ZH_CN_JSON).unwrap();
        let map = interp_values(level, &table);
        assert_eq!(map["default"], "Info");
        assert_eq!(map["variants"], "追踪/调试/信息/警告/错误/关闭");
        let template = table.get(&key_layout::desc(&level.key)).unwrap();
        let out = interpolate(template, &map);
        assert_eq!(out, "日志级别，默认 Info，可选：追踪/调试/信息/警告/错误/关闭");
    }
}
