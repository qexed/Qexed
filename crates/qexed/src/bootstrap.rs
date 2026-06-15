use qexed_config::tool::AppConfigTrait;


pub async fn load(
    args: &qexed_config::app::qexed::args::ServerArgs,
) -> anyhow::Result<bool> {
    if let Some(config_path) = args.config_path.clone() {
        qexed_config::CONFIG_PATH
            .set(config_path)
            .map_err(|_| anyhow::anyhow!("CONFIG_PATH is already initialized"))?;
    }

    let _qexed = qexed_config::app::qexed::Qexed::load_or_create_default(None)?;
    if args.init_settings {
        return Ok(false);
    }
    Ok(true)
}

