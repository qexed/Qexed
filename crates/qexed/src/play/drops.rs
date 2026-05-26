#[cfg(test)]
use qexed_packet::net_types::Position as BlockPosition;
use qexed_protocol::to_client::play::add_entity::EntityPosition;
#[cfg(test)]
use qexed_protocol::types::Slot;

#[cfg(test)]
pub(super) fn block_drop_item(
    block_state: i32,
    position: &BlockPosition,
) -> Option<(EntityPosition, Slot)> {
    if crate::inventory::is_air_block_state(block_state) {
        return None;
    }

    crate::inventory::picked_item_for_block_state(block_state).map(|item_id| {
        let drop_position = super::mining::drop_position(position);
        (drop_position, crate::inventory::simple_item(item_id, 1))
    })
}

#[cfg(test)]
pub(super) fn block_drop_preview_packets(
    entity_id: i32,
    block_state: i32,
    position: &BlockPosition,
) -> anyhow::Result<Option<Vec<bytes::Bytes>>> {
    let Some((drop_position, item)) = block_drop_item(block_state, position) else {
        return Ok(None);
    };
    let entity = crate::entities::DroppedItemEntity {
        entity_id,
        uuid: uuid::Uuid::new_v4(),
        dimension: "minecraft:overworld".to_string(),
        position: drop_position,
        item,
        pickup_ready_at: std::time::Instant::now(),
    };
    Ok(Some(entity.spawn_packets(
        crate::entities::entity_type_id("minecraft:item")?,
    )?))
}

pub(super) fn collect_dropped_items(
    entities: &crate::entities::EntityManager,
    dimension: &str,
    position: EntityPosition,
    inventory: &mut crate::inventory::PlayerInventory,
) -> anyhow::Result<
    Option<(
        Vec<crate::entities::DroppedItemEntity>,
        Vec<crate::inventory::InventorySlotChange>,
    )>,
> {
    let collected = entities.collect_reachable_items(dimension, position)?;
    let mut picked = Vec::new();
    let mut changes = Vec::new();

    for item in collected {
        if let Some(mut item_changes) = inventory.add_item_stack(&item.item) {
            picked.push(item);
            changes.append(&mut item_changes);
        } else {
            entities.restore_dropped_item(item);
        }
    }

    if picked.is_empty() {
        Ok(None)
    } else {
        Ok(Some((picked, changes)))
    }
}

#[cfg(test)]
mod tests {
    use super::{block_drop_item, block_drop_preview_packets};
    use qexed_packet::Packet;

    #[test]
    fn block_drop_packets_spawn_item_entity_for_known_block() {
        let packets = block_drop_preview_packets(
            42,
            1,
            &qexed_packet::net_types::Position {
                x: -5,
                y: 64,
                z: 10,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(packets.len(), 2);
        assert_eq!(
            packets[0][0],
            qexed_protocol::to_client::play::add_entity::AddEntity::ID as u8
        );
        assert_eq!(
            packets[1][0],
            qexed_protocol::to_client::play::set_entity_data::SetEntityData::ID as u8
        );
    }

    #[test]
    fn block_drop_packets_skip_blocks_without_item_mapping() {
        assert!(
            block_drop_preview_packets(
                42,
                crate::inventory::air_block_state(),
                &Default::default()
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn block_drop_item_uses_block_item_mapping() {
        let (_, item) =
            block_drop_item(1, &qexed_packet::net_types::Position { x: 0, y: 64, z: 0 }).unwrap();

        assert_eq!(item.item_count.0, 1);
        assert_eq!(item.item_id.unwrap().0, 1);
    }
}
