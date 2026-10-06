//! 大厅运行时（v4 play/lobby.rs 迁移）。
//!
//! v6 适配：
//! - PacketSink 改用 qexed_connection::transport（v4 qexed_tcp_connect）
//! - 配置类型（Lobby/LobbyAction/...）改用 crate::config（v4 qexed_config 路径不存在）
//! - GameStateChange 等包名按 26.3 协议（boss_event 路径不变；container_set_slot
//!   结构体名在 v6 为 ContainerSetContent）
//! - 物品注册表（v4 crate::inventory）经 [`ItemRegistry`]（crate::drops）注入
//! - 代理转移动作（v4 super::chat::apply_proxy_connect_action）归 play-gameplay
//!   任务；这里收敛为 [`ProxyTransfer`] trait 回调，未注入时发送不可用提示
//! - PlayerInventory（导航物品同步）属 play-gameplay 任务；导航物品同步函数
//!   待其落地后接线（见 sync_navigator_item 的 TODO）

// play-gameplay 接线前部分 API 暂未被本 crate 引用（供其改造 runtime.rs 时使用）。
#![allow(dead_code)]


use std::{collections::HashMap, sync::Arc, time::Duration};

use qexed_connection::transport::PacketSink;
use qexed_packet::net_types::{RestBuffer, VarInt};
use qexed_protocol::{
    to_client::play::{
        boss_event::{BossBarColor, BossBarOverlay, BossEvent, BossEventOperation},
        container_set_content::ContainerSetContent,
        container_set_slot,
        open_screen::OpenScreen,
        system_chat::SystemChat,
    },
    to_server::play::container_click::ContainerClick,
    types::{ComponentsToAdd, Slot, minecraft},
};

use crate::config::{ForwardingMode, Lobby, LobbyAction, LobbyActionKind, LobbyBossBarColor, LobbyBossBarOverlay, LobbyMenuItem, ServerProxyConfig};
use crate::drops::ItemRegistry;
use crate::error::Result;
use crate::util::text_component;

const LOBBY_MENU_WINDOW_ID: i32 = 1;
const GENERIC_9X1_MENU_TYPE: i32 = 0;
const GENERIC_9X6_MENU_TYPE: i32 = 5;
const LOBBY_BOSS_BAR_UUID: uuid::Uuid = uuid::Uuid::from_u128(0x6c6f6262795f6261725f7165786564);
pub const MENU_WINDOW_ID: i32 = LOBBY_MENU_WINDOW_ID;

/// 代理转移动作回调（v4 super::chat::apply_proxy_connect_action 的收敛点）。
///
/// TODO(play-gameplay)：chat 模块迁移后由装配层提供实现（BungeeCord Connect
/// plugin message + Velocity transfer）。未注入时 transfer() 发送不可用提示。
pub trait ProxyTransfer: Send + Sync {
    /// 执行转移；返回 false 表示代理未启用（调用方发送降级消息）。
    fn apply(
        &self,
        server_id: &str,
        message: &str,
        actor: uuid::Uuid,
    ) -> std::result::Result<bool, String>;
}

/// 无代理实现：始终不可用。
#[derive(Debug, Default)]
pub struct NoProxyTransfer;

impl ProxyTransfer for NoProxyTransfer {
    fn apply(
        &self,
        _server_id: &str,
        _message: &str,
        _actor: uuid::Uuid,
    ) -> std::result::Result<bool, String> {
        Ok(false)
    }
}

#[derive(Debug, Clone)]
pub struct LobbyRuntime {
    config: Lobby,
}

/// 代理连接上下文（v4 ProxyConnectContext；plugins/players 域由装配层注入）。
pub struct ProxyConnectContext<'a> {
    pub server_config: &'a ServerProxyConfig,
    pub transfer: &'a dyn ProxyTransfer,
    pub actor: uuid::Uuid,
}

impl LobbyRuntime {
    pub fn new(config: &Lobby) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.enable
    }

    pub fn has_server(&self, server_id: &str) -> bool {
        self.resolve_server_id(server_id).is_some()
    }

    pub fn protect_world(&self) -> bool {
        self.enabled() && self.config.protect_world
    }

    pub fn navigator_slot(&self) -> Option<usize> {
        if !self.enabled() || !self.config.navigator.enable {
            return None;
        }
        Some(usize::from(self.config.navigator.slot.min(8)))
    }

    pub fn navigator_item(&self, items: &dyn ItemRegistry) -> Option<Slot> {
        if !self.enabled() || !self.config.navigator.enable {
            return None;
        }
        Some(named_item(
            items,
            &self.config.navigator.item,
            self.config.navigator.name.as_str(),
            1,
        ))
    }

    pub fn navigator_item_matches(
        &self,
        items: &dyn ItemRegistry,
        selected_slot: usize,
        actual: &Slot,
    ) -> bool {
        self.navigator_slot() == Some(selected_slot)
            && self
                .navigator_item(items)
                .is_some_and(|expected| actual.item_count.0 > 0 && actual == &expected)
    }

    pub fn broadcast_interval(&self) -> Option<Duration> {
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

    pub fn broadcast_message(
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

    pub fn boss_bar_enabled(&self) -> bool {
        self.enabled() && self.config.boss_bar.enable
    }

    pub async fn show_boss_bar<W>(
        &self,
        sink: &mut PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.boss_bar_enabled() {
            return Ok(());
        }
        sink.send(BossEvent {
            id: LOBBY_BOSS_BAR_UUID,
            operation: BossEventOperation::Add {
                name: AnyNbtFrom::from_text(text_component(self.boss_bar_title())),
                progress: 1.0,
                color: boss_bar_color(self.config.boss_bar.color),
                overlay: boss_bar_overlay(self.config.boss_bar.overlay),
                darken_screen: self.config.boss_bar.darken_screen,
                play_music: self.config.boss_bar.play_music,
                create_world_fog: self.config.boss_bar.create_world_fog,
            },
        })
        .await?;
        Ok(())
    }

    pub async fn update_boss_bar<W>(
        &self,
        sink: &mut PacketSink<W>,
        message: &str,
        progress: f32,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.boss_bar_enabled() {
            return Ok(());
        }
        sink.send(BossEvent {
            id: LOBBY_BOSS_BAR_UUID,
            operation: BossEventOperation::UpdateName {
                name: AnyNbtFrom::from_text(text_component(message.trim())),
            },
        })
        .await?;
        sink.send(BossEvent {
            id: LOBBY_BOSS_BAR_UUID,
            operation: BossEventOperation::UpdateProgress {
                progress: progress.clamp(0.0, 1.0),
            },
        })
        .await?;
        Ok(())
    }

    pub async fn update_boss_bar_status<W>(
        &self,
        sink: &mut PacketSink<W>,
        status: &LobbyStatusSnapshot,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let summary = self.status_summary(status);
        self.update_boss_bar(sink, &summary, status.online_ratio())
            .await
    }

    pub async fn remove_boss_bar<W>(
        &self,
        sink: &mut PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.boss_bar_enabled() {
            sink.send(BossEvent {
                id: LOBBY_BOSS_BAR_UUID,
                operation: BossEventOperation::Remove,
            })
            .await?;
        }
        Ok(())
    }

    pub fn status_refresh_interval(&self) -> Option<Duration> {
        None
    }

    pub async fn refresh_status(&self) -> LobbyStatusSnapshot {
        LobbyStatusSnapshot::from_servers(self.configured_transfer_targets())
    }

    /// BungeeCord GetServers 请求（代理模式下询问后端列表）。
    pub fn proxy_server_list_request(
        &self,
        server_config: &ServerProxyConfig,
    ) -> Option<qexed_protocol::to_client::play::custom_payload::CustomPayload> {
        if !self.enabled() || !proxy_backend_switching_enabled(server_config) {
            return None;
        }
        Some(
            qexed_protocol::to_client::play::custom_payload::CustomPayload {
                channel: "bungeecord:main".to_string(),
                data: RestBuffer(bungee_plugin_message(&["GetServers"])),
            },
        )
    }

    pub fn apply_proxy_server_list(
        &self,
        status: &mut LobbyStatusSnapshot,
        payload: &qexed_protocol::to_server::play::custom_payload::CustomPayload,
    ) -> bool {
        if !self.enabled() || payload.channel != "bungeecord:main" {
            return false;
        }
        let Some(servers) = parse_bungee_server_list_response(&payload.data.0) else {
            return false;
        };
        *status = LobbyStatusSnapshot::from_servers(
            servers
                .into_iter()
                .chain(self.configured_transfer_targets())
                .collect(),
        );
        true
    }

    pub fn status_summary(&self, status: &LobbyStatusSnapshot) -> String {
        let total = self.total_server_count(status);
        if total == 0 {
            return "Lobby servers: none".to_string();
        }
        let online = self.online_server_count(status);
        format!("Lobby servers: {online}/{total} online")
    }

    pub fn render_status_placeholders(
        &self,
        template: &str,
        status: &LobbyStatusSnapshot,
    ) -> String {
        template
            .replace(
                "{online_servers}",
                &self.online_server_count(status).to_string(),
            )
            .replace(
                "{total_servers}",
                &self.total_server_count(status).to_string(),
            )
            .replace("{servers}", &self.server_labels(status).join(", "))
    }

    pub async fn open_menu<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
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
            slot_data: self.menu_slots(items, rows, status),
            carried_item: items.empty_slot(),
        })
        .await?;
        Ok(())
    }

    pub async fn refresh_open_menu<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
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
            slot_data: self.menu_slots(items, rows, status),
            carried_item: items.empty_slot(),
        })
        .await?;
        Ok(())
    }

    pub async fn handle_use_item<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
        selected_slot: usize,
        held_item: &Slot,
        status: &LobbyStatusSnapshot,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.navigator_item_matches(items, selected_slot, held_item) {
            return Ok(false);
        }
        self.open_menu(sink, items, status).await?;
        Ok(true)
    }

    pub async fn handle_container_click<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
        click: ContainerClick,
        status: &LobbyStatusSnapshot,
        proxy_context: Option<&ProxyConnectContext<'_>>,
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
            slot_data: items.empty_slot(),
        })
        .await?;

        let Some(item) = self.menu_item_for_slot(click.slot) else {
            return Ok(true);
        };
        self.run_action(sink, items, &item.action, status, proxy_context)
            .await?;
        Ok(true)
    }

    async fn run_action<W>(
        &self,
        sink: &mut PacketSink<W>,
        items: &dyn ItemRegistry,
        action: &LobbyAction,
        status: &LobbyStatusSnapshot,
        proxy_context: Option<&ProxyConnectContext<'_>>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        match action.kind {
            LobbyActionKind::None => {}
            LobbyActionKind::OpenMenu => {
                self.open_menu(sink, items, status).await?;
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
                self.transfer(sink, action, proxy_context).await?;
            }
        }
        Ok(())
    }

    pub async fn transfer_to_server<W>(
        &self,
        sink: &mut PacketSink<W>,
        server_id: &str,
        status: &LobbyStatusSnapshot,
        proxy_context: Option<&ProxyConnectContext<'_>>,
    ) -> Result<bool>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.transfer_to_server_with_message(sink, server_id, "", status, proxy_context)
            .await
    }

    pub async fn transfer_to_server_with_message<W>(
        &self,
        sink: &mut PacketSink<W>,
        server_id: &str,
        message: &str,
        _status: &LobbyStatusSnapshot,
        proxy_context: Option<&ProxyConnectContext<'_>>,
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
        self.transfer(sink, &action, proxy_context).await?;
        Ok(true)
    }

    pub fn server_labels(&self, status: &LobbyStatusSnapshot) -> Vec<String> {
        let mut servers = self
            .server_targets_from_status(status)
            .into_iter()
            .map(|server_id| {
                let name = self
                    .display_name_for_target(&server_id)
                    .unwrap_or_else(|| server_id.clone());
                format!("{name} {}", status.label_for_server(&server_id))
            })
            .collect::<Vec<_>>();
        servers.sort();
        servers
    }

    pub fn server_command_entries(&self, status: &LobbyStatusSnapshot) -> Vec<String> {
        let mut servers = self
            .server_targets_from_status(status)
            .into_iter()
            .map(|id| {
                let name = self
                    .display_name_for_target(&id)
                    .unwrap_or_else(|| id.clone());
                let label = status.label_for_server(&id);
                if name == id {
                    format!("{id} {label}")
                } else {
                    format!("{id}={name} {label}")
                }
            })
            .collect::<Vec<_>>();
        servers.sort();
        servers
    }

    pub fn server_targets(&self) -> Vec<String> {
        self.configured_transfer_targets()
    }

    pub fn server_targets_from_status(&self, status: &LobbyStatusSnapshot) -> Vec<String> {
        let mut targets = self.configured_transfer_targets();
        targets.extend(status.server_ids());
        sort_dedup_case_insensitive(&mut targets);
        targets
    }

    pub fn resolve_server_id(&self, server_id: &str) -> Option<String> {
        let server_id = server_id.trim();
        if server_id.is_empty() {
            return None;
        }
        self.config
            .menu_items
            .iter()
            .filter(|item| item.action.kind == LobbyActionKind::Transfer)
            .filter_map(|item| {
                let id = item.action.target.trim();
                (!id.is_empty()).then_some((id, item.name.trim()))
            })
            .find_map(|(id, name)| {
                if id.eq_ignore_ascii_case(server_id)
                    || (!name.is_empty() && name.eq_ignore_ascii_case(server_id))
                {
                    Some(id.to_string())
                } else {
                    None
                }
            })
            .or_else(|| Some(server_id.to_string()))
    }

    pub fn total_server_count(&self, status: &LobbyStatusSnapshot) -> usize {
        self.server_targets_from_status(status).len()
    }

    pub fn online_server_count(&self, status: &LobbyStatusSnapshot) -> usize {
        self.server_targets_from_status(status)
            .iter()
            .filter(|server| status.status_for_server(server) == LobbyServerStatus::Online)
            .count()
    }

    async fn transfer<W>(
        &self,
        sink: &mut PacketSink<W>,
        action: &LobbyAction,
        proxy_context: Option<&ProxyConnectContext<'_>>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let server_id = action.target.trim();
        if server_id.is_empty() {
            sink.send(SystemChat {
                content: text_component(
                    qexed_language::t("qexed.play.lobby.server_unavailable")
                        .replace("%{target}", &action.target),
                ),
                overlay: false,
            })
            .await?;
            return Ok(());
        }

        let message = if !action.message.trim().is_empty() {
            action.message.trim().to_string()
        } else {
            let name = self
                .display_name_for_target(server_id)
                .unwrap_or_else(|| server_id.to_string());
            qexed_language::t("qexed.play.lobby.connecting").replace("%{name}", &name)
        };
        if let Some(proxy_context) = proxy_context {
            if proxy_backend_switching_enabled(proxy_context.server_config) {
                let applied = proxy_context
                    .transfer
                    .apply(server_id, &message, proxy_context.actor);
                match applied {
                    Ok(true) => return Ok(()),
                    Ok(false) => {}
                    Err(_) => {
                        // 代理侧失败：降级到不可用提示
                    }
                }
            }
        }
        sink.send(SystemChat {
            content: text_component(qexed_language::t("qexed.play.lobby.transfer_unavailable")),
            overlay: false,
        })
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

    fn menu_slots(
        &self,
        items: &dyn ItemRegistry,
        rows: u8,
        status: &LobbyStatusSnapshot,
    ) -> Vec<Slot> {
        let mut slots = vec![items.empty_slot(); usize::from(rows) * 9];
        for item in &self.config.menu_items {
            let slot = usize::from(item.slot);
            if slot >= slots.len() {
                continue;
            }
            slots[slot] = self.menu_slot_item(items, item, status);
        }
        slots
    }

    fn menu_slot_item(
        &self,
        items: &dyn ItemRegistry,
        item: &LobbyMenuItem,
        status: &LobbyStatusSnapshot,
    ) -> Slot {
        if item.action.kind != LobbyActionKind::Transfer {
            let name = self.render_status_placeholders(item.name.as_str(), status);
            let lore = if item.lore.is_empty() {
                Vec::new()
            } else {
                self.render_menu_lore(item, "", LobbyServerStatus::Unknown, status)
            };
            return named_item_with_lore(items, &item.item, name.as_str(), &lore, 1);
        }

        let server_id = item.action.target.trim();
        let server_status = status.status_for_server(server_id);
        let item_name =
            self.render_menu_item_text(item.name.as_str(), server_id, server_status, status);
        let display_name = format!("{} {}", item_name.trim(), server_status.label())
            .trim()
            .to_string();
        let item_name = menu_item_name_for_status(item, server_status);
        let lore = self.render_menu_lore(item, server_id, server_status, status);
        named_item_with_lore(items, item_name, display_name.as_str(), &lore, 1)
    }

    fn menu_item_for_slot(&self, slot: i16) -> Option<&LobbyMenuItem> {
        let slot = u8::try_from(slot).ok()?;
        self.config.menu_items.iter().find(|item| item.slot == slot)
    }

    fn render_menu_lore(
        &self,
        item: &LobbyMenuItem,
        server_id: &str,
        server_status: LobbyServerStatus,
        status: &LobbyStatusSnapshot,
    ) -> Vec<String> {
        if item.lore.is_empty() {
            return vec![server_status_description(server_status)];
        }
        item.lore
            .iter()
            .map(|line| self.render_menu_item_text(line, server_id, server_status, status))
            .collect()
    }

    fn render_menu_item_text(
        &self,
        template: &str,
        server_id: &str,
        server_status: LobbyServerStatus,
        status: &LobbyStatusSnapshot,
    ) -> String {
        let rendered = self.render_status_placeholders(template, status);
        let server_name = self
            .display_name_for_target(server_id)
            .unwrap_or_else(|| server_id.to_string());
        rendered
            .replace("{server}", server_id)
            .replace("{server_name}", &server_name)
            .replace("{status}", server_status.key())
            .replace("{status_label}", server_status.label())
            .replace(
                "{status_description}",
                &server_status_description(server_status),
            )
            .replace("{maintenance_message}", "")
    }

    fn configured_transfer_targets(&self) -> Vec<String> {
        let mut targets = self
            .config
            .menu_items
            .iter()
            .filter(|item| item.action.kind == LobbyActionKind::Transfer)
            .filter_map(|item| {
                let target = item.action.target.trim();
                (!target.is_empty()).then(|| target.to_string())
            })
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        targets
    }

    fn display_name_for_target(&self, target: &str) -> Option<String> {
        self.config
            .menu_items
            .iter()
            .filter(|item| item.action.kind == LobbyActionKind::Transfer)
            .find(|item| {
                item.action
                    .target
                    .trim()
                    .eq_ignore_ascii_case(target.trim())
            })
            .and_then(|item| {
                let name = item.name.trim();
                (!name.is_empty()).then(|| name.to_string())
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyServerStatus {
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
pub struct LobbyStatusSnapshot {
    servers: HashMap<String, LobbyServerStatus>,
}

impl LobbyStatusSnapshot {
    fn from_servers(servers: Vec<String>) -> Self {
        Self {
            servers: servers
                .into_iter()
                .filter_map(|server| {
                    let server = server.trim();
                    (!server.is_empty())
                        .then(|| (server.to_string(), LobbyServerStatus::Unknown))
                })
                .collect(),
        }
    }

    #[cfg(test)]
    pub fn from_servers_for_tests(servers: HashMap<String, LobbyServerStatus>) -> Self {
        Self { servers }
    }

    fn server_ids(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    pub fn changed_servers_since(
        &self,
        previous: &Self,
    ) -> Vec<(String, LobbyServerStatus)> {
        let mut changes = self
            .servers
            .iter()
            .filter_map(|(server, status)| {
                (previous.status_for_server(server) != *status)
                    .then(|| (server.clone(), *status))
            })
            .collect::<Vec<_>>();
        changes.sort_by(|left, right| left.0.cmp(&right.0));
        changes
    }

    pub fn status_for_server(&self, server_id: &str) -> LobbyServerStatus {
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

/// 导航物品写入快捷栏（v4 sync_navigator_item）。
///
/// TODO(play-gameplay)：PlayerInventory 与 InventorySlotChange 定义在 inventory
/// 域（play-gameplay 任务），落地后恢复该函数（inventory.set_hotbar_slot）。
/// 当前返回 None 表示无变更，调用方跳过同步。
pub fn sync_navigator_item(
    _inventory: &mut Option<qexed_player::StoredInventory>,
    _lobby: &LobbyRuntime,
) -> Option<()> {
    None
}

fn named_item(
    items: &dyn ItemRegistry,
    item_name: &str,
    name: &str,
    count: i32,
) -> Slot {
    named_item_with_lore(items, item_name, name, &Vec::new(), count)
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

fn sort_dedup_case_insensitive(values: &mut Vec<String>) {
    values.sort_by_key(|value| value.to_ascii_lowercase());
    values.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
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

fn proxy_backend_switching_enabled(server_config: &ServerProxyConfig) -> bool {
    server_config.proxy
        && matches!(
            server_config.proxy_protocol,
            ForwardingMode::Velocity | ForwardingMode::Victory | ForwardingMode::BungeeCord
        )
}

fn menu_type_for_rows(rows: u8) -> i32 {
    (GENERIC_9X1_MENU_TYPE + i32::from(rows.saturating_sub(1))).min(GENERIC_9X6_MENU_TYPE)
}

fn server_status_description(status: LobbyServerStatus) -> String {
    status.description().to_string()
}

fn bungee_plugin_message(values: &[&str]) -> Vec<u8> {
    let mut data = Vec::new();
    for value in values {
        write_modified_utf8(&mut data, value);
    }
    data
}

fn parse_bungee_server_list_response(data: &[u8]) -> Option<Vec<String>> {
    let mut offset = 0usize;
    let subchannel = read_modified_utf8(data, &mut offset)?;
    if subchannel != "GetServers" {
        return None;
    }
    let servers = read_modified_utf8(data, &mut offset)?;
    Some(
        servers
            .split(',')
            .map(str::trim)
            .filter(|server| !server.is_empty())
            .map(ToString::to_string)
            .collect(),
    )
}

fn read_modified_utf8(data: &[u8], offset: &mut usize) -> Option<String> {
    let len_end = offset.checked_add(2)?;
    let len = usize::from(u16::from_be_bytes(
        data.get(*offset..len_end)?.try_into().ok()?,
    ));
    let start = len_end;
    let end = start.checked_add(len)?;
    let value = std::str::from_utf8(data.get(start..end)?).ok()?.to_string();
    *offset = end;
    Some(value)
}

fn write_modified_utf8(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&bytes[..usize::from(len)]);
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

/// AnyNbt 便捷转换（BossEvent::Add.name 是 AnyNbt）。
struct AnyNbtFrom;

impl AnyNbtFrom {
    fn from_text(component: qexed_protocol::types::TextComponent) -> qexed_packet::net_types::AnyNbt {
        component
    }
}
