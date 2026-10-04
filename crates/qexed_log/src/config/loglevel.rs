use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};
use tklog::LEVEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl LogLevel {
    pub fn as_tklog(self) -> LEVEL {
        match self {
            Self::Trace => LEVEL::Trace,
            Self::Debug => LEVEL::Debug,
            Self::Info => LEVEL::Info,
            Self::Warn => LEVEL::Warn,
            Self::Error => LEVEL::Error,
            Self::Off => LEVEL::Off,
        }
    }
}