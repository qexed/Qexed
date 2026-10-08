//! 路径卫生测试：调用方存在与 crate 同名的本地模块时，宏生成的 impl
//! 仍然必须指向真正的 qexed_config（extern prelude），而不是本地模块。

// 故意撞名的本地模块：内含一个形状不兼容的 Config trait。
// 若宏生成的是 `impl qexed_config::Config`（无前导 ::），路径会解析到这里，
// 编译要么报错、要么静默 impl 错误的 trait。
pub mod qexed_config {
    pub trait Config {
        fn not_the_real_trait(&self) -> bool;
    }
}

// 本地 trait 的实现者（与宏无关，仅证明共存）
struct HygieneConfig {
    name: String,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[qexed_config_macros::app_config("/", "hygiene")]
struct HygieneConfig2 {
    name: String,
}

// 本地 trait 与全局 derive 共存：证明 impl 目标没有被本地模块劫持
impl qexed_config::Config for HygieneConfig {
    fn not_the_real_trait(&self) -> bool {
        true
    }
}

#[test]
fn generated_impl_targets_real_crate() {
    // 前导 :: 绕过本地同名模块，导入真 crate 的 trait
    use ::qexed_config::Config as _;
    // 生成的 impl 存在且指向真 crate：这些关联常量来自真正的 Config trait
    // （本地模块的 Config trait 没有这些常量；若 impl 被劫持会直接编译失败）
    assert_eq!(HygieneConfig2::PATH, "/");
    assert_eq!(HygieneConfig2::NAME, "hygiene");
}

#[test]
fn local_shadowing_trait_still_usable() {
    // 本地 trait 与真 crate 共存，互不干扰
    let c = HygieneConfig { name: "x".into() };
    assert!(qexed_config::Config::not_the_real_trait(&c));
}
