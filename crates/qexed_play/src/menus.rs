//! 菜单运行时（v4 play/menus.rs 迁移）。
//!
//! v6 适配：
//! - PacketSink 改用 qexed_connection::transport
//! - 配置类型（Menus/MenuAction/ChestMenu...）改用 crate::config
//! - container_set_slot 结构体在 v6 叫 ContainerSetContent（字段名一致）
//! - 占位符渲染经 [`PlaceholderRenderer`]（crate::context）注入
//! - Geyser（基岩版表单）归 play-gameplay 任务；bedrock_form 生成 JSON 保留，
//!   发送通道由该任务的 GeyserRuntime 接线
//! - 物品注册表经 [`ItemRegistry`]（crate::drops）注入
//! - PlayerInventory 操作经 [`HotbarSync`] trait 收敛（v4 直接调
//!   crate::inventory::PlayerInventory；inventory 域由 play-gameplay 任务迁移）

use std::{collections::HashSet, sync::Arc};

use qexed_connection::transport::PacketSink;
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        container_close::ContainerClose, container_set_content::ContainerSetContent,
        container_set_slot, open_screen::OpenScreen,
    },
    to_server::play::container_click::ContainerClick,
    types::{ComponentsToAdd, Slot, minecraft},
};

use crate::config::{ChestMenu, MenuAction, MenuActionKind, MenuHotbarItem, MenuItem, Menus};
use crate::context::{PlaceholderContext, PlaceholderRenderer};
use crate::drops::ItemRegistry;
use crate::error::Result;
use crate::util::text_component;

const MENU_WINDOW_ID_RAW: i32 = 2;
const GENERIC_9X1_MENU_TYPE: i32 = 0;
const GENERIC_9X6_MENU_TYPE: i32 = 5;
const BEDROCK_MENU_FORM_PREFIX: &str = "qexed:menu:";
pub const MENU_WINDOW_ID: i32 = MENU_WINDOW_ID_RAW;

/// 快捷栏读写面（v4 crate::inventory::PlayerInventory 的菜单子集）。
///
/// TODO(play-gameplay)：inventory 域落地后由装配层桥接到真实 PlayerInventory。
pub trait HotbarSync: Send + Sync {
    /// 读取快捷栏槽物品。
    fn hotbar_item(&self, slot: usize) -> Option<Slot>;
    /// 写入快捷栏槽；返回是否发生变更。
    fn set_hotbar_slot(&self, slot: usize, item: Slot) -> bool;
    /// 全部快捷栏槽（0..9）。
    fn hotbar_slots(&self) -> Vec<Option<Slot>>;
}

#[derive(Debug, Clone)]
pub struct MenuRuntime {
    config: Menus,
}

pub struct MenuRenderContext<'a> {
    placeholders_enabled: bool,
    renderer: &'a dyn PlaceholderRenderer,
    player: Option<qexed_player::OnlinePlayer>,
    placeholder_context: PlaceholderContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockMenuForm {
    pub menu_id: String,
    pub plugin_form_id: String,
    pub json: String,
}

impl<'a> MenuRenderContext<'a> {
    pub fn new(
        placeholders_enabled: bool,
        renderer: &'a dyn PlaceholderRenderer,
        player: Option<qexed_player::OnlinePlayer>,
        placeholder_context: PlaceholderContext,
    ) -> Self {
        Self {
            placeholders_enabled,
            renderer,
            player,
            placeholder_context,
        }
    }

    fn render(&self, text: &str) -> String {
        self.renderer.render(
            self.placeholders_enabled,
            self.player.as_ref(),
            text,
            &self.placeholder_context,
        )
    }
}

impl MenuRuntime {
    pub fn new(config: &Menus) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.enable
    }

    pub fn reset_inventory_on_join(&self) -> bool {
        self.enabled() && self.config.reset_inventory_on_join
    }

    /// 快捷栏物品同步（v4 sync_hotbar_items；写操作经 HotbarSync）。
    pub fn sync_hotbar_items(
        &self,
        items: &dyn ItemRegistry,
        hotbar: &dyn HotbarSync,
    ) -> Vec<usize> {
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
                if hotbar.set_hotbar_slot(slot, items.empty_slot()) {
                    changes.push(slot);
                }
            }
        }

        for item in &self.config.hotbar_items {
            let slot = usize::from(item.slot.min(8));
            if !self.config.reset_inventory_on_join
                && hotbar
                    .hotbar_item(slot)
                    .is_some_and(|existing| existing.item_count.0 > 0)
            {
                continue;
            }
            if hotbar.set_hotbar_slot(slot, hotbar_slot_item(items, item)) {
                changes.push(slot);
            }
        }

        changes
    }

    /// v4 sync_hotbar_items(inventory) 的 v6 等价：直接操作 play 域的
    /// PlayerInventory（&mut），供 chat 的插件 ResetInventory 恢复菜单物品。
    pub fn sync_hotbar_items_inventory(
        &self,
        items: &dyn ItemRegistry,
        inventory: &mut crate::inventory::PlayerInventory,
    ) -> Vec<usize> {
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
                if inventory.set_hotbar_slot(slot, items.empty_slot()).is_some() {
                    changes.push(slot);
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
            if inventory
                .set_hotbar_slot(slot, hotbar_slot_item(items, item))
                .is_some()
            {
                changes.push(slot);
            }
        }

        changes
    }

    pub fn fixed_hotbar_slot(&self, slot: usize) -> bool {
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

    pub fn hotbar_item_matches(
        &self,
        items: &dyn ItemRegistry,
        slot: usize,
        actual: &Slot,
    ) -> bool {
        if !self.enabled() {
            return false;
        }
        self.config
            .hotbar_items
            .iter()
            .find(|item| usize::from(item.slot.min(8)) == slot)
            .map(|item| hotbar_slot_item(items, item))
            .is_some_and(|expected| menu_slot_item_matches(actual, &expected))
    }

    pub fn action_for_hotbar_item(
        &self,
        items: &dyn ItemRegistry,
        slot: usize,
        actual: &Slot,
    ) -> Option<MenuAction> {
        if !self.enabled() {
            return None;
        }
        self.config
            .hotbar_items
            .iter()
            .find(|item| usize::from(item.slot.min(8)) == slot)
            .filter(|item| {
                let expected = hotbar_slot_item(items, item);
                menu_slot_item_matches(actual, &expected)
            })
            .map(|item| item.action.clone())
    }

    pub async fn open_menu<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
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
            slot_data: menu_slots(items, rows, &menu.items, render_context),
            carried_item: items.empty_slot(),
        })
        .await?;
        Ok(Some(menu.id.clone()))
    }

    /// 基岩版表单 JSON（发送通道由 play-gameplay 的 GeyserRuntime 接线）。
    pub fn bedrock_form(
        &self,
        menu_id: &str,
        render_context: Option<&MenuRenderContext<'_>>,
    ) -> Result<Option<BedrockMenuForm>> {
        let Some(menu) = self.resolve_menu(menu_id) else {
            return Ok(None);
        };
        let title = render_menu_text(&menu.title, render_context);
        let buttons = menu
            .items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "text": bedrock_button_text(item, render_context),
                })
            })
            .collect::<Vec<_>>();
        let json = serde_json::json!({
            "type": "form",
            "title": title,
            "content": "",
            "buttons": buttons,
        });
        Ok(Some(BedrockMenuForm {
            menu_id: menu.id.clone(),
            plugin_form_id: bedrock_menu_form_id(&menu.id),
            json: serde_json::to_string(&json)?,
        }))
    }

    pub fn action_for_bedrock_form_response(
        &self,
        plugin_form_id: &str,
        response: &str,
    ) -> Option<(String, MenuAction)> {
        let menu_id = plugin_form_id.strip_prefix(BEDROCK_MENU_FORM_PREFIX)?;
        let menu = self.resolve_menu(menu_id)?;
        let response = response.trim();
        if response.is_empty() || response.eq_ignore_ascii_case("null") {
            return Some((menu.id.clone(), MenuAction::default()));
        }
        let index = serde_json::from_str::<serde_json::Value>(response)
            .ok()
            .and_then(|value| bedrock_form_button_index(&value))?;
        Some((
            menu.id.clone(),
            menu.items
                .get(index)
                .map(|item| item.action.clone())
                .unwrap_or_default(),
        ))
    }

    pub fn is_bedrock_menu_form_id(&self, plugin_form_id: &str) -> bool {
        plugin_form_id.starts_with(BEDROCK_MENU_FORM_PREFIX)
    }

    pub async fn handle_container_click<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
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
            slot_data: items.empty_slot(),
        })
        .await?;

        let Some(menu) = active_menu_id.and_then(|id| self.resolve_menu(id)) else {
            return Ok(Some(MenuAction::default()));
        };
        self.resync_menu_content(sink, items, menu, render_context)
            .await?;
        Ok(menu
            .items
            .iter()
            .find(|item| i16::from(item.slot) == click.slot)
            .map(|item| item.action.clone())
            .or_else(|| Some(MenuAction::default())))
    }

    pub async fn close_menu<W>(
        &self,
        sink: &mut PacketSink<W>,
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
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
        menu: &ChestMenu,
        render_context: Option<&MenuRenderContext<'_>>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let rows = menu.rows.clamp(1, 6);
        sink.send(ContainerSetContent {
            window_id: VarInt(MENU_WINDOW_ID),
            state_id: VarInt(0),
            slot_data: menu_slots(items, rows, &menu.items, render_context),
            carried_item: items.empty_slot(),
        })
        .await?;
        Ok(())
    }

    fn resolve_menu(&self, menu_id: &str) -> Option<&ChestMenu> {
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

fn hotbar_slot_item(items: &dyn ItemRegistry, item: &MenuHotbarItem) -> Slot {
    named_item_with_lore(items, &item.item, &item.name, &item.lore, 1)
}

fn menu_slot_item_matches(actual: &Slot, expected: &Slot) -> bool {
    actual.item_count.0 > 0 && actual == expected
}

fn menu_slots(
    items: &dyn ItemRegistry,
    rows: u8,
    items_config: &[MenuItem],
    render_context: Option<&MenuRenderContext<'_>>,
) -> Vec<Slot> {
    let mut slots = vec![items.empty_slot(); usize::from(rows) * 9];
    for item in items_config {
        let slot = usize::from(item.slot);
        if slot >= slots.len() {
            continue;
        }
        let name = render_menu_text(&item.name, render_context);
        let lore = render_menu_lore(&item.lore, render_context);
        slots[slot] = named_item_with_lore(items, &item.item, &name, &lore, 1);
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

fn bedrock_menu_form_id(menu_id: &str) -> String {
    format!("{BEDROCK_MENU_FORM_PREFIX}{menu_id}")
}

fn bedrock_button_text(item: &MenuItem, render_context: Option<&MenuRenderContext<'_>>) -> String {
    let name = render_menu_text(&item.name, render_context);
    if !name.trim().is_empty() {
        return name;
    }
    if !item.action.target.trim().is_empty()
        && matches!(
            item.action.kind,
            MenuActionKind::OpenMenu | MenuActionKind::Transfer | MenuActionKind::Command
        )
    {
        return item.action.target.trim().to_string();
    }
    normalize_resource_key(&item.item)
}

fn bedrock_form_button_index(value: &serde_json::Value) -> Option<usize> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
        .and_then(|value| usize::try_from(value).ok())
}

fn named_item_with_lore(
    items: &dyn ItemRegistry,
    item_name: &str,
    name: &str,
    lore: &[String],
    count: i32,
) -> Slot {
    let item_id = items
        .item_id_for_name(normalize_resource_key(item_name).as_str())
        .or_else(|| items.item_id_for_name("minecraft:paper"))
        .unwrap_or(1);
    let mut item = items.simple_item(item_id, count);
    let name = name.trim();
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
    use super::*;
    use crate::config::MenuActionKind;
    use qexed_packet::net_types::VarInt as V;

    struct TestItems;

    impl ItemRegistry for TestItems {
        fn is_air_block_state(&self, block_state: i32) -> bool {
            block_state == 0
        }
        fn air_block_state(&self) -> i32 {
            0
        }
        fn picked_item_for_block_state(&self, block_state: i32) -> Option<i32> {
            (block_state != 0).then_some(block_state)
        }
        fn item_id_for_name(&self, name: &str) -> Option<i32> {
            match name {
                "minecraft:paper" => Some(1),
                "minecraft:compass" => Some(2),
                "minecraft:clock" => Some(3),
                "minecraft:dirt" => Some(4),
                _ => None,
            }
        }
        fn simple_item(&self, item_id: i32, count: i32) -> Slot {
            // 26.3 Slot 序列化要求 item_count>0 时 components 计数字段存在。
            crate::inventory::simple_item(item_id, count)
        }
        fn empty_slot(&self) -> Slot {
            Slot::default()
        }
    }

    #[derive(Default)]
    struct TestHotbar {
        slots: std::sync::Mutex<Vec<Option<Slot>>>,
    }

    /// 空槽语义与 PlayerInventory 对齐：None 与 Some(空物品) 等价（写空不算变更）。
fn is_empty_hotbar_slot(slot: &Option<Slot>) -> bool {
    slot.as_ref().is_none_or(|item| item.item_count.0 <= 0)
}

impl HotbarSync for TestHotbar {
        fn hotbar_item(&self, slot: usize) -> Option<Slot> {
            self.slots
                .lock()
                .unwrap()
                .get(slot)
                .cloned()
                .flatten()
                .filter(|item| item.item_count.0 > 0)
        }
        fn set_hotbar_slot(&self, slot: usize, item: Slot) -> bool {
            let mut slots = self.slots.lock().unwrap();
            while slots.len() <= slot {
                slots.push(None);
            }
            let was_empty = is_empty_hotbar_slot(&slots[slot]);
            let now_empty = item.item_count.0 <= 0;
            let changed = was_empty != now_empty
                || (!was_empty && slots[slot].as_ref() != Some(&item));
            slots[slot] = (!now_empty).then_some(item);
            changed
        }
        fn hotbar_slots(&self) -> Vec<Option<Slot>> {
            self.slots.lock().unwrap().clone()
        }
    }

    #[tokio::test]
    async fn menu_click_resyncs_content_before_returning_action() {
        let runtime = MenuRuntime::new(&Menus {
            enable: true,
            chests: vec![ChestMenu {
                id: "main".to_string(),
                rows: 1,
                items: vec![MenuItem {
                    slot: 0,
                    action: MenuAction {
                        kind: MenuActionKind::Message,
                        message: "clicked".to_string(),
                        ..MenuAction::default()
                    },
                    ..MenuItem::default()
                }],
                ..ChestMenu::default()
            }],
            ..Menus::default()
        });
        let mut output = Vec::new();
        let mut sink = PacketSink::new(&mut output);
        let click = ContainerClick {
            window_id: V(MENU_WINDOW_ID),
            slot: 0,
            ..Default::default()
        };

        let action = runtime
            .handle_container_click(&mut sink, &TestItems, Some("main"), click, None)
            .await
            .unwrap()
            .unwrap();
        sink.flush().await.unwrap();

        assert_eq!(action.kind, MenuActionKind::Message);
        assert_eq!(action.message, "clicked");
    }

    #[test]
    fn bedrock_form_uses_item_order_instead_of_slots() {
        let runtime = MenuRuntime::new(&Menus {
            enable: true,
            chests: vec![ChestMenu {
                id: "main".to_string(),
                title: "Main".to_string(),
                items: vec![
                    MenuItem {
                        slot: 8,
                        name: "First".to_string(),
                        action: MenuAction {
                            kind: MenuActionKind::Message,
                            message: "first".to_string(),
                            ..MenuAction::default()
                        },
                        ..MenuItem::default()
                    },
                    MenuItem {
                        slot: 0,
                        name: "Second".to_string(),
                        action: MenuAction {
                            kind: MenuActionKind::Message,
                            message: "second".to_string(),
                            ..MenuAction::default()
                        },
                        ..MenuItem::default()
                    },
                ],
                ..ChestMenu::default()
            }],
            ..Menus::default()
        });

        let form = runtime.bedrock_form("main", None).unwrap().unwrap();
        let json: serde_json::Value = serde_json::from_str(&form.json).unwrap();
        assert_eq!(form.plugin_form_id, "qexed:menu:main");
        assert_eq!(json["buttons"][0]["text"], "First");
        assert_eq!(json["buttons"][1]["text"], "Second");

        let (_, action) = runtime
            .action_for_bedrock_form_response(&form.plugin_form_id, "1")
            .unwrap();
        assert_eq!(action.message, "second");

        let (_, action) = runtime
            .action_for_bedrock_form_response(&form.plugin_form_id, "\"0\"")
            .unwrap();
        assert_eq!(action.message, "first");
    }

    #[test]
    fn bedrock_form_null_response_closes_menu_without_action() {
        let runtime = MenuRuntime::new(&Menus {
            enable: true,
            chests: vec![ChestMenu {
                id: "main".to_string(),
                items: vec![MenuItem {
                    action: MenuAction {
                        kind: MenuActionKind::Message,
                        message: "clicked".to_string(),
                        ..MenuAction::default()
                    },
                    ..MenuItem::default()
                }],
                ..ChestMenu::default()
            }],
            ..Menus::default()
        });

        let (_, action) = runtime
            .action_for_bedrock_form_response("qexed:menu:main", "null")
            .unwrap();
        assert_eq!(action.kind, MenuActionKind::None);
    }

    #[test]
    fn sync_hotbar_items_only_overwrites_configured_slots_by_default() {
        let runtime = MenuRuntime::new(&Menus {
            enable: true,
            reset_inventory_on_join: true,
            hotbar_items: vec![MenuHotbarItem {
                slot: 4,
                item: "minecraft:compass".to_string(),
                name: "Menu".to_string(),
                action: MenuAction {
                    kind: MenuActionKind::OpenMenu,
                    target: "main".to_string(),
                    ..MenuAction::default()
                },
                ..MenuHotbarItem::default()
            }],
            ..Menus::default()
        });
        let hotbar = TestHotbar::default();

        assert!(runtime.reset_inventory_on_join());
        let changes = runtime.sync_hotbar_items(&TestItems, &hotbar);

        // reset_inventory_on_join + 非 fixed_slots_only：清空未配置槽（空槽不产生变更）+
        // 写入 1 个配置槽（v4 同名测试断言 1 次变更：只覆写已配置的槽）。
        assert_eq!(changes.len(), 1);
        assert!(runtime.fixed_hotbar_slot(4));
        assert!(!runtime.fixed_hotbar_slot(3));
        let item = hotbar.hotbar_item(4).unwrap();
        assert!(runtime.hotbar_item_matches(&TestItems, 4, &item));
        assert_eq!(
            runtime
                .action_for_hotbar_item(&TestItems, 4, &item)
                .unwrap()
                .kind,
            MenuActionKind::OpenMenu
        );
    }
}
