//! 无 `global-root` 特征时的行为：
//! - `Config::ROOT` 覆写与 `*_at` 方法照常工作
//! - 未覆写 `ROOT` 的实现调用无根方法返回 `NotInitialized`

use serde::{Deserialize, Serialize};
use qexed_config::{Config, error::ConfigError};

const TMP: &str = "no_global_root_tmp";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct WithRoot { name: String }
impl Config for WithRoot {
    const ROOT: Option<&'static str> = Some("no_global_root_tmp");
    const PATH: &'static str = "sub";
    const NAME: &'static str = "config";
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct NoRoot { name: String }
impl Config for NoRoot {
    const PATH: &'static str = "sub";
    const NAME: &'static str = "config";
}

#[test]
fn root_override_works_without_feature() {
    let cfg = WithRoot { name: "x".into() };
    WithRoot::save_file(&cfg).unwrap();
    assert!(std::path::Path::new(TMP).join("sub").join("config.toml").exists());
    assert_eq!(WithRoot::load_file(false).unwrap().name, "x");
    std::fs::remove_dir_all(TMP).unwrap();
}

#[test]
fn at_apis_work_without_feature() {
    let base = std::env::temp_dir().join("qexed_cfg_nofeature_at");
    std::fs::create_dir_all(&base).unwrap();
    let cfg = NoRoot { name: "y".into() };
    NoRoot::save_file_at(&base, &cfg).unwrap();
    assert_eq!(NoRoot::load_file_at(&base, false).unwrap().name, "y");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn missing_root_reports_not_initialized() {
    // 未覆写 ROOT 且无 global-root 特征 → 明确报错而非 panic
    let err = NoRoot::root().unwrap_err();
    assert!(matches!(err, ConfigError::NotInitialized));
    assert!(matches!(NoRoot::load_file(false).unwrap_err(), ConfigError::NotInitialized));
}
