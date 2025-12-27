use qexed_packet::{PacketCodec, net_types::VarInt};

use crate::data_type::paletted_container::PalettedContainer;
// 请注意:
// 你需要在这个层级进行处理

#[derive(Debug, Default, PartialEq,Clone)]
pub struct ChunkSection {
    // 非空气方块数量
    pub block_count:i16,
    // 方块状态列表(4096)
    pub block_states:PalettedContainer,
    // 生物群系(64)
    pub biomes:PalettedContainer,
}
impl PacketCodec for ChunkSection {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.block_count.serialize(w)?;
        // 方块状态列表
        
        match &self.block_states {
            PalettedContainer::SingleValued(var_int) => {
                0u8.serialize(w)?;
                var_int.serialize(w)?;
            },
            PalettedContainer::Indirect(indirect) => {},
            PalettedContainer::Direct(_) => {},
            PalettedContainer::Unknown => {
                return Err(anyhow::anyhow!("非法方块调色板"));
            },
                    }
        match &self.biomes {
            PalettedContainer::SingleValued(var_int) => {
                0u8.serialize(w)?;
                var_int.serialize(w)?;
            },
            // 暂未实现
            PalettedContainer::Indirect(indirect) => {},
            // 暂未实现
            PalettedContainer::Direct(_) => {},
            PalettedContainer::Unknown => {
                return Err(anyhow::anyhow!("非法群系调色板"));
            },
        }
        return Ok(());
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.block_count.deserialize(r)?;
        let mut bits_per_entry:u8 = 0;
        bits_per_entry.deserialize(r)?;
        match bits_per_entry{
            0=>{
                let mut v = VarInt(0);
                v.deserialize(r)?;
                self.block_states = PalettedContainer::SingleValued(v);
            }
            // 暂未实现
            4|5|6|7|8=>{

            }
            // 暂未实现
            _=>{}

        }
        let mut bits_per_entry:u8 = 0;
        bits_per_entry.deserialize(r)?;
        match bits_per_entry{
            0=>{
                let mut v = VarInt(0);
                v.deserialize(r)?;
                self.biomes = PalettedContainer::SingleValued(v);
            }
            // 暂未实现
            1|2|3=>{

            }
            // 暂未实现
            _=>{}

        }
        Ok(())
    }
}