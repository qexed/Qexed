use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, ValueEnum};
use qexed_config::{
    app::{
        qexed::Qexed, qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
        qexed_warden::QexedWarden,
    },
    build,
    tool::{AppConfigTrait, AutoDocConfigTrait},
};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

const DEFAULT_LANGS: &[&str] = &["zh-CN", "en"];
const LOGO_SOURCE: &str = "crates/qexed/logo.ico";
rust_i18n::i18n!("./locales");
#[derive(Debug, Parser)]
#[command(name = "qexed_config_to_mdx")]
#[command(about = "Generate Qexed AutoDoc config documentation packages.")]
struct Args {
    /// 输出根目录。实际内容会生成在 <out>/<commit>/ 下。
    #[arg(long, default_value = "docs/config")]
    out: PathBuf,

    /// 文档语言，可重复传入；默认生成 zh-CN 与 en。
    #[arg(long = "lang")]
    langs: Vec<String>,

    /// 输出格式，可重复传入；默认 all。
    #[arg(long = "format", value_enum)]
    formats: Vec<OutputFormat>,

    /// 覆盖输出目录使用的 commit hash。
    #[arg(long)]
    commit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    All,
    Markdown,
    Mdx,
    NextApp,
    NextPages,
    Mkdocs,
    Mdbook,
    Json,
}

impl OutputFormat {
    fn name(&self) -> String {
        match self {
            Self::All => "all",
            Self::Markdown => "markdown",
            Self::Mdx => "mdx",
            Self::NextApp => "next-app",
            Self::NextPages => "next-pages",
            Self::Mkdocs => "mkdocs",
            Self::Mdbook => "mdbook",
            Self::Json => "json",
        }
        .to_string()
    }
}

#[derive(Debug, Serialize)]
struct Manifest {
    commit: String,
    generator: &'static str,
    generated_formats: Vec<String>,
    languages: Vec<String>,
    apps: Vec<AppManifest>,
    asset_logo: String,
}

#[derive(Debug, Serialize)]
struct AppManifest {
    name: String,
    config_file: String,
    field_count: usize,
}

#[derive(Debug, Serialize)]
struct DocBundle {
    commit: String,
    languages: Vec<LanguageDocs>,
}

#[derive(Debug, Serialize)]
struct LanguageDocs {
    lang: String,
    apps: Vec<DocApp>,
}

#[derive(Debug, Clone, Serialize)]
struct DocApp {
    name: String,
    config_file: String,
    config_path: String,
    fields: Vec<DocField>,
}

#[derive(Debug, Clone, Serialize)]
struct DocField {
    path: String,
    description: String,
    value_type: String,
    default_value: Option<String>,
    warning: Option<String>,
    danger: Option<String>,
    pending_deprecated: Option<String>,
    deprecated: Option<String>,
    migration_notice: Option<String>,
}

#[derive(Debug, Default)]
struct NoticeMaps {
    warnings: BTreeMap<String, String>,
    dangers: BTreeMap<String, String>,
    pending_deprecated: BTreeMap<String, String>,
    deprecated: BTreeMap<String, String>,
    migration_notices: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
struct UiText {
    site_title: &'static str,
    commit: &'static str,
    language: &'static str,
    config_file: &'static str,
    field_count: &'static str,
    path: &'static str,
    value_type: &'static str,
    default_value: &'static str,
    description: &'static str,
    notice: &'static str,
    none: &'static str,
    danger: &'static str,
    warning: &'static str,
    pending_deprecated: &'static str,
    deprecated: &'static str,
    migration: &'static str,
    target: &'static str,
    asset: &'static str,
    index: &'static str,
    home: &'static str,
    app_count: &'static str,
    field_unit: &'static str,
    summary: &'static str,
    complex_details: &'static str,
    site_description_prefix: &'static str,
}

fn ui_text(lang: &str) -> UiText {
    if lang.to_ascii_lowercase().starts_with("zh") {
        UiText {
            site_title: "Qexed 配置文档",
            commit: "提交",
            language: "语言",
            config_file: "配置文件",
            field_count: "字段数量",
            path: "配置项",
            value_type: "类型",
            default_value: "默认值",
            description: "说明",
            notice: "提示",
            none: "无",
            danger: "危险",
            warning: "警告",
            pending_deprecated: "即将废弃",
            deprecated: "已废弃",
            migration: "迁移",
            target: "目标",
            asset: "资源",
            index: "索引",
            home: "首页",
            app_count: "应用数量",
            field_unit: "个字段",
            summary: "目录",
            complex_details: "复杂类型详情",
            site_description_prefix: "AutoDoc 生成自提交",
        }
    } else {
        UiText {
            site_title: "Qexed Config Docs",
            commit: "Commit",
            language: "Language",
            config_file: "Config file",
            field_count: "Field count",
            path: "Path",
            value_type: "Type",
            default_value: "Default",
            description: "Description",
            notice: "Notice",
            none: "None",
            danger: "Danger",
            warning: "Warning",
            pending_deprecated: "Pending deprecated",
            deprecated: "Deprecated",
            migration: "Migration",
            target: "Target",
            asset: "Asset",
            index: "Index",
            home: "Home",
            app_count: "Apps",
            field_unit: "fields",
            summary: "Summary",
            complex_details: "Complex Type Details",
            site_description_prefix: "AutoDoc generated at",
        }
    }
}

fn bundle_ui_text(bundle: &DocBundle) -> UiText {
    bundle
        .languages
        .first()
        .map(|language_docs| ui_text(&language_docs.lang))
        .unwrap_or_else(|| ui_text("en"))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let langs = normalized_langs(args.langs);
    let formats = normalized_formats(args.formats);
    let commit = args
        .commit
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| build::COMMIT_HASH.to_string());

    let output_root = args.out.join(&commit);
    fs::create_dir_all(&output_root)
        .with_context(|| format!("无法创建输出目录 {}", output_root.display()))?;

    let bundle = collect_bundle(&commit, &langs)?;
    write_assets(&output_root)?;

    if formats.contains(&OutputFormat::Markdown) {
        write_markdown_docs(&output_root, &bundle, false)?;
    }
    if formats.contains(&OutputFormat::Mdx) {
        write_markdown_docs(&output_root, &bundle, true)?;
    }
    if formats.contains(&OutputFormat::NextApp) {
        write_next_app_docs(&output_root, &bundle)?;
    }
    if formats.contains(&OutputFormat::NextPages) {
        write_next_pages_docs(&output_root, &bundle)?;
    }
    if formats.contains(&OutputFormat::Mkdocs) {
        write_mkdocs_docs(&output_root, &bundle)?;
    }
    if formats.contains(&OutputFormat::Mdbook) {
        write_mdbook_docs(&output_root, &bundle)?;
    }
    if formats.contains(&OutputFormat::Json) {
        write_json_docs(&output_root, &bundle)?;
    }

    write_manifest(&output_root, &bundle, &formats)?;

    println!("generated config docs: {}", output_root.display());
    Ok(())
}

fn normalized_langs(langs: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let source = if langs.is_empty() {
        DEFAULT_LANGS
            .iter()
            .map(|lang| lang.to_string())
            .collect::<Vec<_>>()
    } else {
        langs
    };

    source
        .into_iter()
        .filter_map(|lang| {
            let lang = lang.trim().to_string();
            if lang.is_empty() || !seen.insert(lang.clone()) {
                None
            } else {
                Some(lang)
            }
        })
        .collect()
}

fn normalized_formats(formats: Vec<OutputFormat>) -> Vec<OutputFormat> {
    if formats.is_empty() || formats.contains(&OutputFormat::All) {
        return vec![
            OutputFormat::Markdown,
            OutputFormat::Mdx,
            OutputFormat::NextApp,
            OutputFormat::NextPages,
            OutputFormat::Mkdocs,
            OutputFormat::Mdbook,
            OutputFormat::Json,
        ];
    }

    let mut result = Vec::new();
    for format in formats {
        if format != OutputFormat::All && !result.contains(&format) {
            result.push(format);
        }
    }
    result
}

fn collect_bundle(commit: &str, langs: &[String]) -> Result<DocBundle> {
    let mut languages = Vec::new();

    for lang in langs {
        languages.push(LanguageDocs {
            lang: lang.clone(),
            apps: vec![
                collect_app::<Qexed>("qexed", "qexed.toml", lang)?,
                collect_app::<QexedWarden>("qexed_warden", "qexed_warden.toml", lang)?,
                collect_app::<QexedIpConnectionSpeedTest>(
                    "qexed_ip_connect_speed_test",
                    "qexed_ip_connect_speed_test.toml",
                    lang,
                )?,
            ],
        });
    }

    Ok(DocBundle {
        commit: commit.to_string(),
        languages,
    })
}

fn collect_app<T>(name: &str, config_file: &str, lang: &str) -> Result<DocApp>
where
    T: AppConfigTrait + AutoDocConfigTrait + Serialize + Default,
{
    let defaults = serde_json::to_value(T::default())
        .with_context(|| format!("无法序列化 {name} 默认配置"))?;
    let notices = NoticeMaps {
        warnings: into_map(T::warning_fields(lang)),
        dangers: into_map(T::danger_fields(lang)),
        pending_deprecated: into_map(T::pending_deprecated_fields(lang)),
        deprecated: into_map(T::deprecation_fields(lang)),
        migration_notices: into_map(T::migration_notice_fields(lang)),
    };

    let mut fields = Vec::new();
    for (path, description) in T::doc_fields(lang) {
        let default = value_at_path(&defaults, &path);
        fields.push(DocField {
            value_type: default
                .map(value_type_name)
                .unwrap_or("unknown")
                .to_string(),
            default_value: default.and_then(|value| display_default_value(&path, value)),
            warning: notices.warnings.get(&path).cloned(),
            danger: notices.dangers.get(&path).cloned(),
            pending_deprecated: notices.pending_deprecated.get(&path).cloned(),
            deprecated: notices.deprecated.get(&path).cloned(),
            migration_notice: notices.migration_notices.get(&path).cloned(),
            path,
            description,
        });
    }

    fields.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(DocApp {
        name: name.to_string(),
        config_file: config_file.to_string(),
        config_path: config_path::<T>(config_file),
        fields,
    })
}

fn into_map(values: Vec<(String, String)>) -> BTreeMap<String, String> {
    values.into_iter().collect()
}

fn config_path<T: AppConfigTrait>(config_file: &str) -> String {
    let base = T::PATH.trim_matches('/');
    if base.is_empty() {
        format!("config/{config_file}")
    } else {
        format!("config/{base}/{config_file}")
    }
}

fn value_at_path<'a>(root: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    let mut current = root;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn value_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(number) if number.is_i64() || number.is_u64() => "integer",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn display_default_value(path: &str, value: &JsonValue) -> Option<String> {
    let sanitized = sanitize_default_value(path, value);
    if sanitized.is_null() {
        return None;
    }

    let mut rendered = sanitized.to_string();
    if rendered.len() > 320 {
        rendered.truncate(320);
        rendered.push_str("...");
    }

    Some(rendered)
}

fn sanitize_default_value(path: &str, value: &JsonValue) -> JsonValue {
    if is_sensitive_path(path) {
        return JsonValue::String("<random>".to_string());
    }

    match value {
        JsonValue::Array(values) => JsonValue::Array(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| sanitize_default_value(&format!("{path}.{index}"), value))
                .collect(),
        ),
        JsonValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        sanitize_default_value(&format!("{path}.{key}"), value),
                    )
                })
                .collect(),
        ),
        JsonValue::String(value) if value.len() > 160 => {
            JsonValue::String(format!("{}...", &value[..160]))
        }
        _ => value.clone(),
    }
}

fn is_sensitive_path(path: &str) -> bool {
    path.split('.').any(|part| {
        let part = part.to_ascii_lowercase();
        part.contains("password")
            || part.contains("token")
            || part.contains("secret")
            || part.contains("private_key")
            || part == "key"
    })
}

fn write_assets(output_root: &Path) -> Result<()> {
    let asset_dir = output_root.join("assets");
    fs::create_dir_all(&asset_dir)
        .with_context(|| format!("无法创建资源目录 {}", asset_dir.display()))?;

    let logo_source = Path::new(LOGO_SOURCE);
    let logo_target = asset_dir.join("logo.ico");
    fs::copy(logo_source, &logo_target).with_context(|| {
        format!(
            "无法复制图标 {} 到 {}",
            logo_source.display(),
            logo_target.display()
        )
    })?;
    Ok(())
}

fn write_markdown_docs(output_root: &Path, bundle: &DocBundle, mdx: bool) -> Result<()> {
    let dir = output_root.join(if mdx { "mdx" } else { "markdown" });
    fs::create_dir_all(&dir).with_context(|| format!("无法创建目录 {}", dir.display()))?;

    for language_docs in &bundle.languages {
        let lang_dir = dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建目录 {}", lang_dir.display()))?;

        for app in &language_docs.apps {
            let extension = if mdx { "mdx" } else { "md" };
            let path = lang_dir.join(format!("{}.{}", app.name, extension));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, mdx);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    Ok(())
}

fn write_next_pages_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let pages_dir = output_root.join("next-pages").join("pages").join("config");
    fs::create_dir_all(&pages_dir)
        .with_context(|| format!("无法创建 Next.js pages 目录 {}", pages_dir.display()))?;

    let index_path = pages_dir.join("index.mdx");
    fs::write(&index_path, render_next_index(bundle))
        .with_context(|| format!("无法写入 {}", index_path.display()))?;

    for language_docs in &bundle.languages {
        let lang_dir = pages_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建目录 {}", lang_dir.display()))?;

        let meta = json!({
            "title": format!("Qexed Config {}", language_docs.lang),
            "pages": language_docs.apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        });
        let meta_path = lang_dir.join("_meta.json");
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("无法写入 {}", meta_path.display()))?;

        for app in &language_docs.apps {
            let path = lang_dir.join(format!("{}.mdx", app.name));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, true);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    Ok(())
}

fn write_next_app_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let docs_dir = output_root
        .join("next-app")
        .join("app")
        .join("docs")
        .join("config");
    fs::create_dir_all(&docs_dir)
        .with_context(|| format!("无法创建 Next.js app 目录 {}", docs_dir.display()))?;

    let index_path = docs_dir.join("page.mdx");
    fs::write(&index_path, render_next_app_index(bundle))
        .with_context(|| format!("无法写入 {}", index_path.display()))?;

    for language_docs in &bundle.languages {
        let lang_dir = docs_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建目录 {}", lang_dir.display()))?;

        let lang_index_path = lang_dir.join("page.mdx");
        fs::write(
            &lang_index_path,
            render_next_app_language_index(&bundle.commit, language_docs),
        )
        .with_context(|| format!("无法写入 {}", lang_index_path.display()))?;

        for app in &language_docs.apps {
            let app_dir = lang_dir.join(&app.name);
            fs::create_dir_all(&app_dir)
                .with_context(|| format!("无法创建目录 {}", app_dir.display()))?;

            let path = app_dir.join("page.mdx");
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, false);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    Ok(())
}

fn write_mkdocs_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let root = output_root.join("mkdocs");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&docs_dir)
        .with_context(|| format!("无法创建 MkDocs docs 目录 {}", docs_dir.display()))?;

    let assets_dir = docs_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("无法创建 MkDocs assets 目录 {}", assets_dir.display()))?;
    fs::copy(
        output_root.join("assets").join("logo.ico"),
        assets_dir.join("logo.ico"),
    )
    .with_context(|| "无法复制 MkDocs 图标资源")?;

    fs::write(
        docs_dir.join("index.md"),
        render_site_index(bundle, "MkDocs"),
    )
    .with_context(|| "无法写入 MkDocs 首页")?;

    for language_docs in &bundle.languages {
        let lang_dir = docs_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建 MkDocs 语言目录 {}", lang_dir.display()))?;

        fs::write(
            lang_dir.join("index.md"),
            render_language_index(&bundle.commit, language_docs),
        )
        .with_context(|| format!("无法写入 MkDocs {} 首页", language_docs.lang))?;

        for app in &language_docs.apps {
            let path = lang_dir.join(format!("{}.md", app.name));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, false);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    fs::write(root.join("mkdocs.yml"), render_mkdocs_config(bundle))
        .with_context(|| "无法写入 mkdocs.yml")?;

    Ok(())
}

fn write_mdbook_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let root = output_root.join("mdbook");
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("无法创建 mdBook src 目录 {}", src_dir.display()))?;

    let assets_dir = src_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("无法创建 mdBook assets 目录 {}", assets_dir.display()))?;
    fs::copy(
        output_root.join("assets").join("logo.ico"),
        assets_dir.join("logo.ico"),
    )
    .with_context(|| "无法复制 mdBook 图标资源")?;

    fs::write(
        src_dir.join("index.md"),
        render_site_index(bundle, "mdBook"),
    )
    .with_context(|| "无法写入 mdBook 首页")?;

    for language_docs in &bundle.languages {
        let lang_dir = src_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建 mdBook 语言目录 {}", lang_dir.display()))?;

        fs::write(
            lang_dir.join("index.md"),
            render_language_index(&bundle.commit, language_docs),
        )
        .with_context(|| format!("无法写入 mdBook {} 首页", language_docs.lang))?;

        for app in &language_docs.apps {
            let path = lang_dir.join(format!("{}.md", app.name));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, false);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    fs::write(root.join("book.toml"), render_mdbook_config(bundle))
        .with_context(|| "无法写入 book.toml")?;
    fs::write(src_dir.join("SUMMARY.md"), render_mdbook_summary(bundle))
        .with_context(|| "无法写入 SUMMARY.md")?;

    Ok(())
}

fn write_json_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let dir = output_root.join("json");
    fs::create_dir_all(&dir).with_context(|| format!("无法创建目录 {}", dir.display()))?;

    let path = dir.join("qexed_config_docs.json");
    fs::write(&path, serde_json::to_string_pretty(bundle)?)
        .with_context(|| format!("无法写入 {}", path.display()))?;

    Ok(())
}

fn write_manifest(output_root: &Path, bundle: &DocBundle, formats: &[OutputFormat]) -> Result<()> {
    let first_language = bundle
        .languages
        .first()
        .ok_or_else(|| anyhow!("缺少文档语言"))?;

    let manifest = Manifest {
        commit: bundle.commit.clone(),
        generator: "qexed_config_to_mdx",
        generated_formats: formats.iter().map(OutputFormat::name).collect(),
        languages: bundle
            .languages
            .iter()
            .map(|docs| docs.lang.clone())
            .collect(),
        apps: first_language
            .apps
            .iter()
            .map(|app| AppManifest {
                name: app.name.clone(),
                config_file: app.config_file.clone(),
                field_count: app.fields.len(),
            })
            .collect(),
        asset_logo: "assets/logo.ico".to_string(),
    };

    let path = output_root.join("manifest.json");
    fs::write(&path, serde_json::to_string_pretty(&manifest)?)
        .with_context(|| format!("无法写入 {}", path.display()))?;
    Ok(())
}

fn render_markdown_app(commit: &str, lang: &str, app: &DocApp, mdx: bool) -> String {
    let ui = ui_text(lang);
    let mut out = String::new();

    if mdx {
        out.push_str("---\n");
        out.push_str(&format!("title: \"{}\"\n", escape_yaml(&app.name)));
        out.push_str(&format!("commit: \"{}\"\n", escape_yaml(commit)));
        out.push_str(&format!("lang: \"{}\"\n", escape_yaml(lang)));
        out.push_str("---\n\n");
    }

    out.push_str(&format!("# {}\n\n", app.name));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!("- {}: `{lang}`\n", ui.language));
    out.push_str(&format!("- {}: `{}`\n", ui.config_file, app.config_path));
    out.push_str(&format!("- {}: `{}`\n\n", ui.field_count, app.fields.len()));

    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n",
        ui.path, ui.value_type, ui.default_value, ui.description, ui.notice
    ));
    out.push_str("| --- | --- | --- | --- | --- |\n");

    for field in &app.fields {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            escape_markdown_table(&field.path),
            escape_markdown_table(&field.value_type),
            render_default_cell(field.default_value.as_deref(), ui.none),
            escape_markdown_table(&field.description),
            render_notice_cell(field, &ui),
        ));
    }

    let complex_details = render_complex_details(app, &ui);
    if !complex_details.is_empty() {
        out.push('\n');
        out.push_str(&complex_details);
    }

    out
}

fn render_complex_details(app: &DocApp, ui: &UiText) -> String {
    let complex_fields = app
        .fields
        .iter()
        .filter(|field| matches!(field.value_type.as_str(), "object" | "array"))
        .collect::<Vec<_>>();
    if complex_fields.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!("## {}\n\n", ui.complex_details));

    for field in complex_fields {
        let child_prefix = format!("{}.", field.path);
        let children = app
            .fields
            .iter()
            .filter(|candidate| is_direct_child_path(&candidate.path, &child_prefix))
            .collect::<Vec<_>>();

        out.push_str(&format!("### `{}`\n\n", field.path));
        out.push_str(&format!(
            "- {}: `{}`\n",
            ui.value_type,
            escape_markdown_table(&field.value_type)
        ));
        if let Some(default_value) = &field.default_value {
            out.push_str(&format!(
                "- {}: `{}`\n",
                ui.default_value,
                escape_code_span(default_value)
            ));
        }
        out.push_str(&format!(
            "- {}: {}\n",
            ui.description,
            escape_markdown(&field.description)
        ));
        let notice = render_notice_text(field, ui);
        if !notice.is_empty() {
            out.push_str(&format!("- {}: {}\n", ui.notice, notice));
        }

        if children.is_empty() {
            out.push('\n');
            continue;
        }

        out.push('\n');
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            ui.path, ui.value_type, ui.default_value, ui.description, ui.notice
        ));
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for child in children {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} | {} |\n",
                escape_markdown_table(&child.path),
                escape_markdown_table(&child.value_type),
                render_default_cell(child.default_value.as_deref(), ui.none),
                escape_markdown_table(&child.description),
                render_notice_cell(child, ui),
            ));
        }
        out.push('\n');
    }

    out
}

fn render_default_cell(value: Option<&str>, none_text: &str) -> String {
    value
        .map(|value| format!("`{}`", escape_code_span(value)))
        .unwrap_or_else(|| escape_markdown_table(none_text))
}

fn is_direct_child_path(path: &str, parent_prefix: &str) -> bool {
    path.strip_prefix(parent_prefix)
        .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('.'))
}

fn render_notice_cell(field: &DocField, ui: &UiText) -> String {
    let mut notices = Vec::new();
    push_notice(&mut notices, ui.danger, field.danger.as_deref());
    push_notice(&mut notices, ui.warning, field.warning.as_deref());
    push_notice(
        &mut notices,
        ui.pending_deprecated,
        field.pending_deprecated.as_deref(),
    );
    push_notice(&mut notices, ui.deprecated, field.deprecated.as_deref());
    push_notice(
        &mut notices,
        ui.migration,
        field.migration_notice.as_deref(),
    );

    if notices.is_empty() {
        escape_markdown_table(ui.none)
    } else {
        notices
            .into_iter()
            .map(|notice| escape_markdown_table(&notice))
            .collect::<Vec<_>>()
            .join("<br />")
    }
}

fn render_notice_text(field: &DocField, ui: &UiText) -> String {
    let mut notices = Vec::new();
    push_notice(&mut notices, ui.danger, field.danger.as_deref());
    push_notice(&mut notices, ui.warning, field.warning.as_deref());
    push_notice(
        &mut notices,
        ui.pending_deprecated,
        field.pending_deprecated.as_deref(),
    );
    push_notice(&mut notices, ui.deprecated, field.deprecated.as_deref());
    push_notice(
        &mut notices,
        ui.migration,
        field.migration_notice.as_deref(),
    );

    notices
        .into_iter()
        .map(|notice| escape_mdx_text(&notice))
        .collect::<Vec<_>>()
        .join("; ")
}

fn push_notice(notices: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        notices.push(format!("**{label}:** {value}"));
    }
}

fn render_site_index(bundle: &DocBundle, target: &str) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", ui.site_title));
    out.push_str(&format!("- {}: `{target}`\n", ui.target));
    out.push_str(&format!("- {}: `{}`\n", ui.commit, bundle.commit));
    out.push_str(&format!("- {}: `assets/logo.ico`\n\n", ui.asset));

    for language_docs in &bundle.languages {
        out.push_str(&format!("## {}\n\n", language_docs.lang));
        out.push_str(&format!(
            "- [{}]({}/index.md)\n",
            ui.index,
            escape_markdown_link(&language_docs.lang)
        ));
        for app in &language_docs.apps {
            out.push_str(&format!(
                "- [{}]({}/{}.md)\n",
                app.name,
                escape_markdown_link(&language_docs.lang),
                escape_markdown_link(&app.name)
            ));
        }
        out.push('\n');
    }

    out
}

fn render_language_index(commit: &str, language_docs: &LanguageDocs) -> String {
    let ui = ui_text(&language_docs.lang);
    let mut out = String::new();
    out.push_str(&format!("# {} ({})\n\n", ui.site_title, language_docs.lang));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        ui.app_count,
        language_docs.apps.len()
    ));

    for app in &language_docs.apps {
        out.push_str(&format!(
            "- [{}]({}.md): `{}` {}\n",
            app.name,
            escape_markdown_link(&app.name),
            app.fields.len(),
            ui.field_unit
        ));
    }

    out
}

fn render_mkdocs_config(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("site_name: {}\n", quote_yaml_path(ui.site_title)));
    out.push_str(&format!(
        "site_description: {}\n",
        quote_yaml_path(&format!("{} {}", ui.site_description_prefix, bundle.commit))
    ));
    out.push_str("theme:\n");
    out.push_str("  name: readthedocs\n");
    out.push_str("docs_dir: docs\n");
    out.push_str("nav:\n");
    out.push_str(&format!("  - {}: index.md\n", quote_yaml_key(ui.home)));

    for language_docs in &bundle.languages {
        out.push_str(&format!("  - {}:\n", quote_yaml_key(&language_docs.lang)));
        let language_index = format!("{}/index.md", language_docs.lang);
        out.push_str(&format!(
            "      - {}: {}\n",
            quote_yaml_key(ui_text(&language_docs.lang).index),
            quote_yaml_path(&language_index)
        ));
        for app in &language_docs.apps {
            let app_path = format!("{}/{}.md", language_docs.lang, app.name);
            out.push_str(&format!(
                "      - {}: {}\n",
                quote_yaml_key(&app.name),
                quote_yaml_path(&app_path)
            ));
        }
    }

    out
}

fn render_mdbook_config(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str("[book]\n");
    out.push_str(&format!(
        "title = \"{}\"\n",
        escape_toml_string(ui.site_title)
    ));
    out.push_str("authors = [\"Qexed AutoDoc\"]\n");
    out.push_str("language = \"zh-CN\"\n");
    out.push_str("src = \"src\"\n\n");
    out.push_str("[output.html]\n");
    out.push_str(&format!(
        "git-repository-url = \"https://example.invalid/qexed/{}\"\n",
        escape_toml_string(&bundle.commit)
    ));
    out
}

fn render_mdbook_summary(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", ui.summary));
    out.push_str(&format!("- [{}](index.md)\n", ui.home));

    for language_docs in &bundle.languages {
        out.push_str(&format!(
            "- [{}]({}/index.md)\n",
            language_docs.lang,
            escape_markdown_link(&language_docs.lang)
        ));
        for app in &language_docs.apps {
            out.push_str(&format!(
                "  - [{}]({}/{}.md)\n",
                app.name,
                escape_markdown_link(&language_docs.lang),
                escape_markdown_link(&app.name)
            ));
        }
    }

    out
}

fn render_next_index(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: \"{}\"\n", escape_yaml(ui.site_title)));
    out.push_str(&format!("commit: \"{}\"\n", escape_yaml(&bundle.commit)));
    out.push_str("---\n\n");
    out.push_str(&format!("# {}\n\n", ui.site_title));
    out.push_str(&format!("- {}: `{}`\n", ui.commit, bundle.commit));
    out.push_str(&format!("- {}: `/config/assets/logo.ico`\n\n", ui.asset));

    for language_docs in &bundle.languages {
        out.push_str(&format!("## {}\n\n", language_docs.lang));
        for app in &language_docs.apps {
            out.push_str(&format!(
                "- [{}](./{}/{})\n",
                app.name, language_docs.lang, app.name
            ));
        }
        out.push('\n');
    }

    out
}

fn render_next_app_index(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", ui.site_title));
    out.push_str(&format!("- {}: `{}`\n\n", ui.commit, bundle.commit));

    for language_docs in &bundle.languages {
        out.push_str(&format!(
            "- [{}](./{})\n",
            language_docs.lang,
            escape_markdown_link(&language_docs.lang)
        ));
    }

    out
}

fn render_next_app_language_index(commit: &str, language_docs: &LanguageDocs) -> String {
    let ui = ui_text(&language_docs.lang);
    let mut out = String::new();
    out.push_str(&format!("# {} - {}\n\n", ui.site_title, language_docs.lang));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        ui.app_count,
        language_docs.apps.len()
    ));

    for app in &language_docs.apps {
        out.push_str(&format!(
            "- [{}](./{}): `{}` {}\n",
            app.name,
            escape_markdown_link(&app.name),
            app.fields.len(),
            ui.field_unit
        ));
    }

    out
}

fn escape_yaml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn quote_yaml_key(value: &str) -> String {
    format!("\"{}\"", escape_yaml(value))
}

fn quote_yaml_path(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_markdown_link(value: &str) -> String {
    value
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
}

fn escape_markdown(value: &str) -> String {
    escape_mdx_text(value)
}

fn escape_markdown_table(value: &str) -> String {
    escape_mdx_text(value).replace('|', "\\|")
}

fn escape_code_span(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', " ")
}

fn escape_mdx_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\r', "")
        .replace('\n', "<br />")
}
