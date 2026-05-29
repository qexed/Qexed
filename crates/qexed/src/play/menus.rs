use std::collections::HashSet;

use anyhow::Result;
use qexed_config::app::qexed::server::{MenuAction, MenuHotbarItem, MenuItem, Menus};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        container_close::ContainerClose, container_set_content::ContainerSetContent,
        container_set_slot, open_screen::OpenScreen,
    },
    to_server::play::container_click::ContainerClick,
    types::{ComponentsToAdd, Slot, minecraft},
};

use super::util::text_component;

const MENU_WINDOW_ID_RAW: i32 = 2;
const GENERIC_9X1_MENU_TYPE: i32 = 0;
const GENERIC_9X6_MENU_TYPE: i32 = 5;
pub(super) const MENU_WINDOW_ID: i32 = MENU_WINDOW_ID_RAW;

#[derive(Debug, Clone)]
pub(super) struct MenuRuntime {
    config: Menus,
}

impl MenuRuntime {
    pub(super) fn new(config: &Menus) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.config.enable
    }

    pub(super) fn sync_hotbar_items(
        &self,
        inventory: &mut crate::inventory::PlayerInventory,
    ) -> Vec<crate::inventory::InventorySlotChange> {
        if !self.enabled() {
            return Vec::new();
        }

        let mut changes = Vec::new();
        let configured_slots = self
            .config
            .hotbar_items
            .iter()
            .map(|item| usize::from(item.slot.min(8)))
            .collect::<HashSet<_>>();

        if !self.config.fixed_slots_only && self.config.reset_inventory_on_join {
            for slot in 0..9 {
                if configured_slots.contains(&slot) {
                    continue;
                }
                if let Some(change) =
                    inventory.set_hotbar_slot(slot, crate::inventory::empty_slot())
                {
                    changes.push(change);
                }
            }
        }

        for item in &self.config.hotbar_items {
            let slot = usize::from(item.slot.min(8));
            if !self.config.reset_inventory_on_join
                && inventory
                    .hotbar_item(slot)
                    .is_some_and(|existing| existing.item_count.0 > 0)
            {
                continue;
            }
            if let Some(change) = inventory.set_hotbar_slot(slot, hotbar_slot_item(item)) {
                changes.push(change);
            }
        }

        changes
    }

    pub(super) fn fixed_hotbar_slot(&self, slot: usize) -> bool {
        if !self.enabled() {
            return false;
        }
        if !self.config.fixed_slots_only {
            return slot < 9;
        }
        self.config
            .hotbar_items
            .iter()
            .any(|item| usize::from(item.slot.min(8)) == slot)
    }

    pub(super) fn action_for_hotbar_slot(&self, slot: usize) -> Option<MenuAction> {
        if !self.enabled() {
            return None;
        }
        self.config
            .hotbar_items
            .iter()
            .find(|item| usize::from(item.slot.min(8)) == slot)
            .map(|item| item.action.clone())
    }

    pub(super) async fn open_menu<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        menu_id: &str,
    ) -> Result<Option<String>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let Some(menu) = self.resolve_menu(menu_id) else {
            return Ok(None);
        };
        let rows = menu.rows.clamp(1, 6);
        sink.send(OpenScreen {
            window_id: VarInt(MENU_WINDOW_ID_RAW),
            menu_type: VarInt(menu_type_for_rows(rows)),
            title: text_component(menu.title.clone()),
        })
        .await?;
        sink.send(ContainerSetContent {
            window_id: VarInt(MENU_WINDOW_ID_RAW),
            state_id: VarInt(0),
            slot_data: menu_slots(rows, &menu.items),
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        Ok(Some(menu.id.clone()))
    }

    pub(super) async fn handle_container_click<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        active_menu_id: Option<&str>,
        click: ContainerClick,
    ) -> Result<Option<MenuAction>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.enabled() || click.window_id.0 != MENU_WINDOW_ID {
            return Ok(None);
        }

        sink.send(container_set_slot::ContainerSetContent {
            window_id: VarInt(-1),
            state_id: VarInt(0),
            slot: -1,
            slot_data: crate::inventory::empty_slot(),
        })
        .await?;

        let Some(menu) = active_menu_id.and_then(|id| self.resolve_menu(id)) else {
            return Ok(Some(MenuAction::default()));
        };
        self.resync_menu_content(sink, menu).await?;
        Ok(menu
            .items
            .iter()
            .find(|item| i16::from(item.slot) == click.slot)
            .map(|item| item.action.clone())
            .or_else(|| Some(MenuAction::default())))
    }

    pub(super) async fn close_menu<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        sink.send(ContainerClose {
            window_id: VarInt(MENU_WINDOW_ID),
        })
        .await?;
        Ok(())
    }

    async fn resync_menu_content<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        menu: &qexed_config::app::qexed::server::ChestMenu,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let rows = menu.rows.clamp(1, 6);
        sink.send(ContainerSetContent {
            window_id: VarInt(MENU_WINDOW_ID),
            state_id: VarInt(0),
            slot_data: menu_slots(rows, &menu.items),
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        Ok(())
    }

    fn resolve_menu(&self, menu_id: &str) -> Option<&qexed_config::app::qexed::server::ChestMenu> {
        let requested = menu_id.trim();
        if requested.is_empty() {
            return self.config.chests.first();
        }
        self.config
            .chests
            .iter()
            .find(|menu| menu.id.eq_ignore_ascii_case(requested))
    }
}

fn hotbar_slot_item(item: &MenuHotbarItem) -> Slot {
    named_item_with_lore(&item.item, &item.name, &item.lore, 1)
}

fn menu_slots(rows: u8, items: &[MenuItem]) -> Vec<Slot> {
    let mut slots = vec![crate::inventory::empty_slot(); usize::from(rows) * 9];
    for item in items {
        let slot = usize::from(item.slot);
        if slot >= slots.len() {
            continue;
        }
        slots[slot] = named_item_with_lore(&item.item, &item.name, &item.lore, 1);
    }
    slots
}

fn named_item_with_lore(item_name: &str, name: &str, lore: &[String], count: i32) -> Slot {
    let item_id = crate::inventory::item_id_for_name(normalize_resource_key(item_name).as_str())
        .unwrap_or_else(|| crate::inventory::item_id_for_name("minecraft:paper").unwrap_or(1));
    let mut item = crate::inventory::simple_item(item_id, count);
    let mut components = Vec::new();
    let name = name.trim();
    if !name.is_empty() {
        components.push(ComponentsToAdd::MinecraftItemName(minecraft::ItemName {
            name: text_component(name),
        }));
    }
    if !lore.is_empty() {
        components.push(ComponentsToAdd::MinecraftLore(minecraft::Lore {
            lines: lore
                .iter()
                .map(|line| text_component(line.as_str()))
                .collect(),
        }));
    }
    if !components.is_empty() {
        item.number_of_components_to_add = Some(VarInt(components.len() as i32));
        item.components_to_add = Some(components);
    }
    item
}

fn normalize_resource_key(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn menu_type_for_rows(rows: u8) -> i32 {
    (GENERIC_9X1_MENU_TYPE + i32::from(rows.saturating_sub(1))).min(GENERIC_9X6_MENU_TYPE)
}

#[cfg(test)]
mod tests {
    use qexed_config::app::qexed::server::{MenuAction, MenuActionKind, MenuHotbarItem, Menus};
    #[test]
    fn sync_hotbar_items_only_overwrites_configured_slots_by_default() {
        let runtime = super::MenuRuntime::new(&Menus {
            enable: true,
            reset_inventory_on_join: true,
            hotbar_items: vec![MenuHotbarItem {
                slot: 4,
                item: "minecraft:compass".to_string(),
                name: "Menu".to_string(),
                action: MenuAction {
                    kind: MenuActionKind::OpenMenu,
                    target: "main".to_string(),
                    message: String::new(),
                },
                ..MenuHotbarItem::default()
            }],
            ..Menus::default()
        });
        let mut inventory = crate::inventory::PlayerInventory::empty();

        let changes = runtime.sync_hotbar_items(&mut inventory);

        assert_eq!(changes.len(), 1);
        assert!(runtime.fixed_hotbar_slot(4));
        assert!(!runtime.fixed_hotbar_slot(3));
        assert_eq!(
            runtime.action_for_hotbar_slot(4).unwrap().kind,
            MenuActionKind::OpenMenu
        );
    }

    #[test]
    fn transfer_action_kind_is_supported_for_menu_items() {
        let action: MenuAction = toml::from_str(
            r#"
kind = "transfer"
target = "lobby-1"
message = "Connecting"
"#,
        )
        .unwrap();

        assert_eq!(action.kind, MenuActionKind::Transfer);
        assert_eq!(action.target, "lobby-1");
    }

    #[tokio::test]
    async fn menu_click_resyncs_content_before_returning_action() {
        let runtime = super::MenuRuntime::new(&Menus {
            enable: true,
            chests: vec![qexed_config::app::qexed::server::ChestMenu {
                id: "main".to_string(),
                rows: 1,
                items: vec![qexed_config::app::qexed::server::MenuItem {
                    slot: 0,
                    action: MenuAction {
                        kind: MenuActionKind::Message,
                        message: "clicked".to_string(),
                        ..MenuAction::default()
                    },
                    ..qexed_config::app::qexed::server::MenuItem::default()
                }],
                ..qexed_config::app::qexed::server::ChestMenu::default()
            }],
            ..Menus::default()
        });
        let mut output = Vec::new();
        let mut sink = qexed_tcp_connect::PacketSink::new(&mut output);
        let click = qexed_protocol::to_server::play::container_click::ContainerClick {
            window_id: qexed_packet::net_types::VarInt(super::MENU_WINDOW_ID),
            slot: 0,
            ..Default::default()
        };

        let action = runtime
            .handle_container_click(&mut sink, Some("main"), click)
            .await
            .unwrap()
            .unwrap();
        sink.flush().await.unwrap();

        assert_eq!(action.kind, MenuActionKind::Message);
        assert!(output.contains(&0x14));
        assert!(output.contains(&0x12));
    }
}
