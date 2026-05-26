use anyhow::Result;
use qexed_config::app::qexed::server::{ResourcePack, ResourcePackSource};

mod local;
mod model;
mod object_storage;
mod url;

use local::LocalResourcePack;
pub use model::ResourcePackOffer;
use model::ResourcePackState;
use object_storage::object_storage_download_url;
use url::local_download_url;

#[derive(Clone)]
pub struct ResourcePackManager {
    state: ResourcePackState,
}

impl ResourcePackManager {
    pub async fn from_config(config: &ResourcePack) -> Result<Self> {
        if !config.enable {
            return Ok(Self {
                state: ResourcePackState::Disabled,
            });
        }

        match config.source {
            ResourcePackSource::Url => Ok(Self {
                state: ResourcePackState::Url,
            }),
            ResourcePackSource::ObjectStorage => Ok(Self {
                state: ResourcePackState::ObjectStorage,
            }),
            ResourcePackSource::Local => Ok(Self {
                state: ResourcePackState::Local(LocalResourcePack::from_config(config).await?),
            }),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let ResourcePackState::Local(local) = &mut self.state else {
            return Ok(());
        };
        local.start().await
    }

    pub fn offer(&self, config: &ResourcePack, login_host: &str) -> Option<ResourcePackOffer> {
        match &self.state {
            ResourcePackState::Disabled => None,
            ResourcePackState::Url => {
                let url = config.url.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(ResourcePackOffer {
                        url: url.to_string(),
                        hash: config.hash.trim().to_string(),
                    })
                }
            }
            ResourcePackState::ObjectStorage => {
                object_storage_download_url(config).map(|url| ResourcePackOffer {
                    url,
                    hash: config.hash.trim().to_string(),
                })
            }
            ResourcePackState::Local(local) => Some(ResourcePackOffer {
                url: local_download_url(config, login_host, local),
                hash: local.hash.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests;
