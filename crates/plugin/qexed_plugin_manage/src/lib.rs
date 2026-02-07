use std::{fs, io, path::PathBuf};

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rust_i18n::t;
rust_i18n::i18n!("../../../locales");
pub async fn new() -> anyhow::Result<PluginManage> {
    let mut plugin_manage = PluginManage::new();
    log::info!("{}",t!("qexed_plugin_manage.load_plugin_config_start"));
    plugin_manage.plugin = load_plugin_list()?
        .into_par_iter()
        .filter_map(|path| {
            let binding = path.clone();
            let file_path = binding.to_str().unwrap_or("插件未知");
            let v = match qexed_wasm_runtime::load_plugin_config(path) {
                Ok(v) => {
                    log::info!("{}",t!("qexed_plugin_manage.loading_plugin_config_info",name=v.name,version=v.version));
                    Some(v)
                },
                Err(err) => {
                    log::error!(
                        "{}",
                        t!(
                            "qexed_plugin_manage.loading_plugin_config_err",
                            path = file_path,
                            err = err
                        )
                    );
                    None
                }
            };
            v
        })
        .collect();
    log::info!("{}",t!("qexed_plugin_manage.load_plugin_config_finish"));
    log::info!("插件信息:{:?}",plugin_manage.plugin);
    Ok(plugin_manage)
}
pub struct PluginManage {
    pub plugin: Vec<qexed_wasm_runtime::config::Plugin>,
}
impl PluginManage {
    pub fn new() -> Self {
        Self {
            plugin: Default::default(),
        }
    }

    pub async fn update_plugins_check(
        &self,
        config: &qexed_config::app::qexed::plugin_download::PluginDownload,
    ) -> anyhow::Result<bool> {
        if !config.enable {
            log::info!(
                "{}",
                t!(
                    "qexed_plugin_manage.download_skip",
                    url = "https://doc.qexed.com/docs/xxxx"
                )
            );
            return Ok(true);
        }
        // TODO: 需后续实现

        Ok(true)
    }
}
fn load_plugin_list() -> io::Result<Vec<PathBuf>> {
    // 创建插件目录（如果不存在）
    fs::create_dir_all("plugin")?;

    let wasm_files: Vec<PathBuf> = fs::read_dir("plugin")?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "wasm") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    Ok(wasm_files)
}
