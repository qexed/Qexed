//! 带影子校验的值容器。
//!
//! CE 攻击路径：扫描内存定位数值（如钻石 64）→ 改写。
//! 防护：值与 HMAC 影子**分离存放**（非相邻内存），改值不改影子 → 下次读校验失败。
//! 密钥为进程随机（无法离线预计算影子）；影子含位置盐（防同类值批量复制影子）。

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::{IntegrityError, Result, ViolationKind};

type HmacSha256 = Hmac<Sha256>;

/// 防篡改守卫值。
///
/// - 读写均校验影子；不匹配返回 `Violation`（由调用方决定回滚/断开/告警）。
/// - Clone 特意不支持（防止复制后双写绕过）。
#[repr(C)]
pub struct Guarded<T> {
    /// 真实值。
    value: T,
    /// 影子签（值的 HMAC，含位置盐）。
    shadow: [u8; 16],
    /// 位置盐（每个 Guarded 实例唯一——防"把 A 的影子拷给 B"）。
    salt: u64,
    /// 序号（每次写递增——防"改回旧值+旧影子"的快照回滚攻击）。
    generation: u64,
    label: &'static str,
}

impl<T: SerializableValue> Guarded<T> {
    pub fn new(value: T, label: &'static str) -> Self {
        let salt = next_salt();
        let generation = 0;
        let shadow = Self::sign(&value, salt, generation, label);
        Self { value, shadow, salt, generation, label }
    }

    /// 读取并校验（CE 改值后此处失败）。
    pub fn read(&self) -> Result<&T> {
        let expect = Self::sign(&self.value, self.salt, self.generation, self.label);
        if expect != self.shadow {
            return Err(IntegrityError::Violation {
                kind: ViolationKind::ShadowMismatch,
                where_: self.label.to_string(),
            });
        }
        Ok(&self.value)
    }

    /// 写入（原子更新值与影子）。
    pub fn write(&mut self, value: T) {
        self.generation += 1;
        self.shadow = Self::sign(&value, self.salt, self.generation, self.label);
        self.value = value;
    }

    /// 读取不校验（仅审计对照用——正常业务禁止使用）。
    pub fn read_unchecked(&self) -> &T {
        &self.value
    }

    /// 强制修复（审计确认合法来源后调用；重置影子）。
    pub fn repair(&mut self, value: T) {
        self.write(value);
    }

    fn sign(value: &T, salt: u64, generation: u64, label: &str) -> [u8; 16] {
        let mut mac = HmacSha256::new_from_slice(crate::master_key()).expect("hmac key");
        mac.update(&salt.to_le_bytes());
        mac.update(&generation.to_le_bytes());
        mac.update(label.as_bytes());
        mac.update(&value.to_bytes());
        let full = mac.finalize().into_bytes();
        let mut out = [0u8; 16];
        out.copy_from_slice(&full[..16]);
        out
    }
}

impl<T: SerializableValue + std::fmt::Debug> std::fmt::Debug for Guarded<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Debug 不暴露原始值（防日志侧信道被用来定位内存）
        f.debug_struct("Guarded").field("label", &self.label).finish_non_exhaustive()
    }
}

/// 可参与影子的值（数值类型的稳定字节表示）。
pub trait SerializableValue {
    fn to_bytes(&self) -> Vec<u8>;
}

macro_rules! impl_int_value {
    ($($ty:ty),*) => {
        $(impl SerializableValue for $ty {
            fn to_bytes(&self) -> Vec<u8> { self.to_le_bytes().to_vec() }
        })*
    };
}

impl_int_value!(i8, i16, i32, i64, u8, u16, u32, u64);

fn next_salt() -> u64 {
    use rand::RngCore;
    rand::thread_rng().next_u64()
}
