use crate::types::{ComponentsToAdd, Slot};
use bytes::{Buf as _, BufMut as _};
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

/// `ServerboundSetCreativeModeSlotPacket` (play, to_server, id 0x39)。
///
/// 客户端发来的物品栏是不可信输入：数据组件在线上带 VarInt 长度前缀
/// （component type id + payload 长度 + payload），与共享的
/// `crate::types::Slot`（无长度前缀）不同，因此这里手写 `Packet` 实现。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCreativeModeSlot {
    pub slot_num: i16,
    pub item_stack: Slot,
}

impl Packet for SetCreativeModeSlot {
    const ID: i32 = 0x39;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.slot_num.serialize(w)?;
        serialize_untrusted_slot(&self.item_stack, w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.slot_num.deserialize(r)?;
        self.item_stack = deserialize_untrusted_slot(r)?;
        Ok(())
    }
}

/// Max components counted in a creative slot payload; guards against absurd counts.
const MAX_COMPONENTS: i32 = 4096;

fn serialize_untrusted_slot(slot: &Slot, w: &mut PacketWriter) -> qexed_packet::Result<()> {
    slot.item_count.serialize(w)?;
    if slot.item_count.0 <= 0 {
        return Ok(());
    }

    slot.item_id
        .as_ref()
        .ok_or_else(|| qexed_packet::PacketError::msg("item_id is required when item_count > 0"))?
        .serialize(w)?;

    let components_to_add = slot.components_to_add.as_deref().unwrap_or(&[]);
    VarInt(components_to_add.len() as i32).serialize(w)?;
    let components_to_remove_count = slot
        .number_of_components_to_remove
        .clone()
        .unwrap_or_else(|| {
            VarInt(
                slot.components_to_remove
                    .as_ref()
                    .map_or(0, |items| items.len() as i32),
            )
        });
    components_to_remove_count.serialize(w)?;

    for component in components_to_add {
        serialize_length_prefixed_component(component, w)?;
    }

    if let Some(components_to_remove) = &slot.components_to_remove {
        for component in components_to_remove {
            component.serialize(w)?;
        }
    }

    Ok(())
}

fn deserialize_untrusted_slot(r: &mut PacketReader) -> qexed_packet::Result<Slot> {
    let mut item_count = VarInt::default();
    item_count.deserialize(r)?;
    if item_count.0 <= 0 {
        return Ok(Slot {
            item_count: VarInt(0),
            ..Slot::default()
        });
    }

    let mut item_id = VarInt::default();
    item_id.deserialize(r)?;

    let add_count = read_count("creative slot added components", r)?;
    let remove_count = read_count("creative slot removed components", r)?;

    let mut components_to_add = Vec::new();
    for _ in 0..add_count {
        let mut component_type = VarInt::default();
        component_type.deserialize(r)?;
        if let Some(component) = deserialize_length_prefixed_component(component_type.0, r)? {
            components_to_add.push(component);
        }
    }

    let mut components_to_remove = Vec::with_capacity(remove_count);
    for _ in 0..remove_count {
        let mut component = VarInt::default();
        component.deserialize(r)?;
        components_to_remove.push(component);
    }

    Ok(Slot {
        item_count,
        item_id: Some(item_id),
        number_of_components_to_add: Some(VarInt(components_to_add.len() as i32)),
        number_of_components_to_remove: Some(VarInt(components_to_remove.len() as i32)),
        components_to_add: (!components_to_add.is_empty()).then_some(components_to_add),
        components_to_remove: (!components_to_remove.is_empty()).then_some(components_to_remove),
    })
}

/// 每个组件线上为：type id (VarInt) + payload 长度 (VarInt) + payload。
fn serialize_length_prefixed_component(
    component: &ComponentsToAdd,
    w: &mut PacketWriter,
) -> qexed_packet::Result<()> {
    let mut framed = bytes::BytesMut::new();
    {
        let mut framed_writer = PacketWriter::new(&mut framed);
        component.serialize(&mut framed_writer)?;
    }

    let mut framed_bytes = framed.freeze();
    let mut reader = PacketReader::new(&mut framed_bytes);
    let mut component_type = VarInt::default();
    component_type.deserialize(&mut reader)?;
    component_type.serialize(w)?;

    let payload_len = reader.buf.remaining();
    let payload = reader.buf.copy_to_bytes(payload_len);
    VarInt(payload_len as i32).serialize(w)?;
    w.buf.extend_from_slice(&payload);
    Ok(())
}

/// 读取一个长度前缀组件；未知组件类型返回 `None`（跳过但保持流位置正确）。
fn deserialize_length_prefixed_component(
    component_type: i32,
    r: &mut PacketReader,
) -> qexed_packet::Result<Option<ComponentsToAdd>> {
    let mut payload_len = VarInt::default();
    payload_len.deserialize(r)?;
    if payload_len.0 < 0 {
        return Err(qexed_packet::PacketError::msg(format!(
            "negative component payload length: {}",
            payload_len.0
        )));
    }
    let payload_len = payload_len.0 as usize;
    if payload_len > r.buf.remaining() {
        return Err(qexed_packet::PacketError::msg(format!(
            "component payload length {payload_len} exceeds remaining {}",
            r.buf.remaining()
        )));
    }

    let mut framed = bytes::BytesMut::new();
    {
        let mut framed_writer = PacketWriter::new(&mut framed);
        VarInt(component_type).serialize(&mut framed_writer)?;
    }
    let payload = r.buf.copy_to_bytes(payload_len);
    framed.extend_from_slice(&payload);

    let mut framed_bytes = framed.freeze();
    let mut reader = PacketReader::new(&mut framed_bytes);
    let mut component = ComponentsToAdd::default();
    component.deserialize(&mut reader)?;
    if matches!(component, ComponentsToAdd::Unknown) {
        return Ok(None);
    }
    Ok(Some(component))
}

fn read_count(name: &str, r: &mut PacketReader) -> qexed_packet::Result<usize> {
    let mut len = VarInt::default();
    len.deserialize(r)?;
    if len.0 < 0 || len.0 > MAX_COMPONENTS {
        return Err(qexed_packet::PacketError::msg(format!(
            "invalid {name} count: {}",
            len.0
        )));
    }
    Ok(len.0 as usize)
}

#[cfg(test)]
mod tests {
    use qexed_packet::{Packet, PacketReader, PacketWriter};

    use super::SetCreativeModeSlot;
    use crate::types::Slot;
    use qexed_packet::net_types::VarInt;

    #[test]
    fn empty_slot_round_trips() {
        let packet = SetCreativeModeSlot {
            slot_num: -1,
            item_stack: Slot {
                item_count: VarInt(0),
                ..Slot::default()
            },
        };

        let mut buf = bytes::BytesMut::new();
        let mut writer = PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = PacketReader::new(&mut bytes);
        let mut decoded = SetCreativeModeSlot::default();
        decoded.deserialize(&mut reader).unwrap();
        assert_eq!(decoded, packet);
        assert_eq!(SetCreativeModeSlot::ID, 0x39);
    }
}
