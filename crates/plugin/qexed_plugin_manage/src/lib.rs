use std::{ fs, io, path::PathBuf};

use dashmap::DashMap;
use rayon::iter::{IntoParallelIterator,  ParallelIterator};
use rust_i18n::t;
rust_i18n::i18n!("../../../locales");
pub async fn new() -> anyhow::Result<PluginManage> {
    let mut plugin_manage = PluginManage::new();
    log::info!("{}", t!("qexed_plugin_manage.load_plugin_config_start"));
    plugin_manage.plugin = load_plugin_list()?
        .into_par_iter()
        .filter_map(|path| {
            let binding = path.clone();
            let file_path = binding.to_str().unwrap_or("插件未知");
            let v = match qexed_wasm_runtime::load_plugin_config(path) {
                Ok(v) => {
                    log::info!(
                        "{}",
                        t!(
                            "qexed_plugin_manage.loading_plugin_config_info",
                            name = v.name,
                            version = v.version
                        )
                    );
                    Some(v)
                }
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
    log::info!("{}", t!("qexed_plugin_manage.load_plugin_config_finish"));

    find_duplicate_plugins(&mut plugin_manage.plugin).into_par_iter().for_each(|(name,plugins)|{
        log::error!("{}", t!("qexed_plugin_manage.duplicate_plugins_warning", name = name));
        log::error!("{}", t!("qexed_plugin_manage.check_plugins_instruction"));
        plugins.iter().for_each(|plugin|{
            if let Some(plugin_path) = &plugin.path{
                log::error!("{}", t!("qexed_plugin_manage.plugin_version_path", 
                    version = plugin.version, 
                    path = plugin_path.to_str().unwrap_or(&t!("qexed_plugin_manage.unknown_path"))
                ))
            } else {
                log::error!("{}", t!("qexed_plugin_manage.plugin_version_path", 
                    version = plugin.version,   
                    path = t!("qexed_plugin_manage.unknown_path")
                ))
            }
        });
    });
    log::debug!("插件信息:{:?}",plugin_manage.plugin);
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
fn find_duplicate_plugins(plugins: &mut Vec<qexed_wasm_runtime::config::Plugin>) -> Vec<(String, Vec<qexed_wasm_runtime::config::Plugin>)> {
    // 使用HashMap按插件名称分组
    let plugin_groups: DashMap<String, Vec<qexed_wasm_runtime::config::Plugin>> = DashMap::new();
    
    // 将插件按名称分组[7](@ref)
    for plugin in std::mem::take(plugins) {
        plugin_groups.entry(plugin.name.clone())
            .or_insert_with(Vec::new)
            .push(plugin);
    }
    
    // 收集重复的插件[7](@ref)
    let mut duplicates = Vec::new();
    let mut unique_plugins = Vec::new();
    
    for (name, plugin_list) in plugin_groups {
        if plugin_list.len() > 1 {
            // 出现多次的插件加入结果[1](@ref)
            duplicates.push((name, plugin_list));
        } else {
            // 唯一的插件保留在原向量中
            unique_plugins.extend(plugin_list);
        }
    }
    
    // 恢复原向量中的唯一插件
    *plugins = unique_plugins;
    
    duplicates
}