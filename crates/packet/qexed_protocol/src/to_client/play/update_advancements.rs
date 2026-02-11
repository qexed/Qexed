use qexed_packet::{PacketCodec, net_types::VarInt};

use crate::types::Slot;
#[qexed_packet_macros::packet(id = 0x7B)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct UpdateAdvancements {
    pub reset_or_clear:bool,
    pub advancement_mapping:Vec<AdvancementMapping>,
    pub identifiers:Vec<String>,
    pub progress_mapping:Vec<ProgressMapping>,
    pub show_advancements:bool

}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct AdvancementMapping{
    pub key:String,
    pub value:Advancement,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Advancement{
    pub parentid:Option<String>,
    pub display_data:Option<AdvancementDisplay>,
    pub nested_requirements:Vec<Vec<String>>,
    pub sends_telemetry_data:bool,
}

#[derive(Debug, Default, PartialEq,Clone)]
pub struct AdvancementDisplay{
    pub title:qexed_nbt::Tag,
    pub description:qexed_nbt::Tag,
    pub icon:Slot,
    pub frame_type:VarInt,
    pub flags:i32,
    pub background_texture:Option<String>,
    pub x_coord:f32,
    pub y_coord:f32
}
impl AdvancementDisplay {
    /// 检查是否包含背景纹理
    pub fn has_background_texture(&self) -> bool {
        (self.flags & 0x01) != 0
    }
    
    /// 检查是否显示成就完成时的toast通知
    pub fn show_toast(&self) -> bool {
        (self.flags & 0x02) != 0
    }
    
    /// 检查是否为隐藏成就
    pub fn hidden(&self) -> bool {
        (self.flags & 0x04) != 0
    }
    
    /// 设置背景纹理
    pub fn set_background_texture(&mut self, texture: Option<String>) {
        
        if texture.is_some() {
            self.background_texture = texture;
            self.flags |= 0x01;  // 设置 has background texture 标志
        } else {
            self.flags &= !0x01;  // 清除 has background texture 标志
        }
    }
    
    /// 设置是否显示toast通知
    pub fn set_show_toast(&mut self, show: bool) {
        if show {
            self.flags |= 0x02;
        } else {
            self.flags &= !0x02;
        }
    }
    
    /// 设置是否为隐藏成就
    pub fn set_hidden(&mut self, hidden: bool) {
        if hidden {
            self.flags |= 0x04;
        } else {
            self.flags &= !0x04;
        }
    }
}

impl PacketCodec for AdvancementDisplay {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        // 序列化基础字段
        self.title.serialize(w)?;
        self.description.serialize(w)?;
        self.icon.serialize(w)?;
        self.frame_type.serialize(w)?;
        self.flags.serialize(w)?;
        
        // 根据 flags 的第0位决定是否序列化背景纹理
        if self.has_background_texture() {
            if let Some(ref texture) = self.background_texture {
                texture.serialize(w)?;
            } else {
                // 标志位设置了但没有背景纹理，这是一个错误状态
                return Err(anyhow::anyhow!(
                    "Inconsistent state: flags indicates has background texture, but background_texture is None"
                ));
            }
        }
        
        // 序列化坐标
        self.x_coord.serialize(w)?;
        self.y_coord.serialize(w)?;
        
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        // 反序列化基础字段
        self.title.deserialize(r)?;
        self.description.deserialize(r)?;
        self.icon.deserialize(r)?;
        self.frame_type.deserialize(r)?;
        self.flags.deserialize(r)?;
        
        // 根据 flags 的第0位决定是否反序列化背景纹理
        if self.has_background_texture() {
            let mut texture = String::new();
            texture.deserialize(r)?;
            self.background_texture = Some(texture);
        } else {
            self.background_texture = None;
        }
        
        // 反序列化坐标
        self.x_coord.deserialize(r)?;
        self.y_coord.deserialize(r)?;
        
        Ok(())
    }
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct ProgressMapping{
    pub key:String,
    pub value:AdvancementProgress,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct AdvancementProgress{
    pub criteria:Vec<Criteria>
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Criteria{
    pub criterion_identifier:String,
    pub date_of_achieving:Option<i64>,
}