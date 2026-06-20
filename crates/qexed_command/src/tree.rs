use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::commands::{
    BrigadierString, Commands, MinecraftTime, Node, Varies,
};

use crate::catalog::builtin_command_literals;

pub fn command_tree() -> Commands {
    command_tree_for(builtin_command_literals())
}

pub fn command_tree_for(commands: &[&str]) -> Commands {
    command_tree_for_servers(commands, &[])
}

pub fn command_tree_for_servers(commands: &[&str], server_ids: &[String]) -> Commands {
    command_tree_for_servers_with_extra(commands, server_ids, &[])
}

pub fn command_tree_for_servers_with_extra(
    commands: &[&str],
    server_ids: &[String],
    extra_literals: &[String],
) -> Commands {
    let server_ids = server_command_literals(server_ids);
    let extra_literals = server_command_literals(extra_literals);
    let mut nodes = vec![Node {
        flags: 0x00,
        children: Vec::new(),
        ..Node::default()
    }];
    for command in commands {
        let index = append_command_nodes(&mut nodes, command, &server_ids);
        nodes[0].children.push(VarInt(index));
    }
    for literal in extra_literals {
        if commands.contains(&literal.as_str()) {
            continue;
        }
        let index = nodes.len() as i32;
        nodes.push(literal_with_optional_greedy_argument(
            &literal,
            (nodes.len() + 1) as i32,
        ));
        nodes.push(greedy_string_argument("args"));
        nodes[0].children.push(VarInt(index));
    }

    Commands {
        nodes,
        root_index: VarInt(0),
    }
}

fn append_command_nodes(nodes: &mut Vec<Node>, command: &str, server_ids: &[String]) -> i32 {
    match command {
        "help" => append_help_command_nodes(nodes),
        "list" | "version" | "plugins" | "reload" | "spawn" => {
            append_simple_command_nodes(nodes, command)
        }
        "server" => append_server_command_nodes(nodes, server_ids),
        "gamemode" => append_gamemode_command_nodes(nodes),
        "give" => append_give_command_nodes(nodes),
        "teleport" => append_teleport_command_nodes(nodes),
        "tp" => append_teleport_alias_command_nodes(nodes),
        "time" => append_time_command_nodes(nodes),
        "gamerule" => append_gamerule_command_nodes(nodes),
        "scoreboard" => append_scoreboard_command_nodes(nodes),
        "entity" => append_entity_command_nodes(nodes),
        "npc" => append_npc_command_nodes(nodes),
        "structure" => append_structure_command_nodes(nodes),
        _ => append_fallback_command_nodes(nodes, command),
    }
}

pub fn command_tree_for_lobby(commands: &[&str], server_ids: &[String]) -> Commands {
    command_tree_for_servers(commands, server_ids)
}

pub fn command_tree_for_lobby_with_extra(
    commands: &[&str],
    server_ids: &[String],
    extra_literals: &[String],
) -> Commands {
    command_tree_for_servers_with_extra(commands, server_ids, extra_literals)
}

fn server_command_literals(server_ids: &[String]) -> Vec<String> {
    let mut ids = server_ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty() && !id.chars().any(char::is_whitespace))
        .map(str::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn executable_literal(name: &str) -> Node {
    literal_node(name, true)
}

fn literal_node(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x01 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        ..Node::default()
    }
}

fn literal_with_optional_greedy_argument(name: &str, argument_index: i32) -> Node {
    Node {
        flags: 0x01 | 0x04,
        children: vec![VarInt(argument_index)],
        name: Some(name.to_string()),
        ..Node::default()
    }
}

fn server_literal(argument_index: i32, literal_start_index: i32, literal_count: usize) -> Node {
    let mut children = vec![VarInt(argument_index)];
    children.extend(
        (0..literal_count)
            .map(|offset| VarInt(literal_start_index + i32::try_from(offset).unwrap_or(0))),
    );
    Node {
        flags: 0x01 | 0x04,
        children,
        name: Some("server".to_string()),
        ..Node::default()
    }
}

fn append_simple_command_nodes(nodes: &mut Vec<Node>, name: &str) -> i32 {
    let index = nodes.len() as i32;
    nodes.push(executable_literal(name));
    index
}

fn append_fallback_command_nodes(nodes: &mut Vec<Node>, name: &str) -> i32 {
    let index = nodes.len() as i32;
    nodes.push(literal_with_optional_greedy_argument(
        name,
        (nodes.len() + 1) as i32,
    ));
    nodes.push(greedy_string_argument("arguments"));
    index
}

fn append_server_command_nodes(nodes: &mut Vec<Node>, server_ids: &[String]) -> i32 {
    let index = nodes.len() as i32;
    nodes.push(server_literal(index + 1, index + 2, server_ids.len()));
    nodes.push(ask_server_word_argument("target", true));
    for server_id in server_ids {
        nodes.push(executable_literal(server_id));
    }
    index
}

fn append_help_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(literal_node("help", true));

    let page = nodes.len() as i32;
    nodes.push(integer_argument("page", true));
    let command = nodes.len() as i32;
    nodes.push(ask_server_word_argument("command", true));

    nodes[root as usize].children = vec![VarInt(page), VarInt(command)];
    root
}

fn append_gamemode_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(literal_node("gamemode", false));

    let modes = ["survival", "creative", "adventure", "spectator"];
    let mut mode_indexes = Vec::with_capacity(modes.len());
    for mode in modes {
        let mode_index = nodes.len() as i32;
        nodes.push(literal_node(mode, true));
        mode_indexes.push(mode_index);
    }

    let player = nodes.len() as i32;
    nodes.push(game_profile_argument("player", true));

    nodes[root as usize].children = mode_indexes.iter().copied().map(VarInt).collect();
    for mode_index in mode_indexes {
        nodes[mode_index as usize].children = vec![VarInt(player)];
    }

    root
}

fn append_give_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(literal_node("give", false));

    let item = nodes.len() as i32;
    nodes.push(item_stack_argument("item", true));
    let count = nodes.len() as i32;
    nodes.push(integer_argument("count", true));

    let player = nodes.len() as i32;
    nodes.push(game_profile_argument("player", false));
    let player_item = nodes.len() as i32;
    nodes.push(item_stack_argument("item", true));
    let player_count = nodes.len() as i32;
    nodes.push(integer_argument("count", true));

    nodes[root as usize].children = vec![VarInt(item), VarInt(player)];
    nodes[item as usize].children = vec![VarInt(count)];
    nodes[player as usize].children = vec![VarInt(player_item)];
    nodes[player_item as usize].children = vec![VarInt(player_count)];

    root
}

fn append_time_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(literal_node("time", false));

    let set = nodes.len() as i32;
    nodes.push(literal_node("set", false));
    let add = nodes.len() as i32;
    nodes.push(literal_node("add", false));
    let query = nodes.len() as i32;
    nodes.push(literal_node("query", false));

    let set_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));
    let set_value = nodes.len() as i32;
    nodes.push(time_argument("value", true));

    let add_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));
    let add_value = nodes.len() as i32;
    nodes.push(time_argument("value", true));

    let query_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", true));

    nodes[root as usize].children = vec![VarInt(set), VarInt(add), VarInt(query)];
    nodes[set as usize].children = vec![VarInt(set_dimension)];
    nodes[set_dimension as usize].children = vec![VarInt(set_value)];
    nodes[add as usize].children = vec![VarInt(add_dimension)];
    nodes[add_dimension as usize].children = vec![VarInt(add_value)];
    nodes[query as usize].children = vec![VarInt(query_dimension)];

    root
}

fn append_gamerule_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(literal_node("gamerule", false));

    let dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));

    let do_daylight_cycle = append_bool_gamerule(nodes, "doDaylightCycle");
    let do_block_updates = append_bool_gamerule(nodes, "doBlockUpdates");
    let do_world_read_only = append_bool_gamerule(nodes, "doWorldReadOnly");
    let time_tick_step = append_long_gamerule(nodes, "timeTickStep");
    let fixed_time = append_fixed_time_gamerule(nodes);
    let world_light = append_world_light_gamerule(nodes);

    nodes[root as usize].children = vec![VarInt(dimension)];
    nodes[dimension as usize].children = vec![
        VarInt(do_daylight_cycle),
        VarInt(do_block_updates),
        VarInt(do_world_read_only),
        VarInt(time_tick_step),
        VarInt(fixed_time),
        VarInt(world_light),
    ];

    root
}

fn append_bool_gamerule(nodes: &mut Vec<Node>, name: &str) -> i32 {
    let rule = nodes.len() as i32;
    nodes.push(literal_node(name, true));
    let value = nodes.len() as i32;
    nodes.push(boolean_argument("value", true));
    nodes[rule as usize].children = vec![VarInt(value)];
    rule
}

fn append_long_gamerule(nodes: &mut Vec<Node>, name: &str) -> i32 {
    let rule = nodes.len() as i32;
    nodes.push(literal_node(name, true));
    let value = nodes.len() as i32;
    nodes.push(long_argument("value", true));
    nodes[rule as usize].children = vec![VarInt(value)];
    rule
}

fn append_fixed_time_gamerule(nodes: &mut Vec<Node>) -> i32 {
    let rule = nodes.len() as i32;
    nodes.push(literal_node("fixedTime", true));
    let value = nodes.len() as i32;
    nodes.push(long_argument("value", true));
    let none = nodes.len() as i32;
    nodes.push(executable_literal("none"));
    nodes[rule as usize].children = vec![VarInt(value), VarInt(none)];
    rule
}

fn append_world_light_gamerule(nodes: &mut Vec<Node>) -> i32 {
    let rule = nodes.len() as i32;
    nodes.push(literal_node("worldLight", true));
    let static_light = nodes.len() as i32;
    nodes.push(executable_literal("static"));
    let dynamic_light = nodes.len() as i32;
    nodes.push(executable_literal("dynamic"));
    let value = nodes.len() as i32;
    nodes.push(integer_argument("value", true));
    nodes[rule as usize].children =
        vec![VarInt(static_light), VarInt(dynamic_light), VarInt(value)];
    rule
}

fn append_teleport_alias_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let teleport_index = append_teleport_command_nodes(nodes);
    nodes[teleport_index as usize].name = Some("tp".to_string());
    teleport_index
}

fn append_teleport_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("teleport".to_string()),
        ..Node::default()
    });

    let destination_player = nodes.len() as i32;
    nodes.push(game_profile_argument("destination", true));
    let target = nodes.len() as i32;
    nodes.push(game_profile_argument("target", false));
    let destination_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));

    let target_destination_player = nodes.len() as i32;
    nodes.push(game_profile_argument("destination", true));
    let target_dimension = nodes.len() as i32;
    nodes.push(dimension_argument("dimension", false));

    let pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    let target_pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let target_pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    let dimension_pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let dimension_pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    let target_dimension_pos = nodes.len() as i32;
    nodes.push(vec3_argument("location", true));
    let target_dimension_pos_rotation = nodes.len() as i32;
    nodes.push(rotation_argument("rotation", true));

    nodes[root as usize].children = vec![
        VarInt(destination_player),
        VarInt(target),
        VarInt(destination_dimension),
        VarInt(pos),
    ];
    nodes[target as usize].children = vec![
        VarInt(target_destination_player),
        VarInt(target_dimension),
        VarInt(target_pos),
    ];
    nodes[pos as usize].children = vec![VarInt(pos_rotation)];
    nodes[target_pos as usize].children = vec![VarInt(target_pos_rotation)];
    nodes[destination_dimension as usize].children = vec![VarInt(dimension_pos)];
    nodes[dimension_pos as usize].children = vec![VarInt(dimension_pos_rotation)];
    nodes[target_dimension as usize].children = vec![VarInt(target_dimension_pos)];
    nodes[target_dimension_pos as usize].children = vec![VarInt(target_dimension_pos_rotation)];

    root
}

fn append_entity_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let entity_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("entity".to_string()),
        ..Node::default()
    });

    let list_index = nodes.len() as i32;
    nodes.push(executable_literal("list"));

    let spawn_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("spawn".to_string()),
        ..Node::default()
    });

    let move_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("move".to_string()),
        ..Node::default()
    });

    let remove_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });

    let spawn_entity_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("entity".to_string()),
        ..Node::default()
    });
    let spawn_hologram_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("hologram".to_string()),
        ..Node::default()
    });

    let spawn_params_index = nodes.len() as i32;
    nodes.push(greedy_string_argument("params"));

    let move_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let remove_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));

    nodes[entity_index as usize].children = vec![
        VarInt(list_index),
        VarInt(spawn_index),
        VarInt(move_index),
        VarInt(remove_index),
    ];
    nodes[spawn_index as usize].children =
        vec![VarInt(spawn_entity_index), VarInt(spawn_hologram_index)];
    nodes[move_index as usize].children = vec![VarInt(move_id_index)];
    nodes[remove_index as usize].children = vec![VarInt(remove_id_index)];
    nodes[spawn_entity_index as usize].children = vec![VarInt(spawn_params_index)];
    nodes[spawn_hologram_index as usize].children = vec![VarInt(spawn_params_index)];

    entity_index
}

fn append_npc_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let npc_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("npc".to_string()),
        ..Node::default()
    });

    let list_index = nodes.len() as i32;
    nodes.push(executable_literal("list"));

    let spawn_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("spawn".to_string()),
        ..Node::default()
    });

    let move_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("move".to_string()),
        ..Node::default()
    });

    let remove_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });

    let spawn_params_index = nodes.len() as i32;
    nodes.push(greedy_string_argument("params"));
    let move_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let remove_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));

    nodes[npc_index as usize].children = vec![
        VarInt(list_index),
        VarInt(spawn_index),
        VarInt(move_index),
        VarInt(remove_index),
    ];
    nodes[spawn_index as usize].children = vec![VarInt(spawn_params_index)];
    nodes[move_index as usize].children = vec![VarInt(move_id_index)];
    nodes[remove_index as usize].children = vec![VarInt(remove_id_index)];

    npc_index
}

fn append_structure_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let structure_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("structure".to_string()),
        ..Node::default()
    });

    let list_index = nodes.len() as i32;
    nodes.push(executable_literal("list"));
    let place_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("place".to_string()),
        ..Node::default()
    });
    let locate_index = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01 | 0x04,
        children: Vec::new(),
        name: Some("locate".to_string()),
        ..Node::default()
    });

    let place_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let locate_id_index = nodes.len() as i32;
    nodes.push(word_string_argument("id", true));
    let place_position_index = nodes.len() as i32;
    nodes.push(greedy_string_argument("position"));

    nodes[structure_index as usize].children = vec![
        VarInt(list_index),
        VarInt(place_index),
        VarInt(locate_index),
    ];
    nodes[place_index as usize].children = vec![VarInt(place_id_index)];
    nodes[locate_index as usize].children = vec![VarInt(locate_id_index)];
    nodes[place_id_index as usize].children = vec![VarInt(place_position_index)];

    structure_index
}

fn greedy_string_argument(name: &str) -> Node {
    Node {
        flags: 0x02 | 0x04,
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(5)),
        properties: Some(Varies::BrigadierString(BrigadierString {
            behavior: VarInt(2),
        })),
        ..Node::default()
    }
}

fn word_string_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(5)),
        properties: Some(Varies::BrigadierString(BrigadierString {
            behavior: VarInt(0),
        })),
        ..Node::default()
    }
}

fn ask_server_word_argument(name: &str, executable: bool) -> Node {
    let mut node = word_string_argument(name, executable);
    node.flags |= 0x10;
    node.suggestions_type = Some("minecraft:ask_server".to_string());
    node
}

fn boolean_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(0)),
        ..Node::default()
    }
}

fn long_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(4)),
        ..Node::default()
    }
}

fn game_profile_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 } | 0x10,
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(7)),
        suggestions_type: Some("minecraft:ask_server".to_string()),
        ..Node::default()
    }
}

fn item_stack_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(14)),
        ..Node::default()
    }
}

fn vec3_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(10)),
        ..Node::default()
    }
}

fn rotation_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(29)),
        ..Node::default()
    }
}

fn dimension_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 } | 0x10,
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(41)),
        suggestions_type: Some("minecraft:ask_server".to_string()),
        ..Node::default()
    }
}

fn time_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(43)),
        properties: Some(Varies::MinecraftTime(MinecraftTime { min: 0 })),
        ..Node::default()
    }
}

fn integer_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(3)),
        ..Node::default()
    }
}

fn component_argument(name: &str, executable: bool) -> Node {
    Node {
        flags: 0x02 | if executable { 0x04 } else { 0 },
        children: Vec::new(),
        name: Some(name.to_string()),
        parser_id: Some(VarInt(18)),
        ..Node::default()
    }
}

fn append_scoreboard_command_nodes(nodes: &mut Vec<Node>) -> i32 {
    let root = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("scoreboard".to_string()),
        ..Node::default()
    });

    let objectives = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("objectives".to_string()),
        ..Node::default()
    });
    let players = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("players".to_string()),
        ..Node::default()
    });
    let sidebar = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("sidebar".to_string()),
        ..Node::default()
    });

    let objectives_list = nodes.len() as i32;
    nodes.push(executable_literal("list"));
    let objectives_add = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("add".to_string()),
        ..Node::default()
    });
    let objectives_remove = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });
    let objectives_setdisplay = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("setdisplay".to_string()),
        ..Node::default()
    });

    let add_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let add_criteria = nodes.len() as i32;
    nodes.push(word_string_argument("criteria", true));
    let add_display = nodes.len() as i32;
    nodes.push(component_argument("displayName", true));
    let remove_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", true));
    let display_slot = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("sidebar".to_string()),
        ..Node::default()
    });
    let display_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", true));

    let players_list = nodes.len() as i32;
    nodes.push(executable_literal("list"));
    let players_set = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("set".to_string()),
        ..Node::default()
    });
    let players_add = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("add".to_string()),
        ..Node::default()
    });
    let players_remove = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("remove".to_string()),
        ..Node::default()
    });
    let players_reset = nodes.len() as i32;
    nodes.push(Node {
        flags: 0x01,
        children: Vec::new(),
        name: Some("reset".to_string()),
        ..Node::default()
    });

    let set_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", false));
    let set_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let set_score = nodes.len() as i32;
    nodes.push(integer_argument("score", true));
    let add_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", false));
    let add_objective_name = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let add_score = nodes.len() as i32;
    nodes.push(integer_argument("score", true));
    let remove_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", false));
    let remove_objective_name = nodes.len() as i32;
    nodes.push(word_string_argument("objective", false));
    let remove_score = nodes.len() as i32;
    nodes.push(integer_argument("score", true));
    let reset_target = nodes.len() as i32;
    nodes.push(word_string_argument("targets", true));
    let reset_objective = nodes.len() as i32;
    nodes.push(word_string_argument("objective", true));

    let sidebar_on = nodes.len() as i32;
    nodes.push(executable_literal("on"));
    let sidebar_off = nodes.len() as i32;
    nodes.push(executable_literal("off"));
    let sidebar_reload = nodes.len() as i32;
    nodes.push(executable_literal("reload"));

    nodes[root as usize].children = vec![VarInt(objectives), VarInt(players), VarInt(sidebar)];
    nodes[objectives as usize].children = vec![
        VarInt(objectives_list),
        VarInt(objectives_add),
        VarInt(objectives_remove),
        VarInt(objectives_setdisplay),
    ];
    nodes[objectives_add as usize].children = vec![VarInt(add_objective)];
    nodes[add_objective as usize].children = vec![VarInt(add_criteria)];
    nodes[add_criteria as usize].children = vec![VarInt(add_display)];
    nodes[objectives_remove as usize].children = vec![VarInt(remove_objective)];
    nodes[objectives_setdisplay as usize].children = vec![VarInt(display_slot)];
    nodes[display_slot as usize].children = vec![VarInt(display_objective)];

    nodes[players as usize].children = vec![
        VarInt(players_list),
        VarInt(players_set),
        VarInt(players_add),
        VarInt(players_remove),
        VarInt(players_reset),
    ];
    nodes[players_set as usize].children = vec![VarInt(set_target)];
    nodes[set_target as usize].children = vec![VarInt(set_objective)];
    nodes[set_objective as usize].children = vec![VarInt(set_score)];
    nodes[players_add as usize].children = vec![VarInt(add_target)];
    nodes[add_target as usize].children = vec![VarInt(add_objective_name)];
    nodes[add_objective_name as usize].children = vec![VarInt(add_score)];
    nodes[players_remove as usize].children = vec![VarInt(remove_target)];
    nodes[remove_target as usize].children = vec![VarInt(remove_objective_name)];
    nodes[remove_objective_name as usize].children = vec![VarInt(remove_score)];
    nodes[players_reset as usize].children = vec![VarInt(reset_target)];
    nodes[reset_target as usize].children = vec![VarInt(reset_objective)];

    nodes[sidebar as usize].children = vec![
        VarInt(sidebar_on),
        VarInt(sidebar_off),
        VarInt(sidebar_reload),
    ];
    root
}
