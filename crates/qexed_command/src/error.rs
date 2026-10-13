//! 指令系统错误类型。

use std::fmt;

/// 指令解析 / 执行错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// 指令名未注册（含斜杠内容首词不匹配任何 literal）。
    UnknownCommand(String),
    /// 指令存在但参数与任何签名不符；附带该指令的 usage 提示。
    InvalidUsage { usage: String },
    /// 单个参数解析失败：参数名、期望类型、原始词元。
    BadArgument {
        name: String,
        expected: &'static str,
        raw: String,
    },
    /// 执行回调自身返回的业务错误，原样透传。
    Custom(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(cmd) => write!(f, "Unknown command: /{cmd}"),
            Self::InvalidUsage { usage } => write!(f, "Incorrect argument, usage: {usage}"),
            Self::BadArgument {
                name,
                expected,
                raw,
            } => {
                write!(f, "Incorrect argument for parameter <{name}>, expected {expected}, got {raw:?}")
            }
            Self::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CommandError {}
