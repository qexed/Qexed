use rust_i18n::t;
rust_i18n::i18n!("../../../locales");
pub async fn update_plugins_check(config:&qexed_config::app::qexed::plugin_download::PluginDownload)->anyhow::Result<bool>{
    if !config.enable{
        log::info!("{}",t!("qexed_plugin_manage.download_skip", url = "https://doc.qexed.com/docs/xxxx"));
        return Ok(true);
    }
    // TODO: 需后续实现

    Ok(true)
}