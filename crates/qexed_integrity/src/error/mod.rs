//! 完整性错误与违规分类。

#[derive(Debug, thiserror::Error)]
pub enum IntegrityError {
    #[error("integrity violation: {kind} at {where_}")]
    Violation {
        kind: ViolationKind,
        where_: String,
    },
    #[error("{0}")]
    Message(String),
}

impl IntegrityError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub type Result<T> = std::result::Result<T, IntegrityError>;

/// 违规种类（审计事件分类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// 影子签名不匹配（内存被外部改写）。
    ShadowMismatch,
    /// 客户端上报物品无合法服务端签名。
    UnsignedItem,
    /// 签名有效但不属于该玩家（重放他人签名）。
    ForeignSignature,
    /// 业务不变量破坏（如负余额/超堆叠）。
    InvariantBroken,
}

impl ViolationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ShadowMismatch => "shadow_mismatch",
            Self::UnsignedItem => "unsigned_item",
            Self::ForeignSignature => "foreign_signature",
            Self::InvariantBroken => "invariant_broken",
        }
    }
}
