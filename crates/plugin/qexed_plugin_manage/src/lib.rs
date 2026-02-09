use std::{collections::HashMap, fs, io, path::PathBuf};
pub mod dependency_analyzer;
use dashmap::DashMap;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rust_i18n::t;
rust_i18n::i18n!("../../../locales");
pub async fn new() -> anyhow::Result<PluginManage> {
    let mut plugin_manage = PluginManage::new();
    log::info!("{}", t!("qexed_plugin_manage.load_plugin_config_start"));
    plugin_manage.plugin = load_plugin_list()?
        .into_par_iter()
        .filter_map(|path| {
            let binding = path.clone();
            let file_path = binding.to_str().unwrap_or("Unknown");
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

    find_duplicate_plugins(&mut plugin_manage.plugin)
        .into_par_iter()
        .for_each(|(name, plugins)| {
            log::error!(
                "{}",
                t!("qexed_plugin_manage.duplicate_plugins_warning", name = name)
            );
            log::error!("{}", t!("qexed_plugin_manage.check_plugins_instruction"));
            plugins.iter().for_each(|plugin| {
                if let Some(plugin_path) = &plugin.path {
                    log::error!(
                        "{}",
                        t!(
                            "qexed_plugin_manage.plugin_version_path",
                            version = plugin.version,
                            path = plugin_path
                                .to_str()
                                .unwrap_or(&t!("qexed_plugin_manage.unknown_path"))
                        )
                    )
                } else {
                    log::error!(
                        "{}",
                        t!(
                            "qexed_plugin_manage.plugin_version_path",
                            version = plugin.version,
                            path = t!("qexed_plugin_manage.unknown_path")
                        )
                    )
                }
            });
        });
    depend_check_plugins(&mut plugin_manage.plugin);
    log::error!("{}", t!("qexed_plugin_manage.plugin_circular_dependency"));
    let cleaning_result = dependency_analyzer::filter_cyclic_plugins(&mut plugin_manage.plugin);
    if !cleaning_result.removed_plugins.is_empty() {
        log::error!("{}", t!("qexed_plugin_manage.removed_plugins"));
        for plugin in &cleaning_result.removed_plugins {
            log::error!("{}", format!("  - {}", plugin));
        }
        log::error!(
            "{}",
            t!("qexed_plugin_manage.detected_circular_dependency_path")
        );
        for (i, cycle) in cleaning_result.cycles_detected.iter().enumerate() {
            log::error!("{}", format!("  {}. {}", i + 1, cycle.join(" → ")));
        }
    }
    // 整理插件启动顺序
    let _plugin_run = dependency_analyzer::calculate_plugin_load_order(&plugin_manage.plugin);
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
fn find_duplicate_plugins(
    plugins: &mut Vec<qexed_wasm_runtime::config::Plugin>,
) -> Vec<(String, Vec<qexed_wasm_runtime::config::Plugin>)> {
    // 使用HashMap按插件名称分组
    let plugin_groups: DashMap<String, Vec<qexed_wasm_runtime::config::Plugin>> = DashMap::new();

    // 将插件按名称分组[7](@ref)
    for plugin in std::mem::take(plugins) {
        plugin_groups
            .entry(plugin.name.clone())
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
fn depend_check_plugins(plugins: &mut Vec<qexed_wasm_runtime::config::Plugin>) {
    // 提前构建版本映射，避免重复解析
    let plugin_version: HashMap<String, semver::Version> = plugins
        .par_iter()
        .filter_map(|plugin| {
            semver::Version::parse(&plugin.version)
                .map(|v| (plugin.name.clone(), v))
                .map_err(|err| {
                    log::error!(
                        "{}",
                        t!(
                            "qexed_plugin_manage.plugin_version_decode_err",
                            name = plugin.name,
                            url = "https://doc.qexed.com/docs/xxxx",
                            err = err,
                        )
                    );
                })
                .ok()
        })
        .collect();

    // 使用 retain 替代所有权转移，更高效
    plugins.retain(|plugin| check_plugin_dependencies(plugin, &plugin_version));
}

/// 检查单个插件的依赖关系
fn check_plugin_dependencies(
    plugin: &qexed_wasm_runtime::config::Plugin,
    plugin_version: &HashMap<String, semver::Version>,
) -> bool {
    // 检查依赖冲突：depend 和 softdepend 不能有重叠
    let depend_conflicts: Vec<_> = plugin
        .depend
        .keys()
        .filter(|depend| plugin.softdepend.contains_key(*depend))
        .collect();

    if !depend_conflicts.is_empty() {
        log::error!(
            "{}",
            t!(
                "",
                name = plugin.name,
                conflicts = plugin_string_join(&depend_conflicts)
            )
        );
        return false;
    }

    // 检查硬依赖
    if !check_dependency_set(&plugin.depend, plugin, plugin_version, true) {
        return false;
    }

    // 检查软依赖（允许缺失）
    check_dependency_set(&plugin.softdepend, plugin, plugin_version, false)
}
fn plugin_string_join(plugins: &Vec<&String>) -> String {
    // 通过 .map(|s| s.as_str()) 将每个 &String 转换为 &str
    plugins
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<&str>>()
        .join(",")
}
/// 检查依赖集合（硬依赖或软依赖）
fn check_dependency_set(
    dependencies: &HashMap<String, qexed_wasm_runtime::config::PluginApi>,
    plugin: &qexed_wasm_runtime::config::Plugin,
    plugin_version: &HashMap<String, semver::Version>,
    is_required: bool, // true for hard depend, false for soft depend
) -> bool {
    for (depend, depend_data) in dependencies {
        // 检查自依赖
        if *depend == plugin.name {
            log::error!("插件[{name}]的依赖项插件包括了插件本身", name = plugin.name);
            return false;
        }

        // 检查依赖是否存在
        let Some(version) = plugin_version.get(depend) else {
            if is_required {
                log::error!(
                    "插件[{name}]的依赖项插件缺失: {plugin_depend_name}[{plugin_depend_version}]",
                    name = plugin.name,
                    plugin_depend_name = depend,
                    plugin_depend_version = depend_data.version,
                );
                return false;
            }
            continue; // 软依赖允许缺失
        };

        // 版本要求解析
        let req = match semver::VersionReq::parse(&depend_data.version) {
            Ok(v) => v,
            Err(err) => {
                log::error!(
                    "插件[{name}]的依赖项插件[{plugin_depend_name}]版本声明错误:{err}",
                    name = plugin.name,
                    plugin_depend_name = depend,
                    err = err,
                );
                return false;
            }
        };

        // 版本兼容性检查
        if !req.matches(version) {
            log::error!(
                "插件[{name}]的依赖项插件[{plugin_depend_name}]版本不兼容: 当前v{current_version}, 需要{required_version}",
                name = plugin.name,
                plugin_depend_name = depend,
                current_version = version,
                required_version = &depend_data.version
            );
            return false;
        }
    }
    true
}
