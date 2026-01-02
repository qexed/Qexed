use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
pub mod qexed_tcp_connect_app;
pub mod qexed_tcp_connect_app_start;
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GuardConfig {
    pub version: i32,
    pub tcp_connect_app_start: qexed_tcp_connect_app_start::TcpConnectStart,
    pub tcp_connect_app: super::qexed_tcp_connect_app::TcpConnect,

}
impl AppConfigTrait for GuardConfig {
    const PATH: &'static str = "./config/qexed_guard/";
    const NAME: &'static str = "config";
}
