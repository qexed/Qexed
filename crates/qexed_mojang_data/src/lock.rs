use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::error::{IoCtx, MojangDataError};

const LOCK_WAIT_ATTEMPTS: usize = 600;
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct DownloadLock {
    path: PathBuf,
}

impl DownloadLock {
    pub(super) fn acquire(path: &Path) -> Result<Self, MojangDataError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).io_ctx(format!(
                "无法创建 Mojang 下载锁目录: {}",
                parent.display()
            ))?;
        }

        for _ in 0..LOCK_WAIT_ATTEMPTS {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(_) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(LOCK_RETRY_INTERVAL);
                }
                Err(err) => {
                    return Err(err).io_ctx(format!("无法创建 Mojang 下载锁: {}", path.display()));
                }
            }
        }

        Err(MojangDataError::LockTimeout(path.to_path_buf()))
    }
}

impl Drop for DownloadLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
