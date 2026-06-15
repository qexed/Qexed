use crate::types::Slot;
use anyhow::Context as _;
use bytes::{Buf as _, BufMut as _};
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCreativeModeSlot {
    pub slot_num: i16,
    pub item_stack: Slot,
}

impl Packet for SetCreativeModeSlot {
    const ID: i32 = 0x38;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.slot_num.serialize(w)?;
        serialize_untrusted_slot(&self.item_stack, w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.slot_num.deserialize(r)?;
        self.item_stack = deserialize_untrusted_slot(r)?;
        Ok(())
    }
}

fn serialize_untrusted_slot(slot: &Slot, w: &mut PacketWriter) -> anyhow::Result<()> {
    slot.item_count.serialize(w)?;
    if slot.item_count.0 <= 0 {
        return Ok(());
    }

    slot.item_id
        .as_ref()
        .context("item_id is required when item_count > 0")?
        .serialize(w)?;

    let components_to_add = slot.components_to_add.as_deref().unwrap_or(&[]);
    VarInt(components_to_add.len() as i32).serialize(w)?;
    let components_to_remove_count =
        slot.number_of_components_to_remove
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

fn deserialize_untrusted_slot(r: &mut PacketReader) -> anyhow::Result<Slot> {
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

fn serialize_length_prefixed_component(
    component: &crate::types::ComponentsToAdd,
    w: &mut PacketWriter,
) -> anyhow::Result<()> {
    let mut framed = bytes::BytesMut::new();
    {
        let mut framed_writer = PacketWriter::new(&mut framed);
        component.serialize(&mut framed_writer)?;
    }

    let mut framed_reader = framed.freeze();
    let mut reader = PacketReader::new(&mut framed_reader);
    let mut component_type = VarInt::default();
    component_type.deserialize(&mut reader)?;
    component_type.serialize(w)?;

    let payload_len = reader.buf.remaining();
    VarInt(payload_len as i32).serialize(w)?;
    w.buf.put(reader.buf.copy_to_bytes(payload_len));
    Ok(())
}

fn deserialize_length_prefixed_component(
    component_type: i32,
    r: &mut PacketReader,
) -> anyhow::Result<Option<crate::types::ComponentsToAdd>> {
    let len = read_len("creative slot component payload", r)?;
    let payload = r.buf.copy_to_bytes(len);

    let mut framed = bytes::BytesMut::new();
    {
        let mut framed_writer = PacketWriter::new(&mut framed);
        VarInt(component_type).serialize(&mut framed_writer)?;
    }
    framed.put(payload);

    let mut framed_reader = framed.freeze();
    let mut reader = PacketReader::new(&mut framed_reader);
    let mut component = crate::types::ComponentsToAdd::default();
    if component.deserialize(&mut reader).is_err() {
        return Ok(None);
    }
    if reader.buf.has_remaining() {
        return Ok(None);
    }
    Ok(Some(component))
}

fn read_count(name: &str, r: &mut PacketReader) -> anyhow::Result<usize> {
    let mut count = VarInt::default();
    count.deserialize(r)?;
    if count.0 < 0 {
        anyhow::bail!("negative {name}: {}", count.0);
    }
    if count.0 > 65_536 {
        anyhow::bail!("{name} {} exceeds max 65536", count.0);
    }
    Ok(count.0 as usize)
}

fn read_len(name: &str, r: &mut PacketReader) -> anyhow::Result<usize> {
    let mut len = VarInt::default();
    len.deserialize(r)?;
    if len.0 < 0 {
        anyhow::bail!("negative {name} length: {}", len.0);
    }
    let len = len.0 as usize;
    if len > r.buf.remaining() {
        anyhow::bail!(
            "{name} length {len} exceeds remaining {}",
            r.buf.remaining()
        );
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use qexed_packet::{Packet, PacketCodec, net_types::VarInt};

    use super::SetCreativeModeSlot;

    #[test]
    fn creative_slot_uses_length_prefixed_component_payloads() {
        let packet = SetCreativeModeSlot {
            slot_num: 36,
            item_stack: crate::types::Slot {
                item_count: VarInt(1),
                item_id: Some(VarInt(1029)),
                number_of_components_to_add: Some(VarInt(1)),
                number_of_components_to_remove: Some(VarInt(0)),
                components_to_add: Some(vec![crate::types::ComponentsToAdd::MinecraftItemName(
                    crate::types::minecraft::ItemName {
                        name: qexed_nbt::Tag::String(Arc::from("Menu")),
                    },
                )]),
                components_to_remove: None,
            },
        };

        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = SetCreativeModeSlot::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }

    #[test]
    fn creative_slot_skips_unreadable_delimited_component_payloads() {
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        36_i16.serialize(&mut writer).unwrap();
        VarInt(1).serialize(&mut writer).unwrap();
        VarInt(1029).serialize(&mut writer).unwrap();
        VarInt(1).serialize(&mut writer).unwrap();
        VarInt(0).serialize(&mut writer).unwrap();
        VarInt(60).serialize(&mut writer).unwrap();
        VarInt(1).serialize(&mut writer).unwrap();
        buf.extend_from_slice(&[0x12]);

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = SetCreativeModeSlot::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded.slot_num, 36);
        assert_eq!(decoded.item_stack.item_count, VarInt(1));
        assert_eq!(decoded.item_stack.item_id, Some(VarInt(1029)));
        assert_eq!(
            decoded.item_stack.number_of_components_to_add,
            Some(VarInt(0))
        );
        assert!(decoded.item_stack.components_to_add.is_none());
    }
}
