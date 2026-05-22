use qexed_config_macros::AutoDoc;
use rust_i18n::t;
use serde::{Deserialize, Serialize, de};

#[derive(Debug, Serialize, Deserialize, AutoDoc)]
pub struct Server {
    #[AutoDoc(key = "config.qexed.server.ip")]
    pub ip: String,

    #[AutoDoc(
        key = "config.qexed.server.online",
        warning = "config.qexed.server.warning.online"
    )]
    pub online: bool,

    #[AutoDoc(key = "config.qexed.server.max_player")]
    pub max_player: i32,

    #[AutoDoc(key = "config.qexed.server.display_players")]
    pub display_players: bool,

    #[AutoDoc(
        key = "config.qexed.server.online_mode",
        warning = "config.qexed.server.warning.online_mode"
    )]
    pub online_mode: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.lan_discovery", sub)]
    pub lan_discovery: LanDiscovery,

    #[AutoDoc(key = "config.qexed.server.network_compression_threshold")]
    pub network_compression_threshold: isize,

    #[AutoDoc(key = "config.qexed.server.proxy")]
    pub proxy: bool,

    #[AutoDoc(key = "config.qexed.server.proxy_protocol")]
    pub proxy_protocol: ForwardingMode,

    #[AutoDoc(
        key = "config.qexed.server.proxy_token",
        warning = "config.qexed.server.warning.proxy_token"
    )]
    pub proxy_token: String,

    #[AutoDoc(key = "config.qexed.server.max_port_connections")]
    pub max_port_connections: u16,

    #[AutoDoc(key = "config.qexed.server.rate_limit_window_secs")]
    pub rate_limit_window_secs: u64,

    #[AutoDoc(key = "config.qexed.server.rate_limit_max_attempts")]
    pub rate_limit_max_attempts: u32,

    #[AutoDoc(key = "config.qexed.server.motd")]
    pub motd: Vec<String>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.code_of_conduct")]
    pub code_of_conduct: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world", sub)]
    pub world: World,

    #[AutoDoc(key = "config.qexed.server.favicon")]
    pub favicon: String,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            ip: "0.0.0.0:25565".to_owned(),
            online: false,
            max_player: -1,
            display_players: true,
            online_mode: true,
            lan_discovery: LanDiscovery::default(),
            network_compression_threshold: 256,
            proxy: false,
            proxy_protocol: ForwardingMode::QTunnel,
            proxy_token: nanoid::nanoid!(),
            rate_limit_window_secs: 60,
            rate_limit_max_attempts: 6,
            motd: vec![
                t!("qexed_config.config.server.motd1").to_string(),
                t!("qexed_config.config.server.motd2").to_string(),
            ],
            code_of_conduct: String::new(),
            world: World::default(),
            favicon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAACXBIWXMAAA9hAAAPYQGoP6dpAAACtklEQVR42u2ay0rDQBSGJ2EQCipqERU3SkFQQUERRJSCuHDrQvcu3Powbn0DH6IIohQKIi26ELRF8FLxAlbsyksTmTC2yVwyk3ZizmySkjaT/zvnP5mT1PpuDJTgYaOEDwAAAAAAAAAAAAAAAAAAAAAAAABI5MAyX7YsS3lC07pvLCs+jAAd4DpqARXxia8BpsOzk5r6QgB0iTfZOnbU0TO9bthRCIhT0cRRpX4U4v2yUnUezJokrA10iReZX7XWYN0CdNUO0Wj7BUzm+rGJvpSJKn2c/M7ZikKQArC/1RqVnYPvjon3gyELwWp+N+j3QyJ8cHYNTU30uPuvz1V3W8yd/IHAmph3UToLqOi5uAAc8dnNDU/0eeW95SSf10UPQlgAQZEPC0U0kzAv5R3xtPDFhRQaHc+4+7flK1R5SqHba30W0HUHoe2gVAOIeFo4EZ8v1Bt7dTSzuuTCCqoHKqmvAoRAYM2PWdF3PH9eeQwUvzyf8WpB6cZiimve6rrdqmYMcylMCh6d8seFO088GbkiZkaB5ftOd4yYl/601x/KvylPxBN7DPUidC+Yjn4FTtQqYazBswETgCNueHygIR41xL94wi8ua2gk/eEer771ofvTI7SX/wrl7043Tszb4O6ijda3s6746bFu1J8e8rLCEX92WHL3afG8KDdHsB0AWHNw1wEOhJG5FTQ52uV+fqk9+grnpTHP68YCIBDoEZTuMguhZiBGA5CdTHYl2I5nCEHnhldj/1mcyBrDCABBdaEd/YUdx6jpPI8xAHQWQJl+w6gMoK0QNhNkmy3jLCCyihRprCJ5JqialrINjEhEVd8VYNPEs+4MUSynjXwsLmMJ7W+GTIh+OxsmOy7iY7cUjoN4aIaiAhCX6AcWQdX1eJz+TYbjfPFQAwAAAAAAAAAAAACl8QOub9TOwLTmGwAAAABJRU5ErkJggg==".to_string(),
            max_port_connections: u16::MAX,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc)]
pub struct LanDiscovery {
    #[AutoDoc(key = "config.qexed.server.lan_discovery.enable")]
    pub enable: bool,

    #[AutoDoc(key = "config.qexed.server.lan_discovery.interval_ms")]
    pub interval_ms: u64,
}

impl Default for LanDiscovery {
    fn default() -> Self {
        Self {
            enable: true,
            interval_ms: 1500,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc)]
pub struct World {
    #[AutoDoc(key = "config.qexed.server.world.path")]
    pub path: String,

    #[AutoDoc(key = "config.qexed.server.world.dimension")]
    pub dimension: String,

    #[AutoDoc(key = "config.qexed.server.world.dimension_type")]
    pub dimension_type: String,

    #[AutoDoc(key = "config.qexed.server.world.view_distance")]
    pub view_distance: i32,

    #[serde(default = "default_chunk_load_parallelism")]
    #[AutoDoc(key = "config.qexed.server.world.chunk_load_parallelism")]
    pub chunk_load_parallelism: usize,

    #[AutoDoc(key = "config.qexed.server.world.simulation_distance")]
    pub simulation_distance: i32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light")]
    pub light: LightMode,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.world.light_algorithm")]
    pub light_algorithm: LightAlgorithm,

    #[AutoDoc(key = "config.qexed.server.world.spawn", sub)]
    pub spawn: Spawn,
}

impl Default for World {
    fn default() -> Self {
        Self {
            path: "world".to_string(),
            dimension: "minecraft:overworld".to_string(),
            dimension_type: "minecraft:overworld".to_string(),
            view_distance: 3,
            chunk_load_parallelism: default_chunk_load_parallelism(),
            simulation_distance: 3,
            light: LightMode::default(),
            light_algorithm: LightAlgorithm::default(),
            spawn: Spawn::default(),
        }
    }
}

fn default_chunk_load_parallelism() -> usize {
    4
}

#[derive(Debug, Clone, PartialEq)]
pub enum LightMode {
    Static,
    Dynamic,
    Fixed(u8),
}

impl Default for LightMode {
    fn default() -> Self {
        Self::Static
    }
}

impl Serialize for LightMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Static => serializer.serialize_str("static"),
            Self::Dynamic => serializer.serialize_str("dynamic"),
            Self::Fixed(level) => serializer.serialize_u8(*level),
        }
    }
}

impl<'de> Deserialize<'de> for LightMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = LightMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(r#""static", "dynamic", or an integer brightness from 0 to 15"#)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "static" => Ok(LightMode::Static),
                    "dynamic" => Ok(LightMode::Dynamic),
                    other => other
                        .parse::<u8>()
                        .map_err(|_| E::custom(format!("unknown light mode: {value}")))
                        .and_then(fixed_light_mode),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = u8::try_from(value)
                    .map_err(|_| E::custom(format!("brightness out of range 0..=15: {value}")))?;
                fixed_light_mode(value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = u8::try_from(value)
                    .map_err(|_| E::custom(format!("brightness out of range 0..=15: {value}")))?;
                fixed_light_mode(value)
            }
        }

        fn fixed_light_mode<E>(value: u8) -> Result<LightMode, E>
        where
            E: de::Error,
        {
            if value <= 15 {
                Ok(LightMode::Fixed(value))
            } else {
                Err(E::custom(format!(
                    "brightness out of range 0..=15: {value}"
                )))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LightAlgorithm {
    #[default]
    Fast,
    RayTrace,
}

#[derive(Debug, Clone, Deserialize, Serialize, AutoDoc)]
pub struct Spawn {
    #[AutoDoc(key = "config.qexed.server.world.spawn.x")]
    pub x: f64,

    #[AutoDoc(key = "config.qexed.server.world.spawn.y")]
    pub y: f64,

    #[AutoDoc(key = "config.qexed.server.world.spawn.z")]
    pub z: f64,

    #[AutoDoc(key = "config.qexed.server.world.spawn.yaw")]
    pub yaw: f32,

    #[AutoDoc(key = "config.qexed.server.world.spawn.pitch")]
    pub pitch: f32,
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum ForwardingMode {
    Default,
    QTunnel,
    Victory,
    BungeeCord,
    None,
}

impl Default for ForwardingMode {
    fn default() -> Self {
        ForwardingMode::Default
    }
}

impl std::fmt::Display for ForwardingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardingMode::Default => write!(f, "Default"),
            ForwardingMode::QTunnel => write!(f, "QTunnel"),
            ForwardingMode::Victory => write!(f, "Victory"),
            ForwardingMode::BungeeCord => write!(f, "BungeeCord"),
            ForwardingMode::None => write!(f, "None"),
        }
    }
}

impl std::str::FromStr for ForwardingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(ForwardingMode::Default),
            "qtunnel" => Ok(ForwardingMode::QTunnel),
            "victory" => Ok(ForwardingMode::Victory),
            "bungeecord" => Ok(ForwardingMode::BungeeCord),
            "none" => Ok(ForwardingMode::None),
            _ => Err(format!("未知的转发模式: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LightAlgorithm, LightMode, World};

    #[test]
    fn parses_world_light_string_modes() {
        let static_world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "static"

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(static_world.light, LightMode::Static);

        let dynamic_world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = "dynamic"

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(dynamic_world.light, LightMode::Dynamic);
    }

    #[test]
    fn parses_world_light_fixed_brightness() {
        let world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 4
simulation_distance = 3
light = 12

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(world.light, LightMode::Fixed(12));
    }

    #[test]
    fn parses_world_light_algorithm() {
        let world: World = toml::from_str(
            r#"
path = "world"
dimension = "minecraft:overworld"
dimension_type = "minecraft:overworld"
view_distance = 3
chunk_load_parallelism = 6
simulation_distance = 3
light = "static"
light_algorithm = "ray_trace"

[spawn]
x = 0.0
y = 0.0
z = 0.0
yaw = 0.0
pitch = 0.0
"#,
        )
        .unwrap();
        assert_eq!(world.light_algorithm, LightAlgorithm::RayTrace);
        assert_eq!(world.chunk_load_parallelism, 6);
    }
}
