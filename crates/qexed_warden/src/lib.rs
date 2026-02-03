use qexed_config::tool::AppConfigTrait;
use rust_i18n::t;
rust_i18n::i18n!("../../locales");
pub async fn new()->anyhow::Result<()>{
    let _config = match qexed_config::app::qexed_warden::QexedWarden::load_or_create_default(){
        Ok(v)=>v,
        Err(err)=>{
            log::error!("{}",t!("qexed_warden.config_load_error",err=err));
            return Err(err);
        }
    };
    log::info!("{}",t!("qexed_warden.config_init_finish"));
    Ok(())
}