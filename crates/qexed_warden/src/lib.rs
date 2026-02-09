use qexed_config::tool::AppConfigTrait;

pub mod message;
rust_i18n::i18n!("../../locales");
use rust_i18n::t;
#[unsafe(no_mangle)]
pub async fn new()->anyhow::Result<tokio::sync::mpsc::UnboundedSender<message::Message>>{
    let _config = match qexed_config::app::qexed_warden::QexedWarden::load_or_create_default(){
        Ok(v)=>v,
        Err(err)=>{
            log::error!("{}",t!("config_load_error",err=err));
            return Err(err);
        }
    };
    log::info!("{}",t!("config_init_finish"));
    let (s,_r) = tokio::sync::mpsc::unbounded_channel();
    Ok(s)
}
