use serde::{Deserialize, Serialize};

mod level_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::str::FromStr;

    pub fn serialize<S>(level: &tklog::LEVEL, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{:?}", level);
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<tklog::LEVEL, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        tklog::LEVEL::from_str(&s)
            .map_err(|_| serde::de::Error::custom(format!("unknown LEVEL: {}", s)))
    }
}

mod mode_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(mode: &tklog::MODE, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match mode {
            tklog::MODE::HOUR => "HOUR",
            tklog::MODE::DAY => "DAY",
            tklog::MODE::MONTH => "MONTH",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<tklog::MODE, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "HOUR" => Ok(tklog::MODE::HOUR),
            "DAY" => Ok(tklog::MODE::DAY),
            "MONTH" => Ok(tklog::MODE::MONTH),
            other => Err(serde::de::Error::custom(format!(
                "unknown MODE: {}, expected HOUR/DAY/MONTH",
                other
            ))),
        }
    }
}

fn mode_debug(mode: &tklog::MODE) -> &'static str {
    match mode {
        tklog::MODE::HOUR => "HOUR",
        tklog::MODE::DAY => "DAY",
        tklog::MODE::MONTH => "MONTH",
    }
}

#[derive(Serialize, Deserialize)]
pub struct Log {
    #[serde(with = "level_serde")]
    pub level: tklog::LEVEL,
    #[serde(with = "mode_serde")]
    pub mode: tklog::MODE,
    pub maxbackups: u32,
    pub compress: bool,
}

impl std::fmt::Debug for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Log")
            .field("level", &format!("{:?}", self.level))
            .field("mode", &mode_debug(&self.mode))
            .field("maxbackups", &self.maxbackups)
            .field("compress", &self.compress)
            .finish()
    }
}

impl Default for Log {
    fn default() -> Self {
        Self {
            level: tklog::LEVEL::Info,
            mode: tklog::MODE::DAY,
            maxbackups: 30,
            compress: true,
        }
    }
}

impl qexed_config::tool::AppConfigTrait for Log {
    const PATH: &'static str = "/";
    const NAME: &'static str = "log";
}
