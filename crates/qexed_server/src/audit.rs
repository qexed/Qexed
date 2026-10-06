//! 玩家审计日志（JSONL：方块放置/破坏、物品切换、命令执行）。
//! 迁移自 v4 crates/qexed/src/audit.rs；配置换成 crate::config::PlayerAuditConfig。
//!
//! v4 的 mongodb/mysql 存储（PlayerAuditStorage 之外的设想）在 v6 无对应依赖，
//! 文件 + stdout 已覆盖原实现全部存储模式。

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;

use qexed_packet::net_types::{GameProfile, Position};

use crate::config::{PlayerAuditConfig, PlayerAuditStorage};

#[derive(Debug)]
pub struct PlayerAuditLogger {
    enabled: bool,
    storage: PlayerAuditStorage,
    sink: Option<Mutex<File>>,
    track_block_place: bool,
    track_block_break: bool,
    track_item_switch: bool,
    track_command: bool,
}

impl PlayerAuditLogger {
    pub fn from_config(config: &PlayerAuditConfig) -> Self {
        if !config.enable {
            return Self::disabled();
        }

        let (use_file, _use_stdout) = match config.storage {
            PlayerAuditStorage::File => (true, false),
            PlayerAuditStorage::Stdout => (false, true),
            PlayerAuditStorage::FileAndStdout => (true, true),
        };

        let sink = if use_file {
            open_sink(&config.file_path)
        } else {
            None
        };

        Self {
            enabled: true,
            storage: config.storage,
            sink,
            track_block_place: config.events.block_place,
            track_block_break: config.events.block_break,
            track_item_switch: config.events.item_switch,
            track_command: config.events.command,
        }
    }

    pub fn log_block_place(
        &self,
        player: &GameProfile,
        dimension: &str,
        position: &Position,
        block_state: i32,
        held_item_id: Option<i32>,
    ) {
        if !self.enabled || !self.track_block_place {
            return;
        }
        self.write_record(
            "block_place",
            player,
            dimension,
            json!({
                "x": position.x,
                "y": position.y,
                "z": position.z,
                "block_state": block_state,
                "held_item_id": held_item_id,
            }),
        );
    }

    pub fn log_block_break(
        &self,
        player: &GameProfile,
        dimension: &str,
        position: &Position,
        block_state: i32,
        held_item_id: Option<i32>,
    ) {
        if !self.enabled || !self.track_block_break {
            return;
        }
        self.write_record(
            "block_break",
            player,
            dimension,
            json!({
                "x": position.x,
                "y": position.y,
                "z": position.z,
                "block_state": block_state,
                "held_item_id": held_item_id,
            }),
        );
    }

    pub fn log_item_switch(
        &self,
        player: &GameProfile,
        dimension: &str,
        slot: i16,
        held_item_id: Option<i32>,
        held_item_count: i32,
    ) {
        if !self.enabled || !self.track_item_switch {
            return;
        }
        self.write_record(
            "item_switch",
            player,
            dimension,
            json!({
                "slot": slot,
                "held_item_id": held_item_id,
                "held_item_count": held_item_count,
            }),
        );
    }

    pub fn log_command(&self, player: &GameProfile, dimension: &str, command: &str) {
        if !self.enabled || !self.track_command {
            return;
        }
        self.write_record(
            "command",
            player,
            dimension,
            json!({
                "command": command,
            }),
        );
    }

    fn write_record(
        &self,
        action: &str,
        player: &GameProfile,
        dimension: &str,
        details: serde_json::Value,
    ) {
        if !self.enabled {
            return;
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or_default();
        let line = serde_json::to_string(&json!({
            "ts_ms": now_ms,
            "action": action,
            "player_uuid": player.uuid.to_string(),
            "player_name": player.username,
            "dimension": dimension,
            "details": details,
        }))
        .unwrap_or_else(|_| "{\"action\":\"audit_encode_error\"}".to_string());

        if matches!(
            self.storage,
            PlayerAuditStorage::Stdout | PlayerAuditStorage::FileAndStdout
        ) {
            log::info!("[player_audit] {line}");
        }

        if let Some(sink) = &self.sink {
            let mut sink = sink.lock().expect("player audit sink poisoned");
            if let Err(err) = writeln!(sink, "{line}") {
                log::warn!("failed to write player audit log: {err}");
            }
        }
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            storage: PlayerAuditStorage::File,
            sink: None,
            track_block_place: false,
            track_block_break: false,
            track_item_switch: false,
            track_command: false,
        }
    }
}

fn open_sink(path: &str) -> Option<Mutex<File>> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            log::warn!(
                "failed to create player audit log directory: path={}, error={err}",
                parent.display()
            );
            return None;
        }
    }
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => Some(Mutex::new(file)),
        Err(err) => {
            log::warn!(
                "failed to open player audit log file: path={}, error={err}",
                path.display()
            );
            None
        }
    }
}
