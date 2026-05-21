use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x02)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Success {
    pub uuid:uuid::Uuid,
    pub username:String,
    pub properties:Vec<Properties>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug,Default,PartialEq,Clone)]
pub struct Properties{
    pub name:String,
    pub value:String,
    pub signature:Option<String>,
}
