pub fn visible_commands() -> Vec<&'static str> {
    let mut commands = builtin_command_literals().to_vec();
    commands.sort_unstable();
    commands.dedup();
    commands
}

pub fn is_known_vanilla_command(command: &str) -> bool {
    let command = command.trim().trim_start_matches('/').to_ascii_lowercase();
    if command.is_empty() {
        return false;
    }
    vanilla_command_literals()
        .iter()
        .any(|candidate| *candidate == command)
}

pub fn normalize_command_name(command: &str) -> String {
    match command
        .trim()
        .trim_start_matches('/')
        .to_ascii_lowercase()
        .as_str()
    {
        "tp" => "teleport".to_string(),
        "w" | "tell" => "msg".to_string(),
        "xp" => "experience".to_string(),
        name => name.to_string(),
    }
}

pub fn permission_node(command: &str) -> Option<String> {
    let name = command
        .trim()
        .trim_start_matches('/')
        .split_ascii_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = normalize_command_name(&name);
    (!name.is_empty()).then(|| format!("qexed.command.{name}"))
}

pub fn command_names_for_suggestions() -> Vec<&'static str> {
    let mut commands = builtin_command_literals().to_vec();
    commands.sort_unstable();
    commands.dedup();
    commands
}

pub fn builtin_command_literals() -> &'static [&'static str] {
    &[
        "help",
        "list",
        "version",
        "plugins",
        "gamemode",
        "give",
        "reload",
        "server",
        "spawn",
        "teleport",
        "tp",
        "time",
        "gamerule",
        "scoreboard",
        "entity",
        "npc",
        "structure",
    ]
}

fn vanilla_command_literals() -> &'static [&'static str] {
    // Source: Minecraft Java 1.21.4 command registry cross-checked against
    // https://mappings.dev/1.21.4/net/minecraft/server/commands/index.html
    // Snapshot date: 2026-05-28. Deprecated/testing/debug-only commands are excluded.
    &[
        "advancement",
        "attribute",
        "ban",
        "ban-ip",
        "banlist",
        "bossbar",
        "clear",
        "clone",
        "damage",
        "data",
        "datapack",
        "debug",
        "defaultgamemode",
        "deop",
        "difficulty",
        "effect",
        "enchant",
        "execute",
        "experience",
        "fill",
        "fillbiome",
        "forceload",
        "function",
        "gamemode",
        "gamerule",
        "give",
        "help",
        "item",
        "jfr",
        "kick",
        "kill",
        "list",
        "locate",
        "loot",
        "me",
        "msg",
        "op",
        "pardon",
        "pardon-ip",
        "particle",
        "perf",
        "place",
        "playsound",
        "publish",
        "random",
        "recipe",
        "reload",
        "return",
        "ride",
        "rotate",
        "save-all",
        "save-off",
        "save-on",
        "say",
        "schedule",
        "scoreboard",
        "seed",
        "setblock",
        "setidletimeout",
        "setworldspawn",
        "spawnpoint",
        "spectate",
        "spreadplayers",
        "stop",
        "stopsound",
        "summon",
        "tag",
        "team",
        "teammsg",
        "teleport",
        "tell",
        "tellraw",
        "tick",
        "time",
        "title",
        "tm",
        "tp",
        "transfer",
        "trigger",
        "w",
        "weather",
        "whitelist",
        "worldborder",
    ]
}
