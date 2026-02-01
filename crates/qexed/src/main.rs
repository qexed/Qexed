use qexed_config::tool::AppConfigTrait;

#[tokio::main]
async fn main()->anyhow::Result<()>{
    qexed_log::log_init().await;
    let config = qexed_config::app::qexed::Qexed::load_or_create_default()?;
    tokio::spawn(qexed_update_check::check_version(config.update_check.clone()));

    loop {
        
    }
}