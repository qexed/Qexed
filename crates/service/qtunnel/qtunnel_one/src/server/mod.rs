use qexed_config::app::qtunnel_one::One;

use crate::api::Api;
mod qexed;

pub struct Server{
    pub api:crate::api::Api
}
impl Server {
    pub async fn init(config:One)->anyhow::Result<Self>{        
        Ok(Self { api: Api::init(config).await?})
    }
    pub async fn _listen()->anyhow::Result<()>{
        Ok(())
    }
    pub async fn register(&self)->anyhow::Result<()>{
        self.api.register().await?;
        qexed::register_version_command(&self.api.command).await?;
        Ok(())
    }
}