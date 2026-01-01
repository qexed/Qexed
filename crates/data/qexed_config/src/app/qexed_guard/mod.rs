use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
pub mod data;
pub mod qexed_tcp_connect_app;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GuardConfig {
    pub version: i32,
    pub data:data::Data,
    pub tcp_connect_app: super::qexed_tcp_connect_app::TcpConnect,
}
impl AppConfigTrait for GuardConfig {
    const PATH: &'static str = "./config/qexed_guard/";
    const NAME: &'static str = "config";
}
