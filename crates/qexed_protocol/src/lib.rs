pub mod error;
mod nullpacket;
pub mod protocol_26_3;
pub mod raw_packet;
pub mod to_client;
pub mod to_server;
pub mod types;
pub use nullpacket::NullPacket;
pub use raw_packet::RawPacket;

/// 让文档生成器链入本 crate，从而收集全部 `#[packet]` 的 inventory。
pub fn link_packet_docs() -> usize {
    qexed_packet::collect_packet_docs().len()
}
