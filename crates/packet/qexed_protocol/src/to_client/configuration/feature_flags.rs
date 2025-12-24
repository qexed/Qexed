use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x0c)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct FeatureFlags {
    pub features:Vec<String>,
}