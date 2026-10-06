use qexed_packet::{net_types::VarInt, PacketCodec, PacketReader, PacketWriter};
use crate::types::{NumberFormat, TextComponent};

/// method 字段取值（对应 Java ClientboundSetObjectivePacket 的 METHOD_* 常量）。
pub const METHOD_ADD: i8 = 0;
pub const METHOD_REMOVE: i8 = 1;
pub const METHOD_CHANGE: i8 = 2;

/// ObjectiveCriteria$RenderType 的 idMapper 序号（writeEnum VarInt）。
pub const RENDER_TYPE_INTEGER: i32 = 0;
pub const RENDER_TYPE_HEARTS: i32 = 1;

/// 26.3 ClientboundSetObjective（minecraft:set_objective，id 0x6c）。
/// 依据 server-26.3.jar ClientboundSetObjectivePacket.write：
/// objective_name(Utf) + method(Byte)；仅当 method ∈ {0=add, 2=change} 时写
/// display_name(TRUSTED_STREAM_CODEC) + render_type(writeEnum VarInt) +
/// number_format(NumberFormatTypes.OPTIONAL_STREAM_CODEC：bool 存在标记 + 分发)。
/// method=1(remove) 时后三个字段完全不写——扁平宏模板无法表达条件字段，故手工实现 codec。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetObjective {
    pub objective_name: String,
    pub method: i8,
    pub display_name: TextComponent,
    /// ObjectiveCriteria$RenderType → VarInt（0=integer 1=hearts）。仅 add/change 时写入。
    pub render_type: VarInt,
    pub number_format: Option<NumberFormat>,
}

impl qexed_packet::Packet for SetObjective {
    const ID: i32 = 0x6c;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.objective_name.serialize(w)?;
        self.method.serialize(w)?;
        if self.method == METHOD_ADD || self.method == METHOD_CHANGE {
            self.display_name.serialize(w)?;
            self.render_type.serialize(w)?;
            self.number_format.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.objective_name.deserialize(r)?;
        self.method.deserialize(r)?;
        if self.method == METHOD_ADD || self.method == METHOD_CHANGE {
            self.display_name.deserialize(r)?;
            self.render_type.deserialize(r)?;
            self.number_format.deserialize(r)?;
        } else {
            // Java 在 remove 分支把字段填充为 EMPTY/INTEGER/empty()，仅语义占位，不读线上数据。
            self.display_name = TextComponent::default();
            self.render_type = VarInt(RENDER_TYPE_INTEGER);
            self.number_format = None;
        }
        Ok(())
    }
}
