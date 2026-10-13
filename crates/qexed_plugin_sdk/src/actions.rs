//! [`PlayerAction`] 便捷构造器：v4 action payload 的浅包装。
//!
//! v4 SDK 里动作是 serde 结构体字面量；v6 [`qexed_plugin_api::payload::PlayerAction`]
//! 是 enum，这里给常用动作起名，减少字面量噪音。

use qexed_plugin_api::payload::PlayerAction;

/// 系统聊天消息（无 translate，纯文本）。
pub fn system_message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: false,
    }
}

/// ActionBar 覆盖消息。
pub fn overlay_message(text: impl Into<String>) -> PlayerAction {
    PlayerAction::SystemMessage {
        text: text.into(),
        translate: String::new(),
        with: Vec::new(),
        overlay: true,
    }
}

/// 传送到维度坐标（角度可选）。
#[allow(clippy::too_many_arguments)]
pub fn teleport(dimension: impl Into<String>, x: f64, y: f64, z: f64, yaw: Option<f32>, pitch: Option<f32>) -> PlayerAction {
    PlayerAction::Teleport { dimension: dimension.into(), x, y, z, yaw, pitch }
}

/// 转移到另一台服务器。
pub fn transfer(host: impl Into<String>, port: u16, message: impl Into<String>) -> PlayerAction {
    PlayerAction::Transfer { host: host.into(), port, message: message.into() }
}

/// 打开菜单。
pub fn open_menu(menu: impl Into<String>) -> PlayerAction {
    PlayerAction::OpenMenu { menu: menu.into() }
}

/// 给物品（count=1，无 NBT 装饰）。
pub fn give_item(item: impl Into<String>) -> PlayerAction {
    PlayerAction::GiveItem {
        item: item.into(),
        count: 1,
        name: String::new(),
        lore: Vec::new(),
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
    }
}

/// 给物品（显式数量）。
pub fn give_item_with_count(item: impl Into<String>, count: i32) -> PlayerAction {
    PlayerAction::GiveItem {
        item: item.into(),
        count,
        name: String::new(),
        lore: Vec::new(),
        enchantments: Vec::new(),
        plugin_enchantments: Vec::new(),
    }
}

/// 重置背包。
pub fn reset_inventory(restore_menu_items: bool) -> PlayerAction {
    PlayerAction::ResetInventory { restore_menu_items }
}

/// 设置其他玩家对目标可见性。
pub fn set_players_visible(visible: bool) -> PlayerAction {
    PlayerAction::SetPlayersVisible { visible }
}

/// 施加速度向量。
pub fn velocity(x: f64, y: f64, z: f64, additive: bool) -> PlayerAction {
    PlayerAction::Velocity { x, y, z, additive }
}

/// 对玩家造成伤害。
#[allow(clippy::too_many_arguments)]
pub fn damage_player(username: impl Into<String>, amount: f32, kind: impl Into<String>) -> PlayerAction {
    PlayerAction::DamagePlayer {
        uuid: String::new(),
        username: username.into(),
        amount,
        kind: kind.into(),
        source_entity_id: 0,
        source_x: 0.0,
        source_y: 0.0,
        source_z: 0.0,
        knockback: 0.0,
    }
}

/// boss bar 显示。
#[allow(clippy::too_many_arguments)]
pub fn boss_bar(id: impl Into<String>, title: impl Into<String>, progress: f32, color: impl Into<String>) -> PlayerAction {
    PlayerAction::BossBar {
        id: id.into(),
        title: title.into(),
        progress,
        color: color.into(),
        overlay: String::new(),
    }
}

/// 移除 boss bar。
pub fn remove_boss_bar(id: impl Into<String>) -> PlayerAction {
    PlayerAction::RemoveBossBar { id: id.into() }
}

/// 给基岩玩家发送表单（json 序列化后的表单体）。
pub fn bedrock_form(plugin_form_id: impl Into<String>, form_type: impl Into<String>, json: impl Into<String>) -> PlayerAction {
    PlayerAction::SendBedrockForm {
        uuid: String::new(),
        username: String::new(),
        form_id: None,
        plugin_form_id: plugin_form_id.into(),
        form_type: form_type.into(),
        json: json.into(),
    }
}

/// 生成弹射物（常用默认：无重力命中半径 1，5 秒寿命）。
#[allow(clippy::too_many_arguments)]
pub fn spawn_projectile(
    kind: impl Into<String>,
    dimension: impl Into<String>,
    x: f64, y: f64, z: f64,
    vx: f64, vy: f64, vz: f64,
    source_entity_id: i32,
    damage: f32,
) -> PlayerAction {
    PlayerAction::SpawnProjectile {
        kind: kind.into(),
        configured_event: String::new(),
        tag: String::new(),
        dimension: dimension.into(),
        x, y, z,
        velocity_x: vx,
        velocity_y: vy,
        velocity_z: vz,
        source_entity_id,
        damage,
        knockback: 0.0,
        gravity_per_tick: 0.05,
        hit_radius: 1.0,
        lifetime_ticks: 100,
    }
}
