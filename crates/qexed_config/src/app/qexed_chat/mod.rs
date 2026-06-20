use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub enabled: bool,
    pub system_chat_only: bool,
    pub io_pool: IoPool,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            enabled: true,
            system_chat_only: false,
            io_pool: IoPool::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for Chat {
    const PATH: &'static str = "/";
    const NAME: &'static str = "chat";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoPool {
    pub enabled: bool,
    pub worker_threads: usize,
}

impl Default for IoPool {
    fn default() -> Self {
        Self {
            enabled: false,
            worker_threads: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Chat;
    use qexed_config::tool::AppConfigTrait;

    #[tokio::test]
    async fn creates_chat_toml_with_default_settings() {
        let config_dir =
            std::env::temp_dir().join(format!("qexed-chat-config-test-{}", std::process::id()));
        let _ = qexed_config::CONFIG_PATH.set(config_dir.clone());
        let config_dir = qexed_config::CONFIG_PATH.get().unwrap().clone();

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let join_handle = Chat::load_or_create_default(move |config| async move {
            sender
                .send(config)
                .map_err(|_| anyhow::anyhow!("failed to send chat config"))?;
            Ok(())
        })
        .unwrap();

        join_handle.await.unwrap().unwrap();
        let config = receiver.await.unwrap();
        let file = std::fs::read_to_string(config_dir.join("chat.toml")).unwrap();

        assert!(config.enabled);
        assert!(!config.system_chat_only);
        assert!(!config.io_pool.enabled);
        assert_eq!(config.io_pool.worker_threads, 2);
        assert!(file.contains("enabled = true"));
        assert!(file.contains("system_chat_only = false"));
        assert!(file.contains("[io_pool]"));
    }
}
