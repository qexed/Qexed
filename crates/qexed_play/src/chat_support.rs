//! 聊天命令支持面（v4 play/chat.rs 对 server 域 permissions/commands 的最小依赖，
//! play-gameplay 任务定义）。
//!
//! v4 的 chat.rs 直接使用 `qexed_server::permissions::PermissionManager` 与
//! `crate::commands::*` 帮助函数；v6 依赖方向禁止 play → server，因此：
//! - 权限查询收敛为 [`CommandPermissions`] trait（同步 bool，v4 为 async Result；
//!   server 域 PermissionManager 实现本 trait 后注入）。
//! - 命令名规范化/未知命令判定为纯函数，直接内联（v4 commands.rs 同名函数）。
//! - 命令帮助列表（visible_commands / localized_* 系列）依赖完整命令树与
//!   l10n，标注 TODO(hook)：由 server 域 commands 落地后经 trait 注入。

/// 命令权限查询面（v4 PermissionManager 的 chat 子集）。
pub trait CommandPermissions: Send + Sync {
    /// 玩家是否可执行命令（v4 can_run_command；v6 同步化，权限判定不落盘）。
    fn can_run_command(&self, profile: &qexed_packet::net_types::GameProfile, command: &str) -> bool;

    /// 无权限提示文案（v4 denied_message）。
    fn denied_message(&self) -> String {
        "You do not have permission to run this command.".to_string()
    }
}

/// 全放行权限（单测/默认）：等价 v4 默认权限表。
#[derive(Debug, Default)]
pub struct AllowAllCommandPermissions;

impl CommandPermissions for AllowAllCommandPermissions {
    fn can_run_command(
        &self,
        _profile: &qexed_packet::net_types::GameProfile,
        _command: &str,
    ) -> bool {
        true
    }
}

/// 命令名规范化（v4 commands::normalize_command_name）：去命名空间前缀 + 小写。
pub fn normalize_command_name(raw: &str) -> String {
    let name = raw.trim().to_ascii_lowercase();
    match name.rsplit_once(':') {
        Some((_, last)) if !last.is_empty() => last.to_string(),
        _ => name,
    }
}

/// v4 已知的原版命令表（unknown 命令提示用；帮助条目归 server 域）。
pub fn is_known_vanilla_command(name: &str) -> bool {
    matches!(
        name,
        "help" | "list" | "version" | "plugins" | "lobby" | "server" | "teleport" | "tp"
            | "gamemode" | "gm" | "give" | "time" | "gamerule" | "entity" | "npc"
            | "structure" | "kill" | "say" | "me" | "tell" | "msg" | "w" | "seed"
            | "difficulty" | "spawnpoint" | "setworldspawn" | "weather" | "xp" | "experience"
    )
}

/// 命令帮助条目（v4 commands::CommandHelpEntry）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHelpEntry {
    pub usage: String,
    pub description: Option<String>,
}

impl CommandHelpEntry {
    fn new(usage: impl Into<String>, description: Option<String>) -> Self {
        Self {
            usage: usage.into(),
            description,
        }
    }
}

/// 玩家可见的命令集合（v4 commands::visible_commands；v6 按 CommandPermissions 过滤）。
pub fn visible_commands(
    permissions: &dyn CommandPermissions,
    profile: &qexed_packet::net_types::GameProfile,
) -> Vec<&'static str> {
    const BUILTIN: &[&str] = &[
        "help", "list", "version", "plugins", "lobby", "server", "teleport", "gamemode", "give",
        "time", "gamerule", "entity", "npc", "structure",
    ];
    BUILTIN
        .iter()
        .copied()
        .filter(|command| permissions.can_run_command(profile, command))
        .collect()
}

/// /list 响应文案（v4 commands::localized_list；TODO(hook)：多语言模板由
/// qexed_language 按玩家 locale 提供，此处先英文原版格式）。
pub fn localized_list(locale: &str, online: usize, max: i32, names: &[String]) -> String {
    let _ = locale;
    let players = if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    };
    format!("There are {online} of a max of {max} players online: {players}")
}

/// 未实现命令提示（v4 commands::localized_unimplemented）。
pub fn localized_unimplemented(locale: &str, command: &str) -> String {
    let _ = locale;
    format!("Unknown or unimplemented command: {command}")
}

/// 内置命令帮助条目（v4 commands::localized_builtin_help_entries）。
pub fn localized_builtin_help_entries(commands: &[&str], locale: &str) -> Vec<CommandHelpEntry> {
    let _ = locale;
    commands
        .iter()
        .map(|command| CommandHelpEntry::new(format!("/{command}"), None))
        .collect()
}

/// 插件命令帮助条目（v4 commands::localized_plugin_help_entry；描述键经
/// qexed_language 按玩家 locale 翻译，TODO(hook)）。
pub fn localized_plugin_help_entry(
    command: &str,
    description_key: &str,
    locale: &str,
) -> CommandHelpEntry {
    let _ = locale;
    let description_key = description_key.trim();
    CommandHelpEntry::new(
        format!("/{command}"),
        (!description_key.is_empty()).then(|| description_key.to_string()),
    )
}

/// 服务器帮助总览（v4 commands::localized_server_help）。
pub fn localized_server_help(locale: &str, entries: &[CommandHelpEntry]) -> String {
    let _ = locale;
    let mut lines = vec!["--- Qexed commands ---".to_string()];
    lines.extend(
        entries
            .iter()
            .map(|entry| match &entry.description {
                Some(description) => format!("{} - {description}", entry.usage),
                None => entry.usage.clone(),
            }),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_namespace() {
        assert_eq!(normalize_command_name("qexed:Lobby"), "lobby");
        assert_eq!(normalize_command_name("Tp"), "tp");
        assert_eq!(normalize_command_name(" give "), "give");
    }

    #[test]
    fn known_commands_match_v4_list() {
        assert!(is_known_vanilla_command("help"));
        assert!(is_known_vanilla_command("structure"));
        assert!(!is_known_vanilla_command("definitely_not_a_command"));
    }
}
