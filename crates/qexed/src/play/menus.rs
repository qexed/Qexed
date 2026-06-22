use std::{collections::HashSet, sync::Arc};

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

pub(super) struct MenuRenderContext<'a> {
    placeholders_enabled: bool,
    plugins: &'a crate::plugins::PluginManager,
    player: Option<crate::players::OnlinePlayer>,
    placeholder_context: crate::placeholders::PlaceholderContext,
}

impl<'a> MenuRenderContext<'a> {
    pub(super) fn from_player(
        server_config: Option<&qexed_config::app::qexed::server::Server>,
        plugins: &'a crate::plugins::PluginManager,
        players: &crate::players::PlayerManager,
        actor: uuid::Uuid,
    ) -> Self {
        Self {
            placeholders_enabled: server_config.is_some_and(|config| config.placeholders.enable),
            plugins,
            player: players.player_by_uuid(actor),
            placeholder_context: crate::placeholders::PlaceholderContext {
                online_players: players.online_count(),
                max_players: server_config.map(|config| config.max_player).unwrap_or(-1),
                lobby_online_servers: 0,
                lobby_total_servers: 0,
                lobby_servers: "none".to_string(),
            },
        }
    }

    pub(super) fn from_lobby(
        server_config: Option<&qexed_config::app::qexed::server::Server>,
        plugins: &'a crate::plugins::PluginManager,
        players: &crate::players::PlayerManager,
        actor: uuid::Uuid,
        lobby: &super::lobby::LobbyRuntime,
        lobby_status: &super::lobby::LobbyStatusSnapshot,
    ) -> Self {
        let labels = lobby.server_labels(lobby_status);
        Self {
            placeholders_enabled: server_config.is_some_and(|config| config.placeholders.enable),
            plugins,
            player: players.player_by_uuid(actor),
            placeholder_context: crate::placeholders::PlaceholderContext {
                online_players: players.online_count(),
                max_players: server_config.map(|config| config.max_player).unwrap_or(-1),
                lobby_online_servers: lobby.online_server_count(lobby_status),
                lobby_total_servers: lobby.total_server_count(lobby_status),
                lobby_servers: if labels.is_empty() {
                    "none".to_string()
                } else {
                    labels.join(", ")
                },
            },
        }
    }

    fn render(&self, text: &str) -> String {
        crate::placeholders::format_placeholders(
            self.placeholders_enabled,
            self.plugins,
            self.player.as_ref(),
            text,
            &self.placeholder_context,
        )
    }
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

    pub(super) fn reset_inventory_on_join(&self) -> bool {
        self.enabled() && self.config.reset_inventory_on_join
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

    pub(super) fn hotbar_item_matches(&self, slot: usize, actual: &Slot) -> bool {
        if !self.enabled() {
            return false;
        }
        self.config
            .hotbar_items
            .iter()
            .find(|item| usize::from(item.slot.min(8)) == slot)
            .map(hotbar_slot_item)
            .is_some_and(|expected| menu_slot_item_matches(actual, &expected))
    }

    pub(super) fn action_for_hotbar_item(&self, slot: usize, actual: &Slot) -> Option<MenuAction> {
        if !self.enabled() {
            return None;
        }
        self.config
            .hotbar_items
            .iter()
            .find(|item| usize::from(item.slot.min(8)) == slot)
            .filter(|item| {
                let expected = hotbar_slot_item(item);
                menu_slot_item_matches(actual, &expected)
            })
            .map(|item| item.action.clone())
    }

    pub(super) async fn open_menu<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        menu_id: &str,
        render_context: Option<&MenuRenderContext<'_>>,
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
            title: text_component(render_menu_text(&menu.title, render_context)),
        })
        .await?;
        sink.send(ContainerSetContent {
            window_id: VarInt(MENU_WINDOW_ID_RAW),
            state_id: VarInt(0),
            slot_data: menu_slots(rows, &menu.items, render_context),
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
        render_context: Option<&MenuRenderContext<'_>>,
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
        self.resync_menu_content(sink, menu, render_context).await?;
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
        render_context: Option<&MenuRenderContext<'_>>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let rows = menu.rows.clamp(1, 6);
        sink.send(ContainerSetContent {
            window_id: VarInt(MENU_WINDOW_ID),
            state_id: VarInt(0),
            slot_data: menu_slots(rows, &menu.items, render_context),
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

fn menu_slot_item_matches(actual: &Slot, expected: &Slot) -> bool {
    actual.item_count.0 > 0 && actual == expected
}

fn menu_slots(
    rows: u8,
    items: &[MenuItem],
    render_context: Option<&MenuRenderContext<'_>>,
) -> Vec<Slot> {
    let mut slots = vec![crate::inventory::empty_slot(); usize::from(rows) * 9];
    for item in items {
        let slot = usize::from(item.slot);
        if slot >= slots.len() {
            continue;
        }
        let name = render_menu_text(&item.name, render_context);
        let lore = render_menu_lore(&item.lore, render_context);
        slots[slot] = named_item_with_lore(&item.item, &name, &lore, 1);
    }
    slots
}

fn render_menu_text(text: &str, render_context: Option<&MenuRenderContext<'_>>) -> String {
    render_context
        .map(|context| context.render(text))
        .unwrap_or_else(|| text.to_string())
}

fn render_menu_lore(
    lore: &[String],
    render_context: Option<&MenuRenderContext<'_>>,
) -> Vec<String> {
    lore.iter()
        .map(|line| render_menu_text(line, render_context))
        .collect()
}

fn named_item_with_lore(item_name: &str, name: &str, lore: &[String], count: i32) -> Slot {
    let item_id = crate::inventory::item_id_for_name(normalize_resource_key(item_name).as_str())
        .unwrap_or_else(|| crate::inventory::item_id_for_name("minecraft:paper").unwrap_or(1));
    let mut item = crate::inventory::simple_item(item_id, count);
    let mut components = vec![ComponentsToAdd::MinecraftCustomData(
        minecraft::CustomData {
            data: qexed_nbt::Tag::Compound(Arc::new(
                [(
                    "qexed_menu_item".to_string(),
                    qexed_nbt::Tag::String(Arc::from(normalize_resource_key(item_name))),
                )]
                .into_iter()
                .collect(),
            )),
        },
    )];
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
    item.number_of_components_to_add = Some(VarInt(components.len() as i32));
    item.components_to_add = Some(components);
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

        assert!(runtime.reset_inventory_on_join());
        let changes = runtime.sync_hotbar_items(&mut inventory);

        assert_eq!(changes.len(), 1);
        assert!(runtime.fixed_hotbar_slot(4));
        assert!(!runtime.fixed_hotbar_slot(3));
        let item = inventory.hotbar_item(4).unwrap();
        assert!(runtime.hotbar_item_matches(4, item));
        assert_eq!(
            runtime.action_for_hotbar_item(4, item).unwrap().kind,
            MenuActionKind::OpenMenu
        );
    }

    #[test]
    fn hotbar_menu_action_requires_configured_item_stack() {
        let runtime = super::MenuRuntime::new(&Menus {
            enable: true,
            hotbar_items: vec![MenuHotbarItem {
                slot: 4,
                item: "minecraft:clock".to_string(),
                name: "Menu Clock".to_string(),
                action: MenuAction {
                    kind: MenuActionKind::OpenMenu,
                    target: "main".to_string(),
                    message: String::new(),
                },
                ..MenuHotbarItem::default()
            }],
            ..Menus::default()
        });
        let clock_id = crate::inventory::item_id_for_name("minecraft:clock").unwrap();
        let dirt_id = crate::inventory::item_id_for_name("minecraft:dirt").unwrap();
        let plain_clock = crate::inventory::simple_item(clock_id, 1);
        let dirt = crate::inventory::simple_item(dirt_id, 1);

        assert!(runtime.action_for_hotbar_item(4, &plain_clock).is_none());
        assert!(runtime.action_for_hotbar_item(4, &dirt).is_none());
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
            .handle_container_click(&mut sink, Some("main"), click, None)
            .await
            .unwrap()
            .unwrap();
        sink.flush().await.unwrap();

        assert_eq!(action.kind, MenuActionKind::Message);
        assert!(output.contains(&0x14));
        assert!(output.contains(&0x12));
    }

    #[test]
    fn menu_item_name_and_lore_render_placeholders() {
        let plugins = crate::plugins::PluginManager::empty_for_tests();
        let render_context = super::MenuRenderContext {
            placeholders_enabled: true,
            plugins: &plugins,
            player: Some(test_player("Tester")),
            placeholder_context: crate::placeholders::PlaceholderContext {
                online_players: 3,
                max_players: 20,
                lobby_online_servers: 1,
                lobby_total_servers: 2,
                lobby_servers: "Lobby".to_string(),
            },
        };
        let slots = super::menu_slots(
            1,
            &[qexed_config::app::qexed::server::MenuItem {
                slot: 0,
                item: "minecraft:paper".to_string(),
                name: "Hello %player_name%".to_string(),
                lore: vec![
                    "Online %online_players%/%max_players%".to_string(),
                    "Player %player_name%".to_string(),
                ],
                ..Default::default()
            }],
            Some(&render_context),
        );

        let components = slots[0].components_to_add.as_ref().expect("components");
        let name = components
            .iter()
            .find_map(|component| match component {
                qexed_protocol::types::ComponentsToAdd::MinecraftItemName(item_name) => {
                    Some(text_component_value(&item_name.name))
                }
                _ => None,
            })
            .expect("item name");
        let lore = components
            .iter()
            .find_map(|component| match component {
                qexed_protocol::types::ComponentsToAdd::MinecraftLore(lore) => Some(
                    lore.lines
                        .iter()
                        .map(text_component_value)
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .expect("lore");

        assert_eq!(name, "Hello Tester");
        assert_eq!(lore, vec!["Online 3/20", "Player Tester"]);
    }

    fn text_component_value(component: &qexed_protocol::types::TextComponent) -> String {
        let qexed_nbt::Tag::Compound(values) = component else {
            panic!("expected text component compound");
        };
        let Some(qexed_nbt::Tag::String(text)) = values.get("text") else {
            panic!("expected text field");
        };
        text.to_string()
    }

    fn test_player(username: &str) -> crate::players::OnlinePlayer {
        crate::players::OnlinePlayer {
            profile: qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::nil(),
                username: username.to_string(),
                properties: Vec::new(),
            },
            entity_id: 1,
            game_mode: 0,
            position: qexed_protocol::to_client::play::add_entity::EntityPosition::default(),
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "zh_cn".to_string(),
            displayed_skin_parts: crate::players::DEFAULT_DISPLAYED_SKIN_PARTS,
        }
    }
}
