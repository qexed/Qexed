pub const PROTOCOL_VERSION: i32 = 772;  // 数据包协议版本
pub const MC_VERSION: &'static str = "1.21.8";  // Minecraft游戏版本
pub const QEXED_VERSION: &'static str = "0.1.0a";  // Qexed服务器版本
const fn make_qexed_name() -> &'static str {
    "Qexed 0.1.0a"
}
pub const QEXED_NAME: &'static str = make_qexed_name();
pub const QTUNNEL_VERSION: &'static str = "0.1.0a";  // QTunnel代理端版本
const fn make_qtunnel_name() -> &'static str {
    "QTunnel 0.1.0a"
}
pub const QTUNNEL_NAME: &'static str = make_qtunnel_name();
pub mod app;
pub mod tool;
pub mod public;