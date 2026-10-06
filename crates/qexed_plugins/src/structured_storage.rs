//! 插件结构化存储（v4 plugins/structured_storage.rs 迁移）。
//!
//! v4 支持 sqlite/mysql/mongodb 三种后端；v6 workspace 无相应依赖
//! （TODO(storage)），默认实现为每插件一目录的文件后端，trait 保留供
//! 外部存储接入。

use std::{
    fs,
    path::PathBuf,
    sync::Arc,
};

use crate::config::StructuredStorageConfig;

#[derive(Debug)]
pub(crate) struct StructuredStorageState {
    backend: Arc<dyn StructuredStorageBackend>,
}

impl Default for StructuredStorageState {
    fn default() -> Self {
        Self {
            backend: Arc::new(FileStructuredStorage::new_default()),
        }
    }
}

impl StructuredStorageState {
    pub(crate) fn configure(&mut self, config: &StructuredStorageConfig) {
        match build_backend(config) {
            Ok(backend) => self.backend = backend,
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.storage.unavailable")
                        .replace("%{error}", &err.to_string())
                );
                self.backend = Arc::new(FileStructuredStorage::new_default());
            }
        }
    }

    pub(crate) fn exists(&self, plugin: &str, key: &str) -> bool {
        self.backend.exists(plugin, key).unwrap_or(false)
    }

    pub(crate) fn get(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        self.backend.get(plugin, key).ok().flatten()
    }

    pub(crate) fn set(&self, plugin: &str, key: &str, value: &[u8]) -> bool {
        self.backend.set(plugin, key, value).is_ok()
    }

    pub(crate) fn delete(&self, plugin: &str, key: &str) -> bool {
        self.backend.delete(plugin, key).is_ok()
    }
}

fn build_backend(
    config: &StructuredStorageConfig,
) -> Result<Arc<dyn StructuredStorageBackend>, crate::error::PluginsError> {
    match config.engine {
        crate::config::StructuredStorageEngine::Sqlite => {
            Ok(Arc::new(FileStructuredStorage::new_default()))
        }
        // TODO(storage): mysql/mongodb 后端依赖 v6 workspace 未引入
        crate::config::StructuredStorageEngine::Mysql
        | crate::config::StructuredStorageEngine::Mongodb => Err(
            crate::error::PluginsError::StructuredStorage(config.engine.as_str().to_string()),
        ),
    }
}

/// 结构化存储接口（TODO(storage)：外部存储后端接入时实现此 trait）。
pub(crate) trait StructuredStorageBackend: Send + Sync + std::fmt::Debug {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool, crate::error::PluginsError>;
    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>, crate::error::PluginsError>;
    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<(), crate::error::PluginsError>;
    fn delete(&self, plugin: &str, key: &str) -> Result<(), crate::error::PluginsError>;
}

/// 文件后端：config/plugins/<plugin>/structured/<key> 一键一文件。
#[derive(Debug)]
struct FileStructuredStorage {
    root: PathBuf,
}

impl FileStructuredStorage {
    fn new_default() -> Self {
        Self {
            root: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("config")
                .join("plugins"),
        }
    }

    fn path_for(&self, plugin: &str, key: &str) -> Result<PathBuf, crate::error::PluginsError> {
        let clean = clean_storage_key(key)
            .ok_or_else(|| crate::error::PluginsError::StructuredStorage(format!("invalid key: {key}")))?;
        Ok(self.root.join(plugin).join("structured").join(clean))
    }
}

impl StructuredStorageBackend for FileStructuredStorage {
    fn exists(&self, plugin: &str, key: &str) -> Result<bool, crate::error::PluginsError> {
        Ok(self.path_for(plugin, key)?.is_file())
    }

    fn get(&self, plugin: &str, key: &str) -> Result<Option<Vec<u8>>, crate::error::PluginsError> {
        match fs::read(self.path_for(plugin, key)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn set(&self, plugin: &str, key: &str, value: &[u8]) -> Result<(), crate::error::PluginsError> {
        let path = self.path_for(plugin, key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, value)?;
        Ok(())
    }

    fn delete(&self, plugin: &str, key: &str) -> Result<(), crate::error::PluginsError> {
        match fs::remove_file(self.path_for(plugin, key)?) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

fn clean_storage_key(key: &str) -> Option<PathBuf> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    let mut clean = PathBuf::new();
    for segment in key.split('/') {
        let segment = segment.trim();
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('\\')
            || segment.contains(':')
        {
            return None;
        }
        let safe = segment
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>();
        if safe.is_empty() {
            return None;
        }
        clean.push(safe);
    }
    clean.set_extension("bin");
    Some(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(backend: &dyn StructuredStorageBackend) {
        let plugin = "structured_storage_test_plugin";
        let key = format!(
            "roundtrip-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        );
        let value = b"\x00typed-json-like-value\xff";
        let updated = b"updated-value";

        assert!(!backend.exists(plugin, &key).expect("exists before set"));
        assert_eq!(backend.get(plugin, &key).expect("get before set"), None);

        backend.set(plugin, &key, value).expect("set value");
        assert!(backend.exists(plugin, &key).expect("exists after set"));
        assert_eq!(
            backend.get(plugin, &key).expect("get after set").as_deref(),
            Some(value.as_slice())
        );

        backend.set(plugin, &key, updated).expect("update value");
        assert_eq!(
            backend
                .get(plugin, &key)
                .expect("get after update")
                .as_deref(),
            Some(updated.as_slice())
        );

        backend.delete(plugin, &key).expect("delete value");
        assert!(!backend.exists(plugin, &key).expect("exists after delete"));
        assert_eq!(backend.get(plugin, &key).expect("get after delete"), None);
    }

    #[test]
    fn file_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "qexed-plugins-ss-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let backend = FileStructuredStorage { root: dir };
        roundtrip(&backend);
    }
}
