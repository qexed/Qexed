pub mod config;
pub mod error;
mod authenticator;
mod model;
mod offline;
mod util;

use std::sync::OnceLock;

pub use authenticator::Authenticator;
pub use model::AuthenticatedProfile;
pub use offline::offline_profile;
use qexed_config::Config;

use crate::{config::AuthConfig, error::AuthError};


static AUTH_CONFIG: OnceLock<AuthConfig> = OnceLock::new();

/// 全局初始化。程序启动时调用一次。
pub fn init()->Result<(),AuthError> {
    AUTH_CONFIG.set(AuthConfig::load_and_create_default(true)?).ok();
    Ok(())
}

/// 取全局配置；未初始化直接 panic（属于编程错误）。
pub fn get() -> &'static AuthConfig {
    AUTH_CONFIG.get().expect("auth config not initialized")
}