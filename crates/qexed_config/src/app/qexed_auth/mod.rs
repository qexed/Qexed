use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    pub enabled: bool,
    pub yggdrasil: Yggdrasil,
    pub blocking_pool: BlockingPool,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            enabled: false,
            yggdrasil: Yggdrasil::default(),
            blocking_pool: BlockingPool::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for Auth {
    const PATH: &'static str = "/";
    const NAME: &'static str = "auth";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Yggdrasil {
    pub session_server_url: String,
    pub services_public_keys_url: String,
    pub include_client_ip: bool,
    pub http_timeout: u64,
}

impl Default for Yggdrasil {
    fn default() -> Self {
        Self {
            session_server_url: "https://sessionserver.mojang.com/session/minecraft/hasJoined"
                .to_string(),
            services_public_keys_url: "https://api.minecraftservices.com/publickeys".to_string(),
            include_client_ip: false,
            http_timeout: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingPool {
    pub enabled: bool,
    pub worker_threads: usize,
}

impl Default for BlockingPool {
    fn default() -> Self {
        Self {
            enabled: true,
            worker_threads: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Auth;
    use qexed_config::tool::AppConfigTrait;

    #[tokio::test]
    async fn creates_auth_toml_with_default_yggdrasil_settings() {
        let config_dir =
            std::env::temp_dir().join(format!("qexed-auth-config-test-{}", std::process::id()));
        let _ = qexed_config::CONFIG_PATH.set(config_dir.clone());
        let config_dir = qexed_config::CONFIG_PATH.get().unwrap().clone();

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let join_handle = Auth::load_or_create_default(move |config| async move {
            sender
                .send(config)
                .map_err(|_| anyhow::anyhow!("failed to send auth config"))?;
            Ok(())
        })
        .unwrap();

        join_handle.await.unwrap().unwrap();
        let config = receiver.await.unwrap();
        let file = std::fs::read_to_string(config_dir.join("auth.toml")).unwrap();

        assert!(!config.enabled);
        assert_eq!(
            config.yggdrasil.session_server_url,
            "https://sessionserver.mojang.com/session/minecraft/hasJoined"
        );
        assert_eq!(
            config.yggdrasil.services_public_keys_url,
            "https://api.minecraftservices.com/publickeys"
        );
        assert!(config.blocking_pool.enabled);
        assert_eq!(config.blocking_pool.worker_threads, 2);
        assert!(file.contains("enabled = false"));
        assert!(file.contains("[yggdrasil]"));
        assert!(file.contains(
            "services_public_keys_url = \"https://api.minecraftservices.com/publickeys\""
        ));
        assert!(file.contains("[blocking_pool]"));
        assert!(file.contains("worker_threads = 2"));
    }
}
