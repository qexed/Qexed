use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};
use qexed_packet::error::PacketError;

pub const METHOD_ADD: i8 = 0;
pub const METHOD_REMOVE: i8 = 1;
pub const METHOD_CHANGE: i8 = 2;

pub const RENDER_TYPE_INTEGER: i32 = 0;
pub const RENDER_TYPE_HEARTS: i32 = 1;

#[derive(Debug, PartialEq, Clone)]
pub struct SetObjective {
    pub objective_name: String,
    pub method: i8,
    pub display_name: crate::types::TextComponent,
    pub render_type: VarInt,
    pub number_format: Option<crate::types::NumberFormat>,
}

impl SetObjective {
    pub fn create(
        objective_name: impl Into<String>,
        display_name: crate::types::TextComponent,
    ) -> Self {
        Self {
            objective_name: objective_name.into(),
            method: METHOD_ADD,
            display_name,
            render_type: VarInt(RENDER_TYPE_INTEGER),
            number_format: None,
        }
    }

    pub fn update(
        objective_name: impl Into<String>,
        display_name: crate::types::TextComponent,
    ) -> Self {
        Self {
            objective_name: objective_name.into(),
            method: METHOD_CHANGE,
            display_name,
            render_type: VarInt(RENDER_TYPE_INTEGER),
            number_format: None,
        }
    }

    pub fn remove(objective_name: impl Into<String>) -> Self {
        Self {
            objective_name: objective_name.into(),
            method: METHOD_REMOVE,
            display_name: crate::types::TextComponent::default(),
            render_type: VarInt(RENDER_TYPE_INTEGER),
            number_format: None,
        }
    }

    fn has_objective_definition(&self) -> bool {
        self.method == METHOD_ADD || self.method == METHOD_CHANGE
    }
}

impl Default for SetObjective {
    fn default() -> Self {
        Self {
            objective_name: String::new(),
            method: METHOD_REMOVE,
            display_name: crate::types::TextComponent::default(),
            render_type: VarInt(RENDER_TYPE_INTEGER),
            number_format: None,
        }
    }
}

impl Packet for SetObjective {
    const ID: i32 = 0x6A;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.objective_name.serialize(w)?;
        self.method.serialize(w)?;
        if self.has_objective_definition() {
            self.display_name.serialize(w)?;
            self.render_type.serialize(w)?;
            self.number_format.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        self.objective_name.deserialize(r)?;
        self.method.deserialize(r)?;
        if self.has_objective_definition() {
            self.display_name.deserialize(r)?;
            self.render_type.deserialize(r)?;
            self.number_format.deserialize(r)?;
        } else {
            self.display_name = crate::types::TextComponent::default();
            self.render_type = VarInt(RENDER_TYPE_INTEGER);
            self.number_format = None;
        }
        Ok(())
    }
}
