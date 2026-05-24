#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use qexed_tools::gui::{
    GuiApp, command_error, escape_html, html_page, i18n_script, language_for_qexed_config,
    language_selector, tr,
};

#[derive(Debug, Parser)]
#[command(name = "qexed_favicon_gui")]
struct Args {
    #[arg(long, default_value = "config/qexed.toml")]
    config: String,
}

#[tauri::command]
fn convert_favicon(input: String) -> Result<String, String> {
    qexed_tools::favicon::convert_to_data_uri(input).map_err(command_error)
}

#[tauri::command]
fn apply_favicon(input: String, config: String, language: String) -> Result<String, String> {
    let data_uri = qexed_tools::favicon::convert_to_data_uri(input).map_err(command_error)?;
    qexed_tools::favicon::update_config(&config, &data_uri).map_err(command_error)?;
    Ok(tr(&language, "config_written").to_string())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let language = language_for_qexed_config(&args.config);
    let config = escape_html(&args.config);
    let i18n = i18n_script(
        language,
        &[
            ("title", "Qexed 图标"),
            ("heading", "Favicon 转换"),
            ("png_path", "PNG 文件路径"),
            ("config_path", "配置文件路径"),
            ("apply", "写入配置"),
            ("convert", "只生成 data URI"),
            ("png_placeholder", "icon.png"),
        ],
        &[
            ("title", "Qexed Favicon"),
            ("heading", "Favicon Converter"),
            ("png_path", "PNG file path"),
            ("config_path", "Config file path"),
            ("apply", "Apply to config"),
            ("convert", "Generate data URI only"),
            ("png_placeholder", "icon.png"),
        ],
    );
    let html = html_page(
        language,
        "Qexed Favicon",
        &format!(
            r#"{}
<h1 data-i18n="heading"></h1>
<section>
<label data-i18n="png_path"></label>
<input id="input" data-i18n-placeholder="png_placeholder">
<label data-i18n="config_path"></label>
<input id="config" value="{config}">
<div class="row"><button onclick="apply()" data-i18n="apply"></button><button class="secondary" onclick="convert()" data-i18n="convert"></button></div>
<pre id="out"></pre>
</section>
{i18n}
<script>
const invoke = window.__TAURI__.core.invoke;
async function call(command, data) {{
  try {{
    out.textContent = await invoke(command, data);
  }} catch (error) {{
    out.textContent = String(error);
  }}
}}
function apply() {{
  call('apply_favicon', {{input: input.value, config: config.value, language: guiCurrentLanguage()}});
}}
function convert() {{
  call('convert_favicon', {{input: input.value}});
}}
</script>"#,
            language_selector(language)
        ),
    );

    let app = GuiApp {
        title: "Qexed Favicon".to_string(),
        html,
    };
    let builder = qexed_tools::gui::builder(app)
        .invoke_handler(tauri::generate_handler![convert_favicon, apply_favicon]);
    qexed_tools::gui::run(builder)
}
