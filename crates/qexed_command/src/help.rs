use crate::catalog::{builtin_command_literals, normalize_command_name};

const HELP_ENTRIES_PER_PAGE: usize = 8;

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

pub fn localized_builtin_help_entries(commands: &[&str], locale: &str) -> Vec<CommandHelpEntry> {
    let locale = i18n_locale(locale);
    let mut entries = commands
        .iter()
        .filter_map(|command| localized_builtin_help_entry(command, locale))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.usage.cmp(&right.usage));
    entries
}

pub fn localized_plugin_help_entry(
    command: &str,
    description_key: &str,
    locale: &str,
) -> CommandHelpEntry {
    let locale = i18n_locale(locale);
    let description_key = description_key.trim();
    let description = if description_key.is_empty() {
        None
    } else {
        Some(localized_static(description_key, locale).to_string())
    };
    CommandHelpEntry::new(format!("/{command}"), description)
}

pub fn localized_server_help(locale: &str, entries: &[CommandHelpEntry]) -> String {
    localized_server_help_page(locale, entries, 1)
}

pub fn localized_server_help_page(
    locale: &str,
    entries: &[CommandHelpEntry],
    page: usize,
) -> String {
    localized_help(
        locale,
        localized_static("qexed.command.help.header", locale),
        entries,
        page,
    )
}

pub fn localized_command_help(locale: &str, entries: &[CommandHelpEntry], command: &str) -> String {
    let command = normalize_command_name(command);
    let Some(entry) = entries
        .iter()
        .find(|entry| help_entry_matches_command(entry, &command))
    else {
        return format!(
            "{}\n未找到指令帮助：/{}",
            localized_static("qexed.command.help.header", locale),
            command
        );
    };

    format!(
        "{}\n{}",
        localized_static("qexed.command.help.header", locale),
        localized_help_line(entry)
    )
}

pub fn localized_console_help(locale: &str) -> String {
    let locale_key = i18n_locale(locale);
    let server_entries = localized_builtin_help_entries(builtin_command_literals(), locale_key);
    localized_help(
        locale_key,
        localized_static("qexed.command.help.console_header", locale_key),
        &server_entries,
        1,
    )
}

pub fn localized_list(_locale: &str, online: usize, max: i32, names: &[String]) -> String {
    let players = if names.is_empty() {
        localized_static("qexed.command.list.empty", "zh-CN").to_string()
    } else {
        names.join(", ")
    };
    format!(
        "当前有 {online}/{} 名玩家在线：{players}",
        localized_max_label("zh-CN", max)
    )
}

pub fn localized_unimplemented(_locale: &str, command: &str) -> String {
    format!("该指令已保留但尚未实现：/{command}")
}

pub fn localized_max_label(_locale: &str, max: i32) -> String {
    if max < 0 {
        localized_static("qexed.command.max_players.unlimited", "zh-CN").to_string()
    } else {
        max.to_string()
    }
}

pub fn i18n_locale(_locale: &str) -> &'static str {
    "zh-CN"
}

fn localized_help(
    _locale: &str,
    header: &str,
    entries: &[CommandHelpEntry],
    page: usize,
) -> String {
    if entries.is_empty() {
        let mut lines = vec![format!("{header} 第 1/1 页")];
        lines.push("没有可用指令。".to_string());
        return lines.join("\n");
    }

    let total_pages = entries.len().div_ceil(HELP_ENTRIES_PER_PAGE).max(1);
    let page = page.clamp(1, total_pages);
    let start = (page - 1) * HELP_ENTRIES_PER_PAGE;
    let end = (start + HELP_ENTRIES_PER_PAGE).min(entries.len());

    let mut lines = vec![format!("{header} 第 {page}/{total_pages} 页")];
    lines.extend(entries[start..end].iter().map(localized_help_line));
    lines.join("\n")
}

fn localized_help_line(entry: &CommandHelpEntry) -> String {
    match &entry.description {
        Some(description) => format!("{} - {description}", entry.usage),
        None => entry.usage.clone(),
    }
}

fn help_entry_matches_command(entry: &CommandHelpEntry, command: &str) -> bool {
    entry
        .usage
        .trim()
        .trim_start_matches('/')
        .split_ascii_whitespace()
        .next()
        .is_some_and(|usage_command| normalize_command_name(usage_command) == command)
}

fn localized_builtin_help_entry(command: &str, locale: &str) -> Option<CommandHelpEntry> {
    let (usage, description_key) = match command {
        "help" => ("/help", "qexed.command.help.description"),
        "list" => ("/list", "qexed.command.list.description"),
        "version" => ("/version", "qexed.command.version.description"),
        "plugins" => ("/plugins", "qexed.command.plugin.description"),
        "gamemode" => (
            "/gamemode <survival|creative|adventure|spectator> [player]",
            "qexed.command.gamemode.description",
        ),
        "give" => (
            "/give [player] <item> [count]",
            "qexed.command.give.description",
        ),
        "reload" => ("/reload", "qexed.command.reload.description"),
        "server" => ("/server [id]", "qexed.command.server.description"),
        "spawn" => ("/spawn", "qexed.command.spawn.description"),
        "teleport" => (
            "/teleport <target|location>",
            "qexed.command.teleport.description",
        ),
        "tp" => ("/tp <target|location>", "qexed.command.tp.description"),
        "time" => (
            "/time set|add|query <dimension> ...",
            "qexed.command.time.description",
        ),
        "gamerule" => (
            "/gamerule <dimension> <rule> [value]",
            "qexed.command.gamerule.description",
        ),
        "scoreboard" => ("/scoreboard ...", "qexed.command.scoreboard.description"),
        "entity" => (
            "/entity list|spawn|move|remove ...",
            "qexed.command.entity.description",
        ),
        "npc" => (
            "/npc list|spawn <id> [entity_type] [name...]|move|remove ...",
            "qexed.command.npc.description",
        ),
        "structure" => (
            "/structure list|place|locate ...",
            "qexed.command.structure.description",
        ),
        _ => return None,
    };
    Some(CommandHelpEntry::new(
        usage,
        Some(localized_static(description_key, locale).to_string()),
    ))
}

fn localized_static(key: &str, _locale: &str) -> &'static str {
    match key {
        "qexed.command.help.header" => "---- Qexed 服务器指令 ----",
        "qexed.command.help.console_header" => "---- Qexed 控制台指令 ----",
        "qexed.command.help.description" => "显示帮助。",
        "qexed.command.list.description" => "列出在线玩家。",
        "qexed.command.version.description" => "显示服务器版本。",
        "qexed.command.plugin.description" => "列出已加载插件。",
        "qexed.command.gamemode.description" => "切换游戏模式。",
        "qexed.command.give.description" => "给予物品。",
        "qexed.command.reload.description" => "重新加载运行时资源。",
        "qexed.command.server.description" => "列出或选择大厅服务器。",
        "qexed.command.spawn.description" => "传送到出生点。",
        "qexed.command.teleport.description" => "传送玩家或位置。",
        "qexed.command.tp.description" => "/teleport 的别名。",
        "qexed.command.time.description" => "查询或修改世界时间。",
        "qexed.command.gamerule.description" => "查询或修改游戏规则。",
        "qexed.command.scoreboard.description" => "管理计分板状态。",
        "qexed.command.entity.description" => "管理运行时实体。",
        "qexed.command.npc.description" => "管理 NPC 实体。",
        "qexed.command.structure.description" => "管理结构。",
        "qexed.command.max_players.unlimited" => "无限",
        "qexed.command.list.empty" => "无",
        _ => "",
    }
}
