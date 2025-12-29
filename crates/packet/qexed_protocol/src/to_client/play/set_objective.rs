use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetObjective {
    pub name: String,
    pub action: i8,
    pub display_text: qexed_nbt::Tag,
    pub r#type: VarInt,
    pub number_format: Option<NumberFormat>,
}
impl qexed_packet::Packet for SetObjective {
    const ID: u32 = 0x63;

    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.name.serialize(w)?;
        self.action.serialize(w)?;
        if self.action == 0 || self.action == 2 {
            self.display_text.serialize(w)?;
            self.r#type.serialize(w)?;
            self.number_format.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.name.deserialize(r)?;
        self.action.deserialize(r)?;
        if self.action == 0 || self.action == 2 {
            self.display_text.deserialize(r)?;
            self.r#type.deserialize(r)?;
            self.number_format.deserialize(r)?;
        }
        Ok(())
    }
}
#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum NumberFormat {
    Blank,
    Styled(qexed_nbt::Tag),
    Fuxed(qexed_nbt::Tag),
    Unknown,
}
