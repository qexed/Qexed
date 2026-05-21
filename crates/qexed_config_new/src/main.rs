use qexed_config_new::tool::AutoDocConfigTrait;
use qexed_config_new::tool::AppConfigTrait;
use serde::{Deserialize, Serialize};
use qexed_config_macros::AutoDoc;
rust_i18n::i18n!("locales");
fn main()->anyhow::Result<()>{
    let a = PluginDownload::load_or_create_default(None, None,None)?;
    PluginDownload::doc_fields("zh-CN");

    Ok(())
}
#[qexed_config_macros::app_config("/","test")]
#[derive(Debug, Serialize, Deserialize,AutoDoc)]

pub struct PluginDownload {
    // 是否启用插件下载功能
    #[AutoDoc(
        key = "config.plugin_download.enable",
        pending_deprecated = "config.plugin_download.pending_deprecation.warning",
        migration_notice = "config.plugin_download.migration_notice.enable"
    )]
    pub enable: bool,
    // 插件下载地址(用于插件配置)
    #[AutoDoc(
        key = "config.plugin_download.download",
        warning = "config.plugin_download.warning.download"
    )]
    pub download2: String,
    // 插件下载认证token
    #[AutoDoc(
        key = "config.plugin_download.download_token",
        deprecation = "config.plugin_download.deprecation.download_token",
        migration_notice = "config.plugin_download.migration_notice.download_token",
        sub
    )]
    #[serde(rename="Name")]
    pub download_token: Test,
}

#[derive(Debug,Default, Serialize, Deserialize,AutoDoc)]
pub struct Test {
    // 是否启用插件下载功能
    #[AutoDoc(
        key = "测试",

    )]
    pub hello:i64
}
impl Default for PluginDownload {
    fn default() -> Self {
        Self {
            enable: false,
            download2: "https://api.example.com/plugins/".to_owned(),
            download_token: Test::default(),
        }
    }
}
