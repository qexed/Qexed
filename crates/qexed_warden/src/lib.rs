use qexed_config::tool::AppConfigTrait;

pub mod message;
rust_i18n::i18n!("../../locales");
use rust_i18n::t;
pub async fn new()->anyhow::Result<tokio::sync::mpsc::UnboundedSender<message::Message>>{
    let _config = match qexed_config::app::qexed_warden::QexedWarden::load_or_create_default(){
        Ok(v)=>v,
        Err(err)=>{
            log::error!("{}",t!("qexed_warden.config_load_error",err=err));
            return Err(err);
        }
    };
    log::info!("{}",t!("qexed_warden.config_init_finish"));
    let (r,s) = tokio::sync::mpsc::unbounded_channel();
    Ok(r)
}
pub async fn new2()->anyhow::Result<tokio::sync::mpsc::UnboundedSender<()>>{
    let (r,s) = tokio::sync::mpsc::unbounded_channel();
    Ok(r)
}