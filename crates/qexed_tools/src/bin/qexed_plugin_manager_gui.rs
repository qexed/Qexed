#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use qexed_tools::gui::{
    GuiApp, command_error, escape_html, html_page, i18n_script, language_selector, system_language,
    tr,
};
use qexed_tools::plugins::PluginEntry;

#[derive(Debug, Parser)]
#[command(name = "qexed_plugin_manager_gui")]
struct Args {
    #[arg(long, default_value = "plugins")]
    dir: String,
}

#[tauri::command]
fn plugin_list(dir: String) -> Result<Vec<PluginEntry>, String> {
    qexed_tools::plugins::list(dir).map_err(command_error)
}

#[tauri::command]
fn plugin_install(dir: String, file: String, language: String) -> Result<String, String> {
    let target = qexed_tools::plugins::install(dir, file).map_err(command_error)?;
    Ok(format!(
        "{} {}",
        tr(&language, "plugin_installed"),
        target.display()
    ))
}

#[tauri::command]
fn plugin_enable(dir: String, name: String, language: String) -> Result<String, String> {
    let target = qexed_tools::plugins::enable(dir, &name).map_err(command_error)?;
    Ok(format!(
        "{} {}",
        tr(&language, "plugin_enabled"),
        target.display()
    ))
}

#[tauri::command]
fn plugin_disable(dir: String, name: String, language: String) -> Result<String, String> {
    let target = qexed_tools::plugins::disable(dir, &name).map_err(command_error)?;
    Ok(format!(
        "{} {}",
        tr(&language, "plugin_disabled"),
        target.display()
    ))
}

#[tauri::command]
fn plugin_remove(dir: String, name: String, language: String) -> Result<String, String> {
    qexed_tools::plugins::remove(dir, &name).map_err(command_error)?;
    Ok(tr(&language, "plugin_removed").to_string())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let language = system_language();
    let dir = escape_html(&args.dir);
    let i18n = i18n_script(
        language,
        &[
            ("title", "Qexed 插件管理"),
            ("heading", "插件管理"),
            ("plugin_dir", "插件目录"),
            ("install_file", "安装文件路径"),
            ("refresh", "刷新"),
            ("install_wasm", "安装 wasm"),
            ("state", "状态"),
            ("name", "名称"),
            ("size", "大小"),
            ("actions", "操作"),
            ("enabled", "启用"),
            ("disabled", "禁用"),
            ("enable", "启用"),
            ("disable", "禁用"),
            ("remove", "删除"),
            (
                "file_placeholder",
                "target/wasm32-unknown-unknown/release/example.wasm",
            ),
        ],
        &[
            ("title", "Qexed Plugin Manager"),
            ("heading", "Plugin Manager"),
            ("plugin_dir", "Plugin directory"),
            ("install_file", "Install file path"),
            ("refresh", "Refresh"),
            ("install_wasm", "Install wasm"),
            ("state", "State"),
            ("name", "Name"),
            ("size", "Size"),
            ("actions", "Actions"),
            ("enabled", "Enabled"),
            ("disabled", "Disabled"),
            ("enable", "Enable"),
            ("disable", "Disable"),
            ("remove", "Remove"),
            (
                "file_placeholder",
                "target/wasm32-unknown-unknown/release/example.wasm",
            ),
        ],
    );
    let html = html_page(
        language,
        "Qexed Plugin Manager",
        &format!(
            r#"{}
<h1 data-i18n="heading"></h1>
<section>
<label data-i18n="plugin_dir"></label><input id="dir" value="{dir}">
<label data-i18n="install_file"></label><input id="file" data-i18n-placeholder="file_placeholder">
<div class="row"><button onclick="refresh()" data-i18n="refresh"></button><button onclick="installPlugin()" data-i18n="install_wasm"></button></div>
<table><thead><tr><th data-i18n="state"></th><th data-i18n="name"></th><th data-i18n="size"></th><th data-i18n="actions"></th></tr></thead><tbody id="rows"></tbody></table>
<pre id="out"></pre>
</section>
{i18n}
<script>
const invoke = window.__TAURI__.core.invoke;
let currentPlugins = [];
function showError(error) {{
  out.textContent = String(error);
}}
async function call(command, data={{}}) {{
  try {{
    return await invoke(command, data);
  }} catch (error) {{
    showError(error);
    return null;
  }}
}}
function renderPlugins() {{
  rows.innerHTML = '';
  for (const plugin of currentPlugins) {{
    const tr = document.createElement('tr');
    const state = document.createElement('td');
    state.textContent = plugin.enabled ? guiT('enabled') : guiT('disabled');
    const name = document.createElement('td');
    name.textContent = plugin.name;
    const size = document.createElement('td');
    size.textContent = plugin.size;
    const actions = document.createElement('td');
    const toggle = document.createElement('button');
    toggle.textContent = plugin.enabled ? guiT('disable') : guiT('enable');
    toggle.onclick = () => togglePlugin(plugin.name, plugin.enabled);
    const remove = document.createElement('button');
    remove.className = 'secondary';
    remove.textContent = guiT('remove');
    remove.onclick = () => removePlugin(plugin.name);
    actions.append(toggle, ' ', remove);
    tr.append(state, name, size, actions);
    rows.appendChild(tr);
  }}
}}
async function refresh() {{
  const list = await call('plugin_list', {{dir: dir.value}});
  if (!list) return;
  currentPlugins = list;
  renderPlugins();
}}
async function installPlugin() {{
  const message = await call('plugin_install', {{dir: dir.value, file: file.value, language: guiCurrentLanguage()}});
  if (message) out.textContent = message;
  await refresh();
}}
async function togglePlugin(name, enabled) {{
  const command = enabled ? 'plugin_disable' : 'plugin_enable';
  const message = await call(command, {{dir: dir.value, name, language: guiCurrentLanguage()}});
  if (message) out.textContent = message;
  await refresh();
}}
async function removePlugin(name) {{
  const message = await call('plugin_remove', {{dir: dir.value, name, language: guiCurrentLanguage()}});
  if (message) out.textContent = message;
  await refresh();
}}
document.addEventListener('gui-language-change', renderPlugins);
refresh();
</script>"#,
            language_selector(language)
        ),
    );

    let app = GuiApp {
        title: "Qexed Plugin Manager".to_string(),
        html,
    };
    let builder = qexed_tools::gui::builder(app).invoke_handler(tauri::generate_handler![
        plugin_list,
        plugin_install,
        plugin_enable,
        plugin_disable,
        plugin_remove
    ]);
    qexed_tools::gui::run(builder)
}
