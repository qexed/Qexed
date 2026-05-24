#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::process::Command;

use anyhow::Result;
use clap::Parser;
use qexed_tools::gui::{
    GuiApp, command_error, escape_html, html_page, i18n_script, language_selector, system_language,
    tr,
};

#[derive(Debug, Parser)]
#[command(name = "qexed_config_to_mdx_gui")]
struct Args {
    #[arg(long, default_value = "qexed_config_to_mdx")]
    generator: String,
}

#[tauri::command]
fn generate_config_docs(
    generator: String,
    out: String,
    langs: String,
    formats: String,
    commit: String,
    language: String,
) -> Result<String, String> {
    run_generator(generator, out, langs, formats, commit, language).map_err(command_error)
}

fn run_generator(
    generator: String,
    out: String,
    langs: String,
    formats: String,
    commit: String,
    language: String,
) -> Result<String> {
    let mut cmd = Command::new(generator);
    cmd.arg("--out").arg(out);
    for lang in langs
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        cmd.arg("--lang").arg(lang);
    }
    for format in formats
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        cmd.arg("--format").arg(format);
    }
    if !commit.trim().is_empty() {
        cmd.arg("--commit").arg(commit.trim());
    }

    let output = cmd.output()?;
    Ok(format!(
        "{}: {}\n\n{}:\n{}\n\n{}:\n{}",
        tr(&language, "status"),
        output.status,
        tr(&language, "stdout"),
        String::from_utf8_lossy(&output.stdout),
        tr(&language, "stderr"),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let language = system_language();
    let generator = escape_html(&args.generator);
    let i18n = i18n_script(
        language,
        &[
            ("title", "Qexed 文档生成"),
            ("heading", "配置文档生成"),
            ("generator", "qexed_config_to_mdx 可执行文件"),
            ("out_dir", "输出目录"),
            ("doc_langs", "文档语言，逗号分隔"),
            ("formats", "格式，逗号分隔"),
            ("commit", "Commit 覆盖，可留空"),
            ("generate", "生成"),
        ],
        &[
            ("title", "Qexed Documentation Generator"),
            ("heading", "Config Documentation Generator"),
            ("generator", "qexed_config_to_mdx executable"),
            ("out_dir", "Output directory"),
            ("doc_langs", "Documentation languages, comma-separated"),
            ("formats", "Formats, comma-separated"),
            ("commit", "Commit override, optional"),
            ("generate", "Generate"),
        ],
    );
    let html = html_page(
        language,
        "Qexed Documentation Generator",
        &format!(
            r#"{}
<h1 data-i18n="heading"></h1>
<section>
<label data-i18n="generator"></label><input id="generator" value="{generator}">
<label data-i18n="out_dir"></label><input id="out" value="qexed-config-docs">
<label data-i18n="doc_langs"></label><input id="langs" value="zh-CN,en">
<label data-i18n="formats"></label><input id="formats" value="all">
<label data-i18n="commit"></label><input id="commit">
<button onclick="run()" data-i18n="generate"></button>
<pre id="outbox"></pre>
</section>
{i18n}
<script>
const invoke = window.__TAURI__.core.invoke;
async function run() {{
  try {{
    outbox.textContent = await invoke('generate_config_docs', {{
      generator: generator.value,
      out: out.value,
      langs: langs.value,
      formats: formats.value,
      commit: commit.value,
      language: guiCurrentLanguage()
    }});
  }} catch (error) {{
    outbox.textContent = String(error);
  }}
}}
</script>"#,
            language_selector(language)
        ),
    );

    let app = GuiApp {
        title: "Qexed Documentation Generator".to_string(),
        html,
    };
    let builder = qexed_tools::gui::builder(app)
        .invoke_handler(tauri::generate_handler![generate_config_docs]);
    qexed_tools::gui::run(builder)
}
