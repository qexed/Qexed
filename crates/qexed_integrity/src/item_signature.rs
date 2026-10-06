//! 服务端物品签名（防客户端伪造/注入物品）。
//!
//! 攻击：客户端发 ContainerClick/SetCreativeModeSlot 携带自构 NBT 物品
//! （如 64 钻石带自定义附魔）。防护：贵重物品由服务端签发时附签名
//! （物品字段+玩家+时间窗的 HMAC 截断），回收/转移时验签。

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::{IntegrityError, Result, ViolationKind};

type HmacSha256 = Hmac<Sha256>;

/// 物品签名（截断 HMAC，16 字节 = 128 位安全余量足够）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSignature([u8; 16]);

impl ItemSignature {
    /// 服务端签发（item_bytes = Slot 序列化字节或稳定字段编码）。
    pub fn issue(owner: uuid::Uuid, item_bytes: &[u8]) -> Self {
        let mut mac = HmacSha256::new_from_slice(crate::master_key()).expect("hmac key");
        mac.update(owner.as_bytes());
        mac.update(item_bytes);
        let full = mac.finalize().into_bytes();
        let mut out = [0u8; 16];
        out.copy_from_slice(&full[..16]);
        Self(out)
    }

    /// 验签（owner 绑定——他人签名不可重放）。
    pub fn verify(&self, owner: uuid::Uuid, item_bytes: &[u8]) -> Result<()> {
        let expect = Self::issue(owner, item_bytes);
        // 常时比较（防时序侧信道）
        if constant_time_eq(&self.0, &expect.0) {
            Ok(())
        } else {
            Err(IntegrityError::Violation {
                kind: ViolationKind::ForeignSignature,
                where_: "item_signature".to_string(),
            })
        }
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let arr: [u8; 16] = bytes.try_into().ok()?;
        Some(Self(arr))
    }
}

/// 无签名物品的判定（客户端上报的物品既无签名又不是系统白名单原始物品）。
pub fn reject_unsigned(owner: uuid::Uuid) -> Result<()> {
    let _ = owner;
    Err(IntegrityError::Violation {
        kind: ViolationKind::UnsignedItem,
        where_: "client_reported_item".to_string(),
    })
}

/// 定长字节常时比较（防时序侧信道）。
fn constant_time_eq(a: &[u8; 16], b: &[u8; 16]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
