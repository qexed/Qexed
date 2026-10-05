//! 配置页 MDX 渲染：标题/描述/枚举候选/校验提示全部走翻译表，
//! 渲染期回填 document，缺失翻译显式标注，多语言产物不再互相串文。
//!
//! 排版规则：字段内所有块（标题/元数据列表/描述/候选值表/提示）一律以空行分隔，
//! 元数据合并为一个连续列表放在描述之前——段落插进列表中间会把列表截断，
//! 尾部的孤项（如单独的"默认值"）会以大间距漂浮，观感即"换行没处理好"。

use qexed_doc::{interpolate, DocI18nText, DocSchema, I18nTextKind};
use qexed_language::{key_layout, Translations};
use std::collections::BTreeMap;

/// 渲染期固定文案按语言给两套（zh 系 / 其它），避免散落硬编码中文。
fn ui_label(lang: &str, key: &str) -> &'static str {
    let zh = matches!(lang, "zh-CN" | "zh" | "zh-Hans");
    match (key, zh) {
        ("type", true) => "类型",
        ("type", false) => "Type",
        ("config_key", true) => "配置键",
        ("config_key", false) => "Config key",
        ("writable", true) => "可写",
        ("writable", false) => "Writable",
        ("yes", true) => "是",
        ("yes", false) => "Yes",
        ("no", true) => "否",
        ("no", false) => "No",
        ("default", true) => "默认值",
        ("default", false) => "Default",
        ("range", true) => "取值范围",
        ("range", false) => "Range",
        ("value", true) => "值",
        ("value", false) => "Value",
        ("display_name", true) => "显示名",
        ("display_name", false) => "Display name",
        ("check_error_tip", true) => "校验失败提示",
        ("check_error_tip", false) => "Validation error message",
        ("aliases", true) => "别名",
        ("aliases", false) => "Aliases",
        ("optional", true) => "可选字段（缺省/null 均合法）",
        ("optional", false) => "Optional field (absent/null are both valid)",
        _ => "",
    }
}

/// 翻译/schema 动态文本进 MDX 前的转义。
///
/// MDX 把 {..} 当 JSX 表达式求值、<.. 当 JSX 标签解析：译文里出现 {commit}
/// 这类路由模板会让文档站运行时直接抛 "commit is not defined"。
/// 反引号包裹的行内代码保持原样（代码段里反斜杠会原样显示，不能转义）。
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;
    for c in text.chars() {
        match c {
            '`' => {
                in_code = !in_code;
                out.push('`');
            }
            '{' if !in_code => out.push_str("\\{"),
            '}' if !in_code => out.push_str("\\}"),
            '<' if !in_code => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// 用翻译表解析 DocI18nText：i18n 键查表，直接文本原样返回。
pub fn resolve_i18n(text: &DocI18nText, table: &Translations) -> String {
    match text.kind {
        I18nTextKind::I18n => table.t(&text.value),
        I18nTextKind::Text => text.value.clone(),
    }
}

/// key 是否已有真实翻译（t 未命中原样回退 key）。
fn is_untranslated(table: &Translations, key: &str) -> bool {
    table.get(key).is_none()
}

/// 缺失翻译的显式标注：文档站一眼可见哪些词条待补，不再静默串成其它语言。
fn untranslated_mark(table: &Translations, key: &str) -> &'static str {
    if is_untranslated(table, key) {
        " <!-- TODO: translate -->"
    } else {
        ""
    }
}

/// 插值后清理残留占位符：模板声明了 %{x} 而值缺失时（如无默认值字段的
/// %{default}），保留原占位符会把模板内部细节暴露给读者，这里整段去掉。
fn strip_leftover_placeholders(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("%{") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 2..];
        match after.find('}') {
            Some(end) => rest = &after[end + 1..],
            None => {
                out.push_str("%{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// %{x} 插值表：schema 自带 values + default + 变体显示名（variants 优先，缺则 select）。
/// 变体显示名参与插值，desc 模板里的 %{variants} 展示的总是当前语言的候选值。
pub fn interp_values(
    field: &qexed_doc::DocField,
    table: &Translations,
) -> BTreeMap<String, serde_json::Value> {
    let mut map = field.values.clone();
    if let Some(d) = &field.default {
        let text = match d {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        map.insert("default".to_string(), serde_json::Value::String(text));
    }
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

/// 渲染一个字段小节：标题 + 元数据列表 + 描述 + 候选值表 + 提示，块间恒为空行。
pub fn render_field(field: &qexed_doc::DocField, table: &Translations, lang: &str) -> String {
    let mut sections: Vec<String> = Vec::new();

    // 标题：命中译名，未命中回退字段短名 + 待翻译标注（不再裸奔 i18n key）。
    let mark = untranslated_mark(table, &field.key);
    let display = match table.get(&field.key) {
        Some(name) => esc(name),
        None => field.path.rsplit('.').next().unwrap_or(&field.path).to_string(),
    };
    sections.push(format!("## {display}{mark}"));

    // 元数据合并为一个连续列表：类型/配置键/可写/可选/别名/默认值/范围。
    // 保持列表不被段落打断，渲染出来是一组紧凑的条目。
    let mut meta: Vec<String> = vec![
        format!("- {}: `{}`", ui_label(lang, "type"), field.value_type),
        format!("- {}: `{}`", ui_label(lang, "config_key"), field.path),
        format!(
            "- {}: {}",
            ui_label(lang, "writable"),
            if field.writable {
                ui_label(lang, "yes")
            } else {
                ui_label(lang, "no")
            }
        ),
    ];
    if field.optional {
        meta.push(format!("- {}", ui_label(lang, "optional")));
    }
    if !field.aliases.is_empty() {
        let aliases = field
            .aliases
            .iter()
            .map(|a| format!("`{a}`"))
            .collect::<Vec<_>>()
            .join(", ");
        meta.push(format!("- {}: {aliases}", ui_label(lang, "aliases")));
    }
    if let Some(d) = &field.default {
        meta.push(format!("- {}: `{d}`", ui_label(lang, "default")));
    }
    if let (Some(min), Some(max)) = (field.min, field.max) {
        meta.push(format!(
            "- {}: `{} ≤ x ≤ {}`",
            ui_label(lang, "range"),
            format_num(min),
            format_num(max)
        ));
    }
    sections.push(meta.join("\n"));

    // 描述模板：.desc 键存在才渲染；插值 → 清理缺值占位 → MDX 转义。
    let desc_key = key_layout::desc(&field.key);
    if let Some(template) = table.get(&desc_key) {
        let rendered =
            strip_leftover_placeholders(&interpolate(template, &interp_values(field, table)));
        let rendered = esc(rendered.trim_end());
        if !rendered.is_empty() {
            sections.push(rendered);
        }
    }

    // 候选值表：variants（枚举）优先，退而 select；显示名取当前语言，未命中标注待翻译。
    let variants: Vec<&String> = if field.variants.is_empty() {
        field.select.iter().collect()
    } else {
        field.variants.iter().collect()
    };
    if !variants.is_empty() {
        let mut lines = vec![
            format!("| {} | {} |", ui_label(lang, "value"), ui_label(lang, "display_name")),
            "| --- | --- |".to_string(),
        ];
        for v in variants {
            let vkey = key_layout::variant(&field.key, v);
            let display = esc(&table.t(&vkey));
            let mark = untranslated_mark(table, &vkey);
            lines.push(format!("| `{v}` | {display}{mark} |"));
        }
        sections.push(lines.join("\n"));
    }

    // 警告 / 补充说明 / 校验失败提示：各自独立引用块。
    if let Some(warn) = &field.warn {
        sections.push(format!("> [!WARNING]\n> {}", esc(&resolve_i18n(warn, table))));
    }
    if let Some(tip) = &field.tip {
        sections.push(format!("> [!TIP]\n> {}", esc(&resolve_i18n(tip, table))));
    }
    let ce_key = key_layout::check_error(&field.key);
    if let Some(msg) = table
        .get(&ce_key)
        .map(str::to_string)
        .or_else(|| field.check_error_tip.clone())
    {
        sections.push(format!(
            "> {}: {}{}",
            ui_label(lang, "check_error_tip"),
            esc(&msg),
            untranslated_mark(table, &ce_key)
        ));
    }

    // 嵌套字段：递归展开子 schema。
    if let Some(sub) = &field.sub {
        sections.push(render_schema_section(sub, table, lang));
    }

    sections.join("\n\n") + "\n"
}

/// 递归渲染嵌套 schema：小节标题 + 各字段，块间恒为空行。
fn render_schema_section(schema: &DocSchema, table: &Translations, lang: &str) -> String {
    let mut sections: Vec<String> = Vec::new();
    if let Some(doc) = &schema.document {
        sections.push(format!("### {}", esc(doc)));
    } else if !schema.key.is_empty() {
        let t = esc(&table.t(&schema.key));
        sections.push(format!("### {t}{}", untranslated_mark(table, &schema.key)));
    }
    for field in &schema.fields {
        sections.push(render_field(field, table, lang).trim_end().to_string());
    }
    sections.join("\n\n") + "\n"
}

/// 数值展示：去掉无意义的小数尾（4294967295.0 → 4294967295）。
fn format_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// 渲染整份配置页：标题 + 默认配置 toml + 各字段详解，块间恒为空行。
pub fn render_mdx(
    slug: &str,
    schema: &DocSchema,
    table: &Translations,
    lang: &str,
    data: &str,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    // 页面标题：命中译名，未命中回退结构体短名 + 待翻译标注。
    let title = match table.get(&schema.key) {
        Some(t) => esc(t),
        None => schema.name.clone(),
    };
    sections.push(format!("# {title}{}", untranslated_mark(table, &schema.key)));

    // 默认配置文件（toml）+ 生成时间；围栏与标题同块，闭合后由分块接空行。
    let offset = chrono::FixedOffset::east_opt(8 * 60 * 60).unwrap();
    let time = chrono::Utc::now()
        .with_timezone(&offset)
        .format("%Y-%m-%d %H:%M:%S %z");
    let mut toml_block = format!(
        "## {}\n```toml\n# {}:{time}\n",
        esc(&table.t("qexed.doc.default_config_file")),
        esc(&table.t("qexed.doc.default_config_file.generation_date_label"))
    );
    toml_block.push_str(data);
    if !data.ends_with('\n') {
        toml_block.push('\n');
    }
    toml_block.push_str("```");
    sections.push(toml_block);

    // 字段详解：渲染期回填 document（t(key) + values 插值），保证每语言一份。
    let mut translated = schema.clone();
    qexed_language::translate_schema_with(&mut translated, &|key| table.t(key));
    for field in &translated.fields {
        sections.push(render_field(field, table, lang).trim_end().to_string());
    }

    let _ = slug;
    sections.join("\n\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_language::Translations;

    fn log_schema() -> DocSchema {
        serde_json::from_str(&qexed_log::config::LogConfig::schema_json()).unwrap()
    }

    fn lang_schema() -> DocSchema {
        serde_json::from_str(&qexed_language::config::LanguageConfig::schema_json()).unwrap()
    }

    #[test]
    fn interp_fills_default_and_variants() {
        let schema = log_schema();
        let level = schema.fields.iter().find(|f| f.path == "level").unwrap();
        let table = Translations::from_json(qexed_language::ZH_CN_JSON).unwrap();
        let map = interp_values(level, &table);
        assert_eq!(map["default"], "Info");
        assert_eq!(map["variants"], "追踪/调试/信息/警告/错误/关闭");
        let template = table.get(&key_layout::desc(&level.key)).unwrap();
        let out = interpolate(template, &map);
        assert_eq!(out, "日志级别，默认 Info，可选：追踪/调试/信息/警告/错误/关闭");
    }

    #[test]
    fn en_table_translates_level_and_variants() {
        let schema = log_schema();
        let table = Translations::from_json(qexed_language::EN_JSON).unwrap();
        let level = schema.fields.iter().find(|f| f.path == "level").unwrap();
        let map = interp_values(level, &table);
        assert_eq!(map["variants"], "Trace/Debug/Info/Warn/Error/Off");
        let template = table.get(&key_layout::desc(&level.key)).unwrap();
        let out = interpolate(template, &map);
        assert_eq!(
            out,
            "Log level, default Info, options: Trace/Debug/Info/Warn/Error/Off"
        );
    }

    #[test]
    fn blocks_are_separated_by_blank_lines() {
        // toml 围栏闭合后必须空一行再进字段标题；
        // 元数据列表保持连续（默认值在列表尾部），描述段落整体在列表之后。
        let table = Translations::from_json(qexed_language::ZH_CN_JSON).unwrap();
        let mdx = render_mdx("log", &log_schema(), &table, "zh-CN", "level = \"Info\"\n");
        assert!(mdx.contains("```\n\n## 日志级别\n"), "{mdx}");
        assert!(
            mdx.contains("- 默认值: `\"DAY\"`\n\n按时间切割：每小时/每天/每月"),
            "{mdx}"
        );
        assert!(mdx.contains("追踪/调试/信息/警告/错误/关闭"), "{mdx}");
        // 1 个默认配置小节 + 6 个字段小节。
        assert_eq!(mdx.matches("\n## ").count(), 7, "{mdx}");
    }

    #[test]
    fn en_page_uses_english_table() {
        let table = Translations::from_json(qexed_language::EN_JSON).unwrap();
        let mdx = render_mdx("log", &log_schema(), &table, "en", "");
        assert!(mdx.contains("# Log\n"), "{mdx}");
        assert!(mdx.contains("## Log level\n"), "{mdx}");
        assert!(mdx.contains("Rotation mode"), "{mdx}");
        // LogConfig 全部词条都在英文表里：不允许裸 i18n key 或待翻译标注漏出。
        assert!(!mdx.contains("qexed.crates.log"), "{mdx}");
        assert!(!mdx.contains("TODO: translate"), "{mdx}");
    }

    #[test]
    fn untranslated_gets_todo_mark() {
        // 表里没有的键标注待翻译；命中的键无标注。
        let table = Translations::from_json(qexed_language::EN_JSON).unwrap();
        assert_eq!(
            untranslated_mark(&table, "qexed.missing.key.for.test"),
            " <!-- TODO: translate -->"
        );
        assert_eq!(untranslated_mark(&table, "qexed.crates.log.config.LogConfig"), "");
    }

    #[test]
    fn missing_default_placeholder_is_stripped() {
        // 模板声明了 %{default} 而字段无默认值时，占位符整段清理，不残留 %{。
        let table = Translations::from_json(qexed_language::ZH_CN_JSON).unwrap();
        let mdx = render_mdx("log", &log_schema(), &table, "zh-CN", "");
        assert!(!mdx.contains("%{"), "leftover placeholder: {mdx}");
        assert!(mdx.contains("## 日志文件路径\n"), "{mdx}");
    }

    #[test]
    fn desc_braces_safe_for_mdx() {
        // server_url.desc 的路由模板放进行内代码：代码段内 {commit} 按字面保留，
        // 段外一律转义，页面不再运行时抛 "commit is not defined"。
        let table = Translations::from_json(qexed_language::ZH_CN_JSON).unwrap();
        let mdx = render_mdx("language", &lang_schema(), &table, "zh-CN", "");
        assert!(
            mdx.contains("- `GET /api/v1/qexed/{commit}/language.json`：获取语言列表"),
            "{mdx}"
        );
        assert!(!mdx.contains("%{"), "leftover placeholder: {mdx}");
    }

    #[test]
    fn strip_leftover_placeholders_basics() {
        assert_eq!(strip_leftover_placeholders("a %{x} b"), "a  b");
        assert_eq!(strip_leftover_placeholders("no placeholder"), "no placeholder");
        assert_eq!(strip_leftover_placeholders("unclosed %{x"), "unclosed %{x");
    }

    #[test]
    fn esc_basics() {
        assert_eq!(esc("a{b}c<d"), "a\\{b\\}c&lt;d");
        assert_eq!(esc("plain"), "plain");
        // 行内代码里的花括号保持原样。
        assert_eq!(esc("`{x}`"), "`{x}`");
        assert_eq!(esc("a `c{o}` b{d}"), "a `c{o}` b\\{d\\}");
    }
}
