use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use qexed_config::app::qexed::server::{
    Lobby, LobbyAction, LobbyActionKind, LobbyBossBarColor, LobbyBossBarOverlay, LobbyMenuItem,
    LobbyServer,
};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        boss_event::{BossBarColor, BossBarOverlay, BossBarProperties, BossEvent},
        container_set_content::ContainerSetContent,
        container_set_slot,
        open_screen::OpenScreen,
        system_chat::SystemChat,
        transfer::Transfer,
    },
    to_server::play::{container_click::ContainerClick, interact::Interact},
    types::{ComponentsToAdd, Slot, minecraft},
};

use super::util::text_component;

const LOBBY_MENU_WINDOW_ID: i32 = 1;
const GENERIC_9X1_MENU_TYPE: i32 = 0;
const GENERIC_9X6_MENU_TYPE: i32 = 5;
const LOBBY_BOSS_BAR_UUID: uuid::Uuid = uuid::Uuid::from_u128(0x6c6f6262795f6261725f7165786564);
const MIN_HEALTH_CHECK_INTERVAL_SECS: u64 = 1;
const MAX_HEALTH_CHECK_INTERVAL_SECS: u64 = 300;
const MIN_HEALTH_CHECK_TIMEOUT_MS: u64 = 50;
const MAX_HEALTH_CHECK_TIMEOUT_MS: u64 = 5_000;
pub(super) const MENU_WINDOW_ID: i32 = LOBBY_MENU_WINDOW_ID;

#[derive(Debug, Clone)]
pub(super) struct LobbyRuntime {
    config: Lobby,
    npc_actions: HashMap<String, LobbyAction>,
    servers: HashMap<String, LobbyServer>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct LobbyInteractionOutcome {
    pub handled: bool,
    pub opened_menu: bool,
}

impl LobbyRuntime {
    pub(super) fn new(config: &Lobby) -> Self {
        Self {
            config: config.clone(),
            npc_actions: config
                .npc_actions
                .iter()
                .filter(|action| !action.entity.trim().is_empty())
                .map(|action| (action.entity.trim().to_string(), action.action.clone()))
                .collect(),
            servers: config
                .servers
                .iter()
                .filter(|server| !server.id.trim().is_empty())
                .map(|server| (server.id.trim().to_string(), server.clone()))
                .collect(),
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.config.enable
    }

    pub(super) fn protect_world(&self) -> bool {
        self.enabled() && self.config.protect_world
    }

    pub(super) fn navigator_slot(&self) -> Option<usize> {
        if !self.enabled() || !self.config.navigator.enable {
            return None;
        }
        Some(usize::from(self.config.navigator.slot.min(8)))
    }

    pub(super) fn navigator_item(&self) -> Option<Slot> {
        if !self.enabled() || !self.config.navigator.enable {
            return None;
        }
        Some(named_item(
            &self.config.navigator.item,
            self.config.navigator.name.as_str(),
            1,
        ))
    }

    pub(super) fn navigator_item_matches(&self, selected_slot: usize, actual: &Slot) -> bool {
        self.navigator_slot() == Some(selected_slot)
            && self
                .navigator_item()
                .is_some_and(|expected| actual.item_count.0 > 0 && actual == &expected)
    }

    pub(super) fn broadcast_interval(&self) -> Option<Duration> {
        if !self.enabled()
            || !self.config.broadcast.enable
            || self.config.broadcast.messages.is_empty()
        {
            return None;
        }
        Some(Duration::from_secs(
            self.config.broadcast.interval_secs.max(1),
        ))
    }

    pub(super) fn broadcast_message(
        &self,
        index: usize,
        status: &LobbyStatusSnapshot,
    ) -> Option<String> {
        if self.config.broadcast.messages.is_empty() {
            return None;
        }
        Some(self.render_status_placeholders(
            self.config.broadcast.messages[index % self.config.broadcast.messages.len()].as_str(),
            status,
        ))
    }

    pub(super) fn boss_bar_enabled(&self) -> bool {
        self.enabled() && self.config.boss_bar.enable
    }

    pub(super) async fn show_boss_bar<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.boss_bar_enabled() {
            return Ok(());
        }
        sink.send(BossEvent::add_with_options(
            LOBBY_BOSS_BAR_UUID,
            text_component(self.boss_bar_title()),
            1.0,
            boss_bar_color(self.config.boss_bar.color),
            boss_bar_overlay(self.config.boss_bar.overlay),
            BossBarProperties {
                darken_screen: self.config.boss_bar.darken_screen,
                play_music: self.config.boss_bar.play_music,
                create_world_fog: self.config.boss_bar.create_world_fog,
            },
        ))
        .await?;
        Ok(())
    }

    pub(super) async fn update_boss_bar<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        message: &str,
        progress: f32,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.boss_bar_enabled() {
            return Ok(());
        }
        sink.send(BossEvent::update_name(
            LOBBY_BOSS_BAR_UUID,
            text_component(message.trim()),
        ))
        .await?;
        sink.send(BossEvent::update_progress(
            LOBBY_BOSS_BAR_UUID,
            progress.clamp(0.0, 1.0),
        ))
        .await?;
        Ok(())
    }

    pub(super) async fn update_boss_bar_status<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        status: &LobbyStatusSnapshot,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let summary = self.status_summary(status);
        self.update_boss_bar(sink, &summary, status.online_ratio())
            .await
    }

    pub(super) async fn remove_boss_bar<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.boss_bar_enabled() {
            sink.send(BossEvent::remove(LOBBY_BOSS_BAR_UUID)).await?;
        }
        Ok(())
    }

    pub(super) fn status_refresh_interval(&self) -> Option<Duration> {
        if self.enabled() && !self.config.servers.is_empty() {
            Some(Duration::from_secs(
                self.config.health_check.interval_secs.clamp(
                    MIN_HEALTH_CHECK_INTERVAL_SECS,
                    MAX_HEALTH_CHECK_INTERVAL_SECS,
                ),
            ))
        } else {
            None
        }
    }

    pub(super) async fn refresh_status(&self) -> LobbyStatusSnapshot {
        let mut servers = HashMap::new();
        for server in &self.config.servers {
            let id = server.id.trim();
            if id.is_empty() {
                continue;
            }
            let status = if !server.enable {
                LobbyServerStatus::Disabled
            } else if server.maintenance {
                LobbyServerStatus::Maintenance
            } else if server.host.trim().is_empty() || server.port == 0 {
                LobbyServerStatus::Offline
            } else if self.server_is_reachable(server).await {
                LobbyServerStatus::Online
            } else {
                LobbyServerStatus::Offline
            };
            servers.insert(id.to_string(), status);
        }
        LobbyStatusSnapshot { servers }
    }

    pub(super) fn status_summary(&self, status: &LobbyStatusSnapshot) -> String {
        let total = self
            .config
            .servers
            .iter()
            .filter(|server| !server.id.trim().is_empty())
            .count();
        if total == 0 {
            return "Lobby servers: none".to_string();
        }
        let online = self
            .config
            .servers
            .iter()
            .filter(|server| {
                status.status_for_server(server.id.trim()) == LobbyServerStatus::Online
            })
            .count();
        format!("Lobby servers: {online}/{total} online")
    }

    pub(super) fn render_status_placeholders(
        &self,
        template: &str,
        status: &LobbyStatusSnapshot,
    ) -> String {
        template
            .replace(
                "{online_servers}",
                &self.online_server_count(status).to_string(),
            )
            .replace("{total_servers}", &self.total_server_count().to_string())
            .replace("{servers}", &self.server_labels(status).join(", "))
    }

    pub(super) async fn open_menu<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        status: &LobbyStatusSnapshot,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.enabled() {
            return Ok(());
        }

        let rows = self.menu_rows();
        sink.send(OpenScreen {
            window_id: VarInt(LOBBY_MENU_WINDOW_ID),
            menu_type: VarInt(menu_type_for_rows(rows)),
            title: text_component(self.config.menu_title.clone()),
        })
        .await?;
        sink.send(ContainerSetContent {
            window_id: VarInt(LOBBY_MENU_WINDOW_ID),
            state_id: VarInt(0),
            slot_data: self.menu_slots(rows, status),
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        Ok(())
    }

    pub(super) async fn refresh_open_menu<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        status: &LobbyStatusSnapshot,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.enabled() {
            return Ok(());
        }
        let rows = self.menu_rows();
        sink.send(ContainerSetContent {
            window_id: VarInt(LOBBY_MENU_WINDOW_ID),
            state_id: VarInt(0),
            slot_data: self.menu_slots(rows, status),
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        Ok(())
    }

    pub(super) async fn handle_use_item<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        selected_slot: usize,
        held_item: &Slot,
        status: &LobbyStatusSnapshot,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.navigator_item_matches(selected_slot, held_item) {
            return Ok(false);
        }
        self.open_menu(sink, status).await?;
        Ok(true)
    }

    pub(super) async fn handle_container_click<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        click: ContainerClick,
        status: &LobbyStatusSnapshot,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.enabled() || click.window_id.0 != LOBBY_MENU_WINDOW_ID {
            return Ok(false);
        }

        sink.send(container_set_slot::ContainerSetContent {
            window_id: VarInt(-1),
            state_id: VarInt(0),
            slot: -1,
            slot_data: crate::inventory::empty_slot(),
        })
        .await?;

        let Some(item) = self.menu_item_for_slot(click.slot) else {
            return Ok(true);
        };
        self.run_action(sink, &item.action, status).await?;
        Ok(true)
    }

    pub(super) async fn handle_entity_interact<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        entities: &crate::entities::EntityManager,
        interact: Interact,
        status: &LobbyStatusSnapshot,
    ) -> Result<LobbyInteractionOutcome>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.enabled() || !is_primary_interact(&interact) {
            return Ok(LobbyInteractionOutcome::default());
        }

        let Some(entity) = entities.entity_by_runtime_id(interact.entity_id.0) else {
            return Ok(LobbyInteractionOutcome::default());
        };
        let Some(action) = self.npc_actions.get(&entity.key) else {
            return Ok(LobbyInteractionOutcome::default());
        };
        self.run_action(sink, action, status).await?;
        Ok(LobbyInteractionOutcome {
            handled: true,
            opened_menu: action_opens_menu(action),
        })
    }

    async fn run_action<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        action: &LobbyAction,
        status: &LobbyStatusSnapshot,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        match action.kind {
            LobbyActionKind::None => {}
            LobbyActionKind::OpenMenu => {
                self.open_menu(sink, status).await?;
            }
            LobbyActionKind::Message => {
                let message = if action.message.trim().is_empty() {
                    action.target.as_str()
                } else {
                    action.message.as_str()
                };
                if !message.trim().is_empty() {
                    sink.send(SystemChat {
                        content: text_component(message),
                        overlay: false,
                    })
                    .await?;
                }
            }
            LobbyActionKind::Transfer => {
                self.transfer(sink, action, status).await?;
            }
        }
        Ok(())
    }

    pub(super) async fn transfer_to_server<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        server_id: &str,
        status: &LobbyStatusSnapshot,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.transfer_to_server_with_message(sink, server_id, "", status)
            .await
    }

    pub(super) async fn transfer_to_server_with_message<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        server_id: &str,
        message: &str,
        status: &LobbyStatusSnapshot,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.enabled() {
            return Ok(false);
        }
        let action = LobbyAction {
            kind: LobbyActionKind::Transfer,
            target: server_id.trim().to_string(),
            message: message.trim().to_string(),
        };
        self.transfer(sink, &action, status).await?;
        Ok(true)
    }

    pub(super) fn server_labels(&self, status: &LobbyStatusSnapshot) -> Vec<String> {
        let mut servers = self
            .config
            .servers
            .iter()
            .map(|server| {
                let name = display_server_name(server)
                    .unwrap_or(server.id.as_str())
                    .to_string();
                format!("{name} {}", status.label_for_server(server.id.trim()))
            })
            .collect::<Vec<_>>();
        servers.sort();
        servers
    }

    pub(super) fn server_command_entries(&self, status: &LobbyStatusSnapshot) -> Vec<String> {
        let mut servers = self
            .config
            .servers
            .iter()
            .filter_map(|server| {
                let id = server.id.trim();
                if id.is_empty() {
                    return None;
                }
                let name = display_server_name(server).unwrap_or(id);
                let label = status.label_for_server(id);
                Some(if name == id {
                    format!("{id} {label}")
                } else {
                    format!("{id}={name} {label}")
                })
            })
            .collect::<Vec<_>>();
        servers.sort();
        servers
    }

    pub(super) fn resolve_server_id(&self, server_id: &str) -> Option<String> {
        let server_id = server_id.trim();
        if server_id.is_empty() {
            return None;
        }
        if self.servers.contains_key(server_id) {
            return Some(server_id.to_string());
        }
        self.config
            .servers
            .iter()
            .filter_map(|server| {
                let id = server.id.trim();
                (!id.is_empty()).then_some((id, display_server_name(server)))
            })
            .find_map(|(id, name)| {
                if id.eq_ignore_ascii_case(server_id)
                    || name.is_some_and(|name| name.eq_ignore_ascii_case(server_id))
                {
                    Some(id.to_string())
                } else {
                    None
                }
            })
    }

    pub(super) fn total_server_count(&self) -> usize {
        self.config
            .servers
            .iter()
            .filter(|server| !server.id.trim().is_empty())
            .count()
    }

    pub(super) fn online_server_count(&self, status: &LobbyStatusSnapshot) -> usize {
        self.config
            .servers
            .iter()
            .filter(|server| {
                status.status_for_server(server.id.trim()) == LobbyServerStatus::Online
            })
            .count()
    }

    async fn transfer<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        action: &LobbyAction,
        status: &LobbyStatusSnapshot,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let Some(server) = self.servers.get(action.target.trim()) else {
            sink.send(SystemChat {
                content: text_component(format!("Server is unavailable: {}", action.target)),
                overlay: false,
            })
            .await?;
            return Ok(());
        };

        if server_is_known_unavailable(status, &server.id) {
            send_unavailable_server_message(sink, server).await?;
            return Ok(());
        }

        if server.host.trim().is_empty()
            || server.port == 0
            || !self.server_is_reachable(server).await
        {
            send_unavailable_server_message(sink, server).await?;
            return Ok(());
        }

        let message = if action.message.trim().is_empty() {
            format!(
                "Connecting to {}...",
                display_server_name(server).unwrap_or(server.id.as_str())
            )
        } else {
            action.message.clone()
        };
        sink.send(SystemChat {
            content: text_component(message),
            overlay: false,
        })
        .await?;
        sink.send(Transfer::new(server.host.trim(), server.port))
            .await?;
        Ok(())
    }

    fn menu_rows(&self) -> u8 {
        self.config.menu_rows.clamp(1, 6)
    }

    fn boss_bar_title(&self) -> &str {
        let title = self.config.boss_bar.title.trim();
        if title.is_empty() { "Qexed" } else { title }
    }

    async fn server_is_reachable(&self, server: &LobbyServer) -> bool {
        server_is_reachable(server, self.health_check_timeout()).await
    }

    fn health_check_timeout(&self) -> Duration {
        Duration::from_millis(
            self.config
                .health_check
                .timeout_ms
                .clamp(MIN_HEALTH_CHECK_TIMEOUT_MS, MAX_HEALTH_CHECK_TIMEOUT_MS),
        )
    }

    fn menu_slots(&self, rows: u8, status: &LobbyStatusSnapshot) -> Vec<Slot> {
        let mut slots = vec![crate::inventory::empty_slot(); usize::from(rows) * 9];
        for item in &self.config.menu_items {
            let slot = usize::from(item.slot);
            if slot >= slots.len() {
                continue;
            }
            slots[slot] = self.menu_slot_item(item, status);
        }
        slots
    }

    fn menu_slot_item(&self, item: &LobbyMenuItem, status: &LobbyStatusSnapshot) -> Slot {
        if item.action.kind != LobbyActionKind::Transfer {
            let name = self.render_status_placeholders(item.name.as_str(), status);
            let lore = if item.lore.is_empty() {
                Vec::new()
            } else {
                self.render_menu_lore(item, None, LobbyServerStatus::Unknown, status)
            };
            return named_item_with_lore(&item.item, name.as_str(), &lore, 1);
        }

        let server_id = item.action.target.trim();
        let server_status = status.status_for_server(server_id);
        let server = self.servers.get(server_id);
        let item_name =
            self.render_menu_item_text(item.name.as_str(), server, server_status, status);
        let display_name = format!("{} {}", item_name.trim(), server_status.label())
            .trim()
            .to_string();
        let item_name = menu_item_name_for_status(item, server_status);
        let lore = self.render_menu_lore(item, server, server_status, status);
        named_item_with_lore(item_name, display_name.as_str(), &lore, 1)
    }

    fn menu_item_for_slot(&self, slot: i16) -> Option<&LobbyMenuItem> {
        let slot = u8::try_from(slot).ok()?;
        self.config.menu_items.iter().find(|item| item.slot == slot)
    }

    fn render_menu_lore(
        &self,
        item: &LobbyMenuItem,
        server: Option<&LobbyServer>,
        server_status: LobbyServerStatus,
        status: &LobbyStatusSnapshot,
    ) -> Vec<String> {
        if item.lore.is_empty() {
            return vec![server_status_description(server_status, server)];
        }
        item.lore
            .iter()
            .map(|line| self.render_menu_item_text(line, server, server_status, status))
            .collect()
    }

    fn render_menu_item_text(
        &self,
        template: &str,
        server: Option<&LobbyServer>,
        server_status: LobbyServerStatus,
        status: &LobbyStatusSnapshot,
    ) -> String {
        let rendered = self.render_status_placeholders(template, status);
        let server_id = server.map(|server| server.id.trim()).unwrap_or_default();
        let server_name = server.and_then(display_server_name).unwrap_or(server_id);
        let maintenance_message = server
            .map(|server| server.maintenance_message.trim())
            .unwrap_or_default();
        rendered
            .replace("{server}", server_id)
            .replace("{server_name}", server_name)
            .replace("{status}", server_status.key())
            .replace("{status_label}", server_status.label())
            .replace(
                "{status_description}",
                &server_status_description(server_status, server),
            )
            .replace("{maintenance_message}", maintenance_message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LobbyServerStatus {
    Unknown,
    Online,
    Offline,
    Disabled,
    Maintenance,
}

impl LobbyServerStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Unknown => "[checking]",
            Self::Online => "[online]",
            Self::Offline => "[offline]",
            Self::Disabled => "[disabled]",
            Self::Maintenance => "[maintenance]",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Unknown => "Status is being checked.",
            Self::Online => "Click to connect.",
            Self::Offline => "Backend is currently unavailable.",
            Self::Disabled => "Backend entry is disabled.",
            Self::Maintenance => "Backend is under maintenance.",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Unknown => "checking",
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Disabled => "disabled",
            Self::Maintenance => "maintenance",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct LobbyStatusSnapshot {
    servers: HashMap<String, LobbyServerStatus>,
}

impl LobbyStatusSnapshot {
    #[cfg(test)]
    pub(super) fn from_servers_for_tests(servers: HashMap<String, LobbyServerStatus>) -> Self {
        Self { servers }
    }

    pub(super) fn changed_servers_since(
        &self,
        previous: &Self,
    ) -> Vec<(String, LobbyServerStatus)> {
        let mut changes = self
            .servers
            .iter()
            .filter_map(|(server, status)| {
                (previous.status_for_server(server) != *status).then(|| (server.clone(), *status))
            })
            .collect::<Vec<_>>();
        changes.sort_by(|left, right| left.0.cmp(&right.0));
        changes
    }

    pub(super) fn status_for_server(&self, server_id: &str) -> LobbyServerStatus {
        self.servers
            .get(server_id.trim())
            .copied()
            .unwrap_or(LobbyServerStatus::Unknown)
    }

    fn online_ratio(&self) -> f32 {
        if self.servers.is_empty() {
            return 1.0;
        }
        let online = self
            .servers
            .values()
            .filter(|status| **status == LobbyServerStatus::Online)
            .count();
        online as f32 / self.servers.len() as f32
    }

    fn label_for_server(&self, server_id: &str) -> &'static str {
        self.status_for_server(server_id).label()
    }
}

pub(super) fn sync_navigator_item(
    inventory: &mut crate::inventory::PlayerInventory,
    lobby: &LobbyRuntime,
) -> Vec<crate::inventory::InventorySlotChange> {
    let Some(slot) = lobby.navigator_slot() else {
        return Vec::new();
    };
    let Some(item) = lobby.navigator_item() else {
        return Vec::new();
    };
    inventory.set_hotbar_slot(slot, item).into_iter().collect()
}

fn named_item(item_name: &str, name: &str, count: i32) -> Slot {
    named_item_with_lore(item_name, name, &Vec::new(), count)
}

fn menu_item_name_for_status(item: &LobbyMenuItem, status: LobbyServerStatus) -> &str {
    match status {
        LobbyServerStatus::Unknown => item.unknown_item.as_str(),
        LobbyServerStatus::Online => item.item.as_str(),
        LobbyServerStatus::Offline => item.offline_item.as_str(),
        LobbyServerStatus::Disabled => item.disabled_item.as_str(),
        LobbyServerStatus::Maintenance => item.maintenance_item.as_str(),
    }
}

fn named_item_with_lore(item_name: &str, name: &str, lore: &[String], count: i32) -> Slot {
    let item_id = crate::inventory::item_id_for_name(normalize_resource_key(item_name).as_str())
        .unwrap_or_else(|| crate::inventory::item_id_for_name("minecraft:paper").unwrap_or(1));
    let mut item = crate::inventory::simple_item(item_id, count);
    let name = name.trim();
    let mut components = vec![ComponentsToAdd::MinecraftCustomData(
        minecraft::CustomData {
            data: qexed_nbt::Tag::Compound(Arc::new(
                [(
                    "qexed_lobby_item".to_string(),
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

pub(super) fn is_primary_interact(interact: &Interact) -> bool {
    interact.hand.0 == 0
}

fn action_opens_menu(action: &LobbyAction) -> bool {
    action.kind == LobbyActionKind::OpenMenu
}

fn display_server_name(server: &LobbyServer) -> Option<&str> {
    let name = server.name.trim();
    (!name.is_empty()).then_some(name)
}

fn server_status_description(status: LobbyServerStatus, server: Option<&LobbyServer>) -> String {
    if status == LobbyServerStatus::Maintenance {
        if let Some(message) = server
            .map(|server| server.maintenance_message.trim())
            .filter(|message| !message.is_empty())
        {
            return message.to_string();
        }
    }
    status.description().to_string()
}

fn server_is_known_unavailable(status: &LobbyStatusSnapshot, server_id: &str) -> bool {
    matches!(
        status.status_for_server(server_id),
        LobbyServerStatus::Offline | LobbyServerStatus::Disabled | LobbyServerStatus::Maintenance
    )
}

fn boss_bar_color(color: LobbyBossBarColor) -> BossBarColor {
    match color {
        LobbyBossBarColor::Pink => BossBarColor::Pink,
        LobbyBossBarColor::Blue => BossBarColor::Blue,
        LobbyBossBarColor::Red => BossBarColor::Red,
        LobbyBossBarColor::Green => BossBarColor::Green,
        LobbyBossBarColor::Yellow => BossBarColor::Yellow,
        LobbyBossBarColor::Purple => BossBarColor::Purple,
        LobbyBossBarColor::White => BossBarColor::White,
    }
}

fn boss_bar_overlay(overlay: LobbyBossBarOverlay) -> BossBarOverlay {
    match overlay {
        LobbyBossBarOverlay::Progress => BossBarOverlay::Progress,
        LobbyBossBarOverlay::Notched6 => BossBarOverlay::Notched6,
        LobbyBossBarOverlay::Notched10 => BossBarOverlay::Notched10,
        LobbyBossBarOverlay::Notched12 => BossBarOverlay::Notched12,
        LobbyBossBarOverlay::Notched20 => BossBarOverlay::Notched20,
    }
}

async fn server_is_reachable(server: &LobbyServer, timeout: Duration) -> bool {
    let address = format!("{}:{}", server.host.trim(), server.port);
    let connect = tokio::net::TcpStream::connect(address);
    matches!(tokio::time::timeout(timeout, connect).await, Ok(Ok(_)))
}

async fn send_unavailable_server_message<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    server: &LobbyServer,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let message = if server.maintenance && !server.maintenance_message.trim().is_empty() {
        server.maintenance_message.trim().to_string()
    } else {
        format!("Server is unavailable: {}", server.id)
    };
    sink.send(SystemChat {
        content: text_component(message),
        overlay: false,
    })
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use qexed_config::app::qexed::server::{
        LobbyAction, LobbyActionKind, LobbyHealthCheck, LobbyMenuItem, LobbyNavigator, LobbyServer,
    };

    #[test]
    fn navigator_item_overwrites_configured_hotbar_slot() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            navigator: LobbyNavigator {
                slot: 2,
                item: "minecraft:compass".to_string(),
                name: "Selector".to_string(),
                ..LobbyNavigator::default()
            },
            ..Default::default()
        });
        let mut inventory = crate::inventory::PlayerInventory::empty();

        let changes = super::sync_navigator_item(&mut inventory, &lobby);

        assert_eq!(changes.len(), 1);
        assert_eq!(inventory.held_item().item_count.0, 0);
        assert!(matches!(
            &changes[0],
            crate::inventory::InventorySlotChange::Hotbar { slot: 2, item }
                if item.item_count.0 > 0
        ));
    }

    #[test]
    fn navigator_use_requires_configured_item_stack() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            navigator: LobbyNavigator {
                slot: 2,
                item: "minecraft:compass".to_string(),
                name: "Selector".to_string(),
                ..LobbyNavigator::default()
            },
            ..Default::default()
        });
        let compass_id = crate::inventory::item_id_for_name("minecraft:compass").unwrap();
        let dirt_id = crate::inventory::item_id_for_name("minecraft:dirt").unwrap();
        let plain_compass = crate::inventory::simple_item(compass_id, 1);
        let dirt = crate::inventory::simple_item(dirt_id, 1);

        assert!(!lobby.navigator_item_matches(2, &plain_compass));
        assert!(!lobby.navigator_item_matches(2, &dirt));
        assert!(lobby.navigator_item_matches(2, &lobby.navigator_item().unwrap()));
    }

    #[test]
    fn menu_items_are_looked_up_by_slot() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            menu_items: vec![LobbyMenuItem {
                slot: 13,
                name: "Survival".to_string(),
                action: LobbyAction {
                    kind: LobbyActionKind::Transfer,
                    target: "survival".to_string(),
                    message: String::new(),
                },
                ..LobbyMenuItem::default()
            }],
            ..Default::default()
        });

        assert_eq!(
            lobby.menu_item_for_slot(13).unwrap().action.kind,
            LobbyActionKind::Transfer
        );
        assert!(lobby.menu_item_for_slot(14).is_none());
    }

    #[test]
    fn only_open_menu_actions_mark_lobby_menu_open() {
        assert!(super::action_opens_menu(&LobbyAction {
            kind: LobbyActionKind::OpenMenu,
            target: String::new(),
            message: String::new(),
        }));
        assert!(!super::action_opens_menu(&LobbyAction {
            kind: LobbyActionKind::Message,
            target: String::new(),
            message: String::new(),
        }));
        assert!(!super::action_opens_menu(&LobbyAction {
            kind: LobbyActionKind::Transfer,
            target: "survival".to_string(),
            message: String::new(),
        }));
    }

    #[test]
    fn server_labels_include_cached_status() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            servers: vec![LobbyServer {
                id: "survival".to_string(),
                name: "Survival".to_string(),
                host: "127.0.0.1".to_string(),
                port: 25566,
                ..LobbyServer::default()
            }],
            ..Default::default()
        });
        let mut servers = HashMap::new();
        servers.insert("survival".to_string(), super::LobbyServerStatus::Online);
        let status = super::LobbyStatusSnapshot { servers };

        assert_eq!(lobby.server_labels(&status), vec!["Survival [online]"]);
        assert_eq!(
            lobby.server_command_entries(&status),
            vec!["survival=Survival [online]"]
        );
        assert_eq!(lobby.status_summary(&status), "Lobby servers: 1/1 online");
    }

    #[test]
    fn server_targets_resolve_by_id_case_and_display_name() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            servers: vec![LobbyServer {
                id: "survival".to_string(),
                name: "Survival Games".to_string(),
                host: "127.0.0.1".to_string(),
                port: 25566,
                ..LobbyServer::default()
            }],
            ..Default::default()
        });

        assert_eq!(
            lobby.resolve_server_id("survival").as_deref(),
            Some("survival")
        );
        assert_eq!(
            lobby.resolve_server_id("SURVIVAL").as_deref(),
            Some("survival")
        );
        assert_eq!(
            lobby.resolve_server_id("survival games").as_deref(),
            Some("survival")
        );
        assert_eq!(lobby.resolve_server_id("missing"), None);
    }

    #[test]
    fn broadcast_messages_render_server_status_placeholders() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            servers: vec![LobbyServer {
                id: "survival".to_string(),
                name: "Survival".to_string(),
                host: "127.0.0.1".to_string(),
                port: 25566,
                ..LobbyServer::default()
            }],
            broadcast: qexed_config::app::qexed::server::LobbyBroadcast {
                enable: true,
                interval_secs: 30,
                messages: vec!["{online_servers}/{total_servers}: {servers}".to_string()],
            },
            ..Default::default()
        });
        let mut servers = HashMap::new();
        servers.insert("survival".to_string(), super::LobbyServerStatus::Online);
        let status = super::LobbyStatusSnapshot { servers };

        assert_eq!(
            lobby.broadcast_message(0, &status).unwrap(),
            "1/1: Survival [online]"
        );
    }

    #[test]
    fn offline_transfer_menu_item_uses_barrier() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            servers: vec![LobbyServer {
                id: "survival".to_string(),
                name: "Survival".to_string(),
                host: "127.0.0.1".to_string(),
                port: 25566,
                ..LobbyServer::default()
            }],
            menu_items: vec![LobbyMenuItem {
                slot: 13,
                item: "minecraft:diamond".to_string(),
                name: "Survival".to_string(),
                action: LobbyAction {
                    kind: LobbyActionKind::Transfer,
                    target: "survival".to_string(),
                    message: String::new(),
                },
                ..LobbyMenuItem::default()
            }],
            ..Default::default()
        });
        let mut servers = HashMap::new();
        servers.insert("survival".to_string(), super::LobbyServerStatus::Offline);
        let status = super::LobbyStatusSnapshot { servers };

        let slots = lobby.menu_slots(3, &status);
        let barrier_id = crate::inventory::item_id_for_name("minecraft:barrier").unwrap();
        assert_eq!(slots[13].item_id.as_ref().unwrap().0, barrier_id);
    }

    #[test]
    fn transfer_menu_items_use_configured_status_icons() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            servers: vec![
                LobbyServer {
                    id: "offline".to_string(),
                    ..LobbyServer::default()
                },
                LobbyServer {
                    id: "disabled".to_string(),
                    ..LobbyServer::default()
                },
                LobbyServer {
                    id: "maintenance".to_string(),
                    maintenance_message: "Restarting".to_string(),
                    ..LobbyServer::default()
                },
            ],
            menu_items: vec![
                LobbyMenuItem {
                    slot: 10,
                    offline_item: "minecraft:red_wool".to_string(),
                    action: LobbyAction {
                        kind: LobbyActionKind::Transfer,
                        target: "offline".to_string(),
                        message: String::new(),
                    },
                    ..LobbyMenuItem::default()
                },
                LobbyMenuItem {
                    slot: 11,
                    disabled_item: "minecraft:gray_wool".to_string(),
                    action: LobbyAction {
                        kind: LobbyActionKind::Transfer,
                        target: "disabled".to_string(),
                        message: String::new(),
                    },
                    ..LobbyMenuItem::default()
                },
                LobbyMenuItem {
                    slot: 12,
                    maintenance_item: "minecraft:orange_wool".to_string(),
                    action: LobbyAction {
                        kind: LobbyActionKind::Transfer,
                        target: "maintenance".to_string(),
                        message: String::new(),
                    },
                    ..LobbyMenuItem::default()
                },
                LobbyMenuItem {
                    slot: 13,
                    unknown_item: "minecraft:clock".to_string(),
                    action: LobbyAction {
                        kind: LobbyActionKind::Transfer,
                        target: "checking".to_string(),
                        message: String::new(),
                    },
                    ..LobbyMenuItem::default()
                },
            ],
            ..Default::default()
        });
        let mut servers = HashMap::new();
        servers.insert("offline".to_string(), super::LobbyServerStatus::Offline);
        servers.insert("disabled".to_string(), super::LobbyServerStatus::Disabled);
        servers.insert(
            "maintenance".to_string(),
            super::LobbyServerStatus::Maintenance,
        );
        let status = super::LobbyStatusSnapshot { servers };

        let slots = lobby.menu_slots(3, &status);
        assert_eq!(
            slots[10].item_id.as_ref().unwrap().0,
            crate::inventory::item_id_for_name("minecraft:red_wool").unwrap()
        );
        assert_eq!(
            slots[11].item_id.as_ref().unwrap().0,
            crate::inventory::item_id_for_name("minecraft:gray_wool").unwrap()
        );
        assert_eq!(
            slots[12].item_id.as_ref().unwrap().0,
            crate::inventory::item_id_for_name("minecraft:orange_wool").unwrap()
        );
        assert_eq!(
            slots[13].item_id.as_ref().unwrap().0,
            crate::inventory::item_id_for_name("minecraft:clock").unwrap()
        );
    }

    #[test]
    fn status_snapshot_reports_changed_servers() {
        let mut previous = HashMap::new();
        previous.insert("survival".to_string(), super::LobbyServerStatus::Offline);
        let previous = super::LobbyStatusSnapshot { servers: previous };

        let mut current = HashMap::new();
        current.insert("survival".to_string(), super::LobbyServerStatus::Online);
        current.insert("minigames".to_string(), super::LobbyServerStatus::Offline);
        let current = super::LobbyStatusSnapshot { servers: current };

        assert_eq!(
            current.changed_servers_since(&previous),
            vec![
                ("minigames".to_string(), super::LobbyServerStatus::Offline),
                ("survival".to_string(), super::LobbyServerStatus::Online),
            ]
        );
    }

    #[test]
    fn known_offline_status_blocks_transfer_probe() {
        let mut servers = HashMap::new();
        servers.insert("survival".to_string(), super::LobbyServerStatus::Offline);
        let status = super::LobbyStatusSnapshot { servers };

        assert!(super::server_is_known_unavailable(&status, "survival"));
        assert!(!super::server_is_known_unavailable(&status, "unknown"));
    }

    #[test]
    fn maintenance_and_disabled_servers_are_unavailable() {
        let mut servers = HashMap::new();
        servers.insert(
            "maintenance".to_string(),
            super::LobbyServerStatus::Maintenance,
        );
        servers.insert("disabled".to_string(), super::LobbyServerStatus::Disabled);
        let status = super::LobbyStatusSnapshot { servers };

        assert_eq!(status.label_for_server("maintenance"), "[maintenance]");
        assert_eq!(status.label_for_server("disabled"), "[disabled]");
        assert!(super::server_is_known_unavailable(&status, "maintenance"));
        assert!(super::server_is_known_unavailable(&status, "disabled"));
    }

    #[test]
    fn menu_lore_renders_server_status_placeholders() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            servers: vec![LobbyServer {
                id: "survival".to_string(),
                name: "Survival".to_string(),
                maintenance: true,
                maintenance_message: "Restarting".to_string(),
                ..LobbyServer::default()
            }],
            ..Default::default()
        });
        let item = LobbyMenuItem {
            lore: vec![
                "{server_name} {status_label}".to_string(),
                "{status_description}".to_string(),
            ],
            ..LobbyMenuItem::default()
        };
        let mut servers = HashMap::new();
        servers.insert(
            "survival".to_string(),
            super::LobbyServerStatus::Maintenance,
        );
        let status = super::LobbyStatusSnapshot { servers };
        let server = lobby.servers.get("survival");

        assert_eq!(
            lobby.render_menu_lore(
                &item,
                server,
                super::LobbyServerStatus::Maintenance,
                &status
            ),
            vec!["Survival [maintenance]", "Restarting"]
        );
    }

    #[test]
    fn health_check_settings_are_clamped() {
        let lobby = super::LobbyRuntime::new(&qexed_config::app::qexed::server::Lobby {
            enable: true,
            health_check: LobbyHealthCheck {
                interval_secs: 0,
                timeout_ms: 10_000,
            },
            servers: vec![LobbyServer {
                id: "survival".to_string(),
                name: String::new(),
                host: "127.0.0.1".to_string(),
                port: 25566,
                ..LobbyServer::default()
            }],
            ..Default::default()
        });

        assert_eq!(
            lobby.status_refresh_interval().unwrap(),
            std::time::Duration::from_secs(1)
        );
        assert_eq!(
            lobby.health_check_timeout(),
            std::time::Duration::from_millis(5_000)
        );
    }

    #[test]
    fn boss_bar_style_config_maps_to_protocol_values() {
        assert_eq!(
            super::boss_bar_color(qexed_config::app::qexed::server::LobbyBossBarColor::Blue),
            qexed_protocol::to_client::play::boss_event::BossBarColor::Blue
        );
        assert_eq!(
            super::boss_bar_overlay(
                qexed_config::app::qexed::server::LobbyBossBarOverlay::Notched10
            ),
            qexed_protocol::to_client::play::boss_event::BossBarOverlay::Notched10
        );
    }
}
