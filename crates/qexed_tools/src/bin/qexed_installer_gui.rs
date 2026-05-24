#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use qexed_tools::gui::{
    GuiApp, command_error, html_page, i18n_script, language_selector, system_language, tr,
};

#[derive(Debug, Parser)]
#[command(name = "qexed_installer_gui")]
struct Args {}

#[tauri::command]
fn install_package(archive: String, target: String, language: String) -> Result<String, String> {
    qexed_tools::installer::install(&archive, &target).map_err(command_error)?;
    Ok(format!("{} {target}", tr(&language, "installed_to")))
}

#[tauri::command]
fn create_package(source: String, output: String, language: String) -> Result<String, String> {
    qexed_tools::installer::create_package(&source, &output).map_err(command_error)?;
    Ok(format!("{} {output}", tr(&language, "package_created")))
}

fn main() -> Result<()> {
    Args::parse();
    let language = system_language();
    let i18n = i18n_script(
        language,
        &[
            ("title", "Qexed 安装工具"),
            ("heading", "安装工具"),
            ("install_section", "解压安装包"),
            ("archive_path", "安装包路径"),
            ("target_dir", "安装目录"),
            ("install", "安装"),
            ("create_section", "创建安装包"),
            ("source_dir", "源目录"),
            ("output_path", "输出包路径"),
            ("pack", "打包"),
            ("archive_placeholder", "qexed.qxpack"),
            ("target_placeholder", "D:/Qexed"),
            ("source_placeholder", "dist/qexed"),
            ("output_placeholder", "qexed.qxpack"),
        ],
        &[
            ("title", "Qexed Installer"),
            ("heading", "Installer"),
            ("install_section", "Extract package"),
            ("archive_path", "Package path"),
            ("target_dir", "Install directory"),
            ("install", "Install"),
            ("create_section", "Create package"),
            ("source_dir", "Source directory"),
            ("output_path", "Output package path"),
            ("pack", "Pack"),
            ("archive_placeholder", "qexed.qxpack"),
            ("target_placeholder", "D:/Qexed"),
            ("source_placeholder", "dist/qexed"),
            ("output_placeholder", "qexed.qxpack"),
        ],
    );
    let html = html_page(
        language,
        "Qexed Installer",
        &format!(
            r#"{}
<h1 data-i18n="heading"></h1>
<section>
<h2 data-i18n="install_section"></h2>
<label data-i18n="archive_path"></label><input id="archive" data-i18n-placeholder="archive_placeholder">
<label data-i18n="target_dir"></label><input id="target" data-i18n-placeholder="target_placeholder">
<button onclick="installPackage()" data-i18n="install"></button>
</section>
<section>
<h2 data-i18n="create_section"></h2>
<label data-i18n="source_dir"></label><input id="source" data-i18n-placeholder="source_placeholder">
<label data-i18n="output_path"></label><input id="output" data-i18n-placeholder="output_placeholder">
<button onclick="pack()" data-i18n="pack"></button>
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
function installPackage() {{
  call('install_package', {{archive: archive.value, target: target.value, language: guiCurrentLanguage()}});
}}
function pack() {{
  call('create_package', {{source: source.value, output: output.value, language: guiCurrentLanguage()}});
}}
</script>"#,
            language_selector(language)
        ),
    );

    let app = GuiApp {
        title: "Qexed Installer".to_string(),
        html,
    };
    let builder = qexed_tools::gui::builder(app)
        .invoke_handler(tauri::generate_handler![install_package, create_package]);
    qexed_tools::gui::run(builder)
}
