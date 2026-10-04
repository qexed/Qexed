//! 语言文件拉取与装载：三级服务器回退、本地缓存、全局表装配。

use crate::config::LanguageConfig;
use crate::error::LanguageError;
use crate::{storage_dir, Translations, CURRENT, ZH_CN_JSON};
use std::path::{Path, PathBuf};

/// 本体语言文件的缓存路径：`<config>/languages/qexed/{commit}/{language}.json`。
pub fn qexed_cache_path(commit: &str, language: &str) -> Result<PathBuf, LanguageError> {
    Ok(storage_dir()?.join("qexed").join(commit).join(format!("{language}.json")))
}

/// 插件语言文件的缓存路径：`<config>/languages/{author}/{plugin}/{commit}/{language}.json`。
pub fn plugin_cache_path(
    author: &str,
    plugin: &str,
    commit: &str,
    language: &str,
) -> Result<PathBuf, LanguageError> {
    Ok(storage_dir()?
        .join(author)
        .join(plugin)
        .join(commit)
        .join(format!("{language}.json")))
}

/// 从 url 拉取翻译 json 文本；非 200 或网络错误一律 Err。
async fn fetch(url: &str) -> Result<String, LanguageError> {
    let resp = reqwest::get(url).await?;
    if !resp.status().is_success() {
        return Err(LanguageError::Remote(format!("HTTP {}", resp.status())));
    }
    Ok(resp.text().await?)
}

/// 三级服务器回退拉取插件翻译：
/// plugin_server_url[plugin] → plugin_author_server_url[author] → server_url。
/// 每级按服务器路由拼 `/api/v1/{author}/{plugin}/{commit}/{language}.json`。
pub async fn fetch_plugin(
    config: &LanguageConfig,
    author: &str,
    plugin: &str,
    commit: &str,
    language: &str,
) -> Result<String, LanguageError> {
    let route = format!("/api/v1/{author}/{plugin}/{commit}/{language}.json");
    let mut errors: Vec<String> = Vec::new();
    if let Some(lang_map) = config.plugin_server_url.get(plugin) {
        let Some(base) = lang_map.get(language) else {
            return Err(LanguageError::AllSourcesFailed(format!(
                "plugin server url for {plugin}/{language} not configured"
            )));
        };
        let url = format!("{}{}", base.trim_end_matches('/'), route);
        match fetch(&url).await {
            Ok(text) => return Ok(text),
            Err(e) => errors.push(format!("plugin server: {e}")),
        }
    }
    if let Some(base) = config.plugin_author_server_url.get(author) {
        let url = format!("{}{}", base.trim_end_matches('/'), route);
        match fetch(&url).await {
            Ok(text) => return Ok(text),
            Err(e) => errors.push(format!("author server: {e}")),
        }
    }
    if let Some(base) = config.server_url.as_deref() {
        let url = format!("{}{}", base.trim_end_matches('/'), route);
        match fetch(&url).await {
            Ok(text) => return Ok(text),
            Err(e) => errors.push(format!("default server: {e}")),
        }
    }
    Err(LanguageError::AllSourcesFailed(errors.join("; ")))
}

/// 从服务器拉取本体翻译：`{server_url}/api/v1/qexed/{commit}/{language}.json`。
pub async fn fetch_qexed(
    config: &LanguageConfig,
    commit: &str,
    language: &str,
) -> Result<String, LanguageError> {
    let base = config
        .server_url
        .as_deref()
        .ok_or_else(|| LanguageError::AllSourcesFailed("server_url not configured".into()))?;
    let full = format!(
        "{}{}",
        base.trim_end_matches('/'),
        format!("/api/v1/qexed/{commit}/{language}.json")
    );
    fetch(&full).await
}

/// 缓存写入。
async fn write_cache(path: &Path, body: &str) -> Result<(), LanguageError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, body).await?;
    Ok(())
}

/// 确保一份翻译就绪：缓存命中 → 读缓存；损坏 → 删除重拉；缺失 → 拉取并写缓存。
/// `fetch_me` 为该文件的回退拉取闭包（本体/插件各自不同）。
pub async fn ensure_translations(
    cache: PathBuf,
    fetch_me: impl std::future::Future<Output = Result<String, LanguageError>>,
) -> Result<Translations, LanguageError> {
    if let Ok(text) = tokio::fs::read_to_string(&cache).await {
        match Translations::from_json(&text) {
            Ok(table) => return Ok(table),
            Err(_) => {
                // 缓存损坏：删除重拉
                let _ = tokio::fs::remove_file(&cache).await;
            }
        }
    }
    let body = fetch_me.await?;
    write_cache(&cache, &body).await?;
    Translations::from_json(&body).map_err(LanguageError::from)
}

/// 按语言加载本体翻译表（不装全局）：缓存 → 远程 → 内嵌 zh-CN 兜底。
/// 供离线工具（如 doc_to_mdx）按语言生成产物。
pub async fn load_translations(
    config: &LanguageConfig,
    commit: &str,
    language: &str,
) -> Translations {
    let cache = match qexed_cache_path(commit, language) {
        Ok(p) => p,
        Err(_) => return Translations::from_json(ZH_CN_JSON).unwrap_or_default(),
    };
    match ensure_translations(cache, fetch_qexed(config, commit, language)).await {
        Ok(table) => table,
        Err(e) => {
            eprintln!("[qexed_language] {language}: fallback to embedded zh-CN: {e}");
            Translations::from_json(ZH_CN_JSON).unwrap_or_default()
        }
    }
}

/// init 全流程：enable 关闭 → 内嵌兜底；否则 缓存 → 远程 → 内嵌 三级装本体表。
pub async fn run(
    config: &LanguageConfig,
    commit: &str,
    language: &str,
) -> Result<(), LanguageError> {
    if !config.enable {
        let table = Translations::from_json(ZH_CN_JSON).unwrap_or_default();
        *CURRENT.write().unwrap() = Some(table);
        return Ok(());
    }
    let cache = qexed_cache_path(commit, language)?;
    let loaded = ensure_translations(cache, fetch_qexed(config, commit, language)).await;
    match loaded {
        Ok(table) => {
            *CURRENT.write().unwrap() = Some(table);
        }
        Err(e) => {
            // 全源失败：内嵌 zh-CN 兜底，进程照常启动（翻译缺英文等仍回退 key）。
            log::error!("fallback to embedded zh-CN: {e}");
            let table = Translations::from_json(ZH_CN_JSON).unwrap_or_default();
            *CURRENT.write().unwrap() = Some(table);
        }
    }
    
    Ok(())
}