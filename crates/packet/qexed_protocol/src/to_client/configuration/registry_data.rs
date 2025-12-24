use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x07)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RegistryData {
    pub id:String,
    pub entries:Vec<Entries>,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Entries {
    pub entry_id: String,
    pub data: Option<qexed_nbt::Tag>,
}
