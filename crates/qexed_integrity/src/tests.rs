//! 完整性防护测试（含内存篡改模拟）。

use crate::audit::{AuditEvent, Auditor};
use crate::error::ViolationKind;
use crate::guarded::Guarded;
use crate::item_signature::ItemSignature;

#[test]
fn guarded_normal_read_write() {
    let mut diamonds = Guarded::new(64i64, "diamonds");
    assert_eq!(*diamonds.read().unwrap(), 64);
    diamonds.write(128);
    assert_eq!(*diamonds.read().unwrap(), 128);
}

#[test]
fn guarded_detects_memory_tamper() {
    let mut diamonds = Guarded::new(64i64, "diamonds");
    // 模拟 CE：绕过 write 直接改内存（unsafe 访问私有字段等价物——
    // 这里通过裸指针改 value 模拟外部工具写内存）
    let value_ptr: *mut i64 = &mut diamonds as *mut _ as *mut i64;
    unsafe {
        *value_ptr = 999_999; // CE 写死钻石
    }
    // 读取必须检测到影子不符
    let result = diamonds.read();
    let err = result.err().expect("tamper must be detected");
    match err {
        crate::IntegrityError::Violation { kind, .. } => {
            assert_eq!(kind, ViolationKind::ShadowMismatch);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn guarded_tamper_then_repair() {
    let mut balance = Guarded::new(1_000i64, "balance");
    let ptr: *mut i64 = &mut balance as *mut _ as *mut i64;
    unsafe { *ptr = 1_000_000_000 };
    assert!(balance.read().is_err());
    // 审计确认合法后修复
    balance.repair(1_000);
    assert_eq!(*balance.read().unwrap(), 1_000);
}

#[test]
fn guarded_generation_prevents_rollback() {
    // 快照值+影子回滚攻击：拿到旧 (value, shadow) 组合恢复
    let mut gems = Guarded::new(10i32, "gems");
    gems.write(20);
    // 即使外部把值改回 10 并复制了"曾经的合法影子"，generation 已变 → 失败
    let ptr: *mut i32 = &mut gems as *mut _ as *mut i32;
    unsafe { *ptr = 10 };
    assert!(gems.read().is_err());
}

#[test]
fn guarded_debug_hides_value() {
    let secret = Guarded::new(12345i64, "wallet");
    let text = format!("{secret:?}");
    assert!(!text.contains("12345"), "Debug 不得泄漏值（防日志侧信道定位）");
}

#[test]
fn item_signature_roundtrip() {
    let owner = uuid::Uuid::new_v4();
    let item = b"minecraft:diamond x64 with sharpness 5";
    let sig = ItemSignature::issue(owner, item);
    assert!(sig.verify(owner, item).is_ok());
}

#[test]
fn item_signature_rejects_forged() {
    let owner = uuid::Uuid::new_v4();
    let forged = ItemSignature::issue(owner, b"hacked item");
    // 验证目标物品与签名不符
    assert!(forged.verify(owner, b"real item").is_err());
}

#[test]
fn item_signature_owner_bound() {
    let alice = uuid::Uuid::new_v4();
    let bob = uuid::Uuid::new_v4();
    let item = b"netherite ingot";
    let sig = ItemSignature::issue(alice, item);
    // Bob 重放 Alice 的签名 → ForeignSignature
    let err = sig.verify(bob, item).err().unwrap();
    match err {
        crate::IntegrityError::Violation { kind, .. } => {
            assert_eq!(kind, ViolationKind::ForeignSignature);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn auditor_collects_events() {
    let mut auditor = Auditor::new();
    auditor.register("balance_check", || {
        vec![AuditEvent {
            kind: ViolationKind::InvariantBroken,
            label: "economy.balance".to_string(),
            detail: "negative".to_string(),
        }]
    });
    let report = auditor.run_round();
    assert!(report.has_violations());
    assert!(report.summary().contains("invariant_broken"));
}

#[test]
fn auditor_clean_round() {
    let mut auditor = Auditor::new();
    auditor.register("noop", || Vec::new());
    let report = auditor.run_round();
    assert!(!report.has_violations());
    assert_eq!(report.summary(), "integrity ok");
}
