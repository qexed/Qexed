//! 集成测试：验证 `Config` trait 与 `split_secrets` / `merge_secrets` 的联动。
//!
//! 注意：`config_path` 使用进程级 `OnceLock`，一个测试二进制只能初始化一次。
//! 因此这里用一个全局 `setup()` 初始化根目录，各测试通过不同的 `Config::PATH`
//! 落到不同子目录，互不干扰。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use qexed_config::{config_path, init_config_path, Config};

const PLACEHOLDER: &str = "<stored in .secrets>";

static INIT: OnceLock<PathBuf> = OnceLock::new();

/// 进程内第一次调用时初始化一个唯一的临时配置根目录。
fn setup() -> &'static PathBuf {
    INIT.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!(
            "qexed_config_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        init_config_path(dir.clone()).expect("init config path");
        dir
    })
}

/// 复刻 `config_file` 的路径推导逻辑（它不对外，这里是测试私有副本）。
fn main_file(path: &str, name: &str) -> PathBuf {
    config_path().unwrap().join(path).join(format!("{name}.toml"))
}

/// secrets 文件路径：与主文件同模块目录，位于 `<PATH>/.secrets/<NAME>.toml`。
fn secret_file(path: &str, name: &str) -> PathBuf {
    config_path()
        .unwrap()
        .join(path)
        .join(".secrets")
        .join(format!("{name}.toml"))
}

fn read(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

// ---------------------------------------------------------------------------
// 基本往返
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_basic_secret() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg {
        token: String,
        name: String,
    }
    impl Config for Cfg {
        const PATH: &'static str = "roundtrip_basic_secret";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["token"];
    }

    let cfg = Cfg { token: "sk-abc".into(), name: "alice".into() };
    Cfg::create_file(Some(&cfg)).unwrap();

    let main = read(&main_file(Cfg::PATH, Cfg::NAME)).expect("main file exists");
    let secret = read(&secret_file(Cfg::PATH, Cfg::NAME)).expect("secret file exists");

    assert!(main.contains(PLACEHOLDER), "main = {main}");
    assert!(!main.contains("sk-abc"), "main = {main}");
    assert!(main.contains("alice"), "main = {main}");

    assert!(secret.contains("sk-abc"), "secret = {secret}");
    assert!(!secret.contains("alice"), "secret = {secret}");

    let loaded = Cfg::load_file(false).unwrap();
    assert_eq!(loaded, cfg);
}

#[test]
fn roundtrip_nested_secret() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Auth { username: String, password: String }
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { auth: Auth }
    impl Config for Cfg {
        const PATH: &'static str = "roundtrip_nested_secret";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["auth.password"];
    }

    let cfg = Cfg { auth: Auth { username: "alice".into(), password: "hunter2".into() } };
    Cfg::create_file(Some(&cfg)).unwrap();

    let main = read(&main_file(Cfg::PATH, Cfg::NAME)).unwrap();
    assert!(main.contains("alice"));
    assert!(main.contains(PLACEHOLDER));
    assert!(!main.contains("hunter2"));

    let secret = read(&secret_file(Cfg::PATH, Cfg::NAME)).unwrap();
    assert!(secret.contains("hunter2"));
    assert!(!secret.contains("alice"));

    assert_eq!(Cfg::load_file(false).unwrap(), cfg);
}

#[test]
fn roundtrip_array_wildcard_secret() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Server { host: String, api_key: String }
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { servers: Vec<Server> }
    impl Config for Cfg {
        const PATH: &'static str = "roundtrip_array_wildcard_secret";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["servers.*.api_key"];
    }

    let cfg = Cfg {
        servers: vec![
            Server { host: "a.example.com".into(), api_key: "key-a".into() },
            Server { host: "b.example.com".into(), api_key: "key-b".into() },
        ],
    };
    Cfg::create_file(Some(&cfg)).unwrap();

    let main = read(&main_file(Cfg::PATH, Cfg::NAME)).unwrap();
    assert!(main.contains("a.example.com"));
    assert!(main.contains("b.example.com"));
    assert!(!main.contains("key-a"), "main = {main}");
    assert!(!main.contains("key-b"), "main = {main}");

    let secret = read(&secret_file(Cfg::PATH, Cfg::NAME)).unwrap();
    assert!(secret.contains("key-a"));
    assert!(secret.contains("key-b"));
    assert!(!secret.contains("a.example.com"));

    assert_eq!(Cfg::load_file(false).unwrap(), cfg);
}

// ---------------------------------------------------------------------------
// 文件创建 / 清理
// ---------------------------------------------------------------------------

#[test]
fn load_and_create_default_creates_files() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg {
        #[serde(default)] token: String,
        #[serde(default)] name: String,
    }
    impl Config for Cfg {
        const PATH: &'static str = "load_and_create_default_creates_files";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["token"];
    }

    let main_path = main_file(Cfg::PATH, Cfg::NAME);
    assert!(!main_path.exists(), "should be clean before test");

    let cfg = Cfg::load_and_create_default(false).unwrap();
    assert_eq!(cfg, Cfg::default());
    assert!(main_path.exists(), "main file should have been created");
}

#[test]
fn no_secret_fields_no_secret_file() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { name: String }
    impl Config for Cfg {
        const PATH: &'static str = "no_secret_fields_no_secret_file";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &[];
    }

    let cfg = Cfg { name: "alice".into() };
    Cfg::create_file(Some(&cfg)).unwrap();

    assert!(
        !secret_file(Cfg::PATH, Cfg::NAME).exists(),
        "no secret file expected when SECRETS is empty"
    );

    let main = read(&main_file(Cfg::PATH, Cfg::NAME)).unwrap();
    assert!(main.contains("alice"));
    assert!(!main.contains(PLACEHOLDER));
}

// ---------------------------------------------------------------------------
// 未知字段 / 规则变更
// ---------------------------------------------------------------------------

#[test]
fn save_preserves_unknown_fields_in_main_file() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { token: String, name: String }
    impl Config for Cfg {
        const PATH: &'static str = "save_preserves_unknown_fields_in_main_file";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["token"];
    }

    let cfg = Cfg { token: "sk-abc".into(), name: "alice".into() };
    Cfg::create_file(Some(&cfg)).unwrap();

    let main_path = main_file(Cfg::PATH, Cfg::NAME);
    let mut main = read(&main_path).unwrap();
    main.push_str("\nunknown_field = \"keep-me\"\n");
    std::fs::write(&main_path, &main).unwrap();

    Cfg::save_file(&cfg).unwrap();

    let after = read(&main_path).unwrap();
    assert!(
        after.contains("unknown_field") && after.contains("keep-me"),
        "unknown field lost after save:\n{after}"
    );
}

#[test]
fn previously_secret_field_stays_secret_after_rule_change() {
    setup();

    // 阶段一：token 在 SECRETS 中
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct CfgA { token: String, name: String }
    impl Config for CfgA {
        const PATH: &'static str = "previously_secret_field_stays_secret_after_rule_change";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["token"];
    }

    let a = CfgA { token: "sk-abc".into(), name: "alice".into() };
    CfgA::create_file(Some(&a)).unwrap();
    assert!(main_file(CfgA::PATH, CfgA::NAME).exists());
    assert!(secret_file(CfgA::PATH, CfgA::NAME).exists());

    // 阶段二：SECRETS 不再声明 token，但旧 secrets 里存在 token →
    // "一旦机密，永远是机密"，仍被抽出。
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct CfgB { token: String, name: String }
    impl Config for CfgB {
        const PATH: &'static str =
            "previously_secret_field_stays_secret_after_rule_change";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &[];
    }

    let b = CfgB { token: "sk-abc".into(), name: "alice".into() };
    CfgB::save_file(&b).unwrap();

    let main = read(&main_file(CfgB::PATH, CfgB::NAME)).unwrap();
    let secret = read(&secret_file(CfgB::PATH, CfgB::NAME)).unwrap();
    assert!(main.contains(PLACEHOLDER), "token should still be a placeholder: {main}");
    assert!(secret.contains("sk-abc"), "token should still be in secrets: {secret}");
}

// ---------------------------------------------------------------------------
// 加载边界
// ---------------------------------------------------------------------------

#[test]
fn load_with_placeholder_but_missing_secret_file_does_not_panic() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { token: String, name: String }
    impl Config for Cfg {
        const PATH: &'static str = "load_with_placeholder_but_missing_secret_file_does_not_panic";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["token"];
    }

    let cfg = Cfg { token: "sk-abc".into(), name: "alice".into() };
    Cfg::create_file(Some(&cfg)).unwrap();

    std::fs::remove_file(secret_file(Cfg::PATH, Cfg::NAME)).unwrap();

    // 主文件里还是占位符，secrets 文件没了 → 不应 panic，
    // 反序列化得到的就是占位符字符串本身。
    let loaded = Cfg::load_file(false).unwrap();
    assert_eq!(loaded.name, "alice");
    assert_eq!(loaded.token, PLACEHOLDER);
}

#[test]
fn load_overrides_placeholder_with_secret_value() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { token: String, name: String }
    impl Config for Cfg {
        const PATH: &'static str = "load_overrides_placeholder_with_secret_value";
        const NAME: &'static str = "config";
        const SECRETS: &'static [&'static str] = &["token"];
    }

    let cfg = Cfg { token: "sk-abc".into(), name: "alice".into() };
    Cfg::create_file(Some(&cfg)).unwrap();

    // 主文件落盘的是占位符
    assert!(read(&main_file(Cfg::PATH, Cfg::NAME)).unwrap().contains(PLACEHOLDER));

    // 加载后被 secrets 覆盖为真值
    assert_eq!(Cfg::load_file(false).unwrap().token, "sk-abc");
}

// ---------------------------------------------------------------------------
// 根目录覆写（ROOT）与显式根目录（*_at）
// ---------------------------------------------------------------------------

#[test]
fn root_const_override_writes_to_own_directory() {
    setup();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Cfg { name: String }
    impl Config for Cfg {
        const ROOT: Option<&'static str> = Some("root_override_dir");
        const PATH: &'static str = "sub";
        const NAME: &'static str = "config";
    }

    let cfg = Cfg { name: "own-root".into() };
    Cfg::save_file(&cfg).unwrap();

    // 落在 ROOT 覆写的目录，而不是全局根目录
    let in_override = std::path::Path::new("root_override_dir").join("sub").join("config.toml");
    assert!(in_override.exists());
    let global = config_path().unwrap().join("sub").join("config.toml");
    assert!(!global.exists());

    // 从覆写目录加载回来
    assert_eq!(Cfg::load_file(false).unwrap().name, "own-root");

    std::fs::remove_dir_all("root_override_dir").unwrap();
}

#[test]
fn explicit_root_at_apis_do_not_need_init() {
    // 该测试故意不调用 setup()：*_at API 不依赖 init_config_path。
    // 但 OnceLock 是进程级的，其他测试可能已初始化；这里用独立临时目录验证语义即可。
    let tmp = std::env::temp_dir().join(format!(
        "qexed_config_at_test_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&tmp).unwrap();

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct CfgAt { name: String }
    impl Config for CfgAt {
        const PATH: &'static str = "deep/dir";
        const NAME: &'static str = "config";
    }

    let cfg = CfgAt { name: "at-api".into() };
    CfgAt::save_file_at(&tmp, &cfg).unwrap();

    let expected = tmp.join("deep/dir").join("config.toml");
    assert!(expected.exists());
    assert_eq!(CfgAt::load_file_at(&tmp, false).unwrap().name, "at-api");

    // load_and_create_default_at 在文件缺失时自动创建
    std::fs::remove_dir_all(&tmp).unwrap();
    let loaded = CfgAt::load_and_create_default_at(&tmp, false).unwrap();
    assert_eq!(loaded, CfgAt::default());
    assert!(tmp.join("deep/dir").join("config.toml").exists());

    std::fs::remove_dir_all(&tmp).unwrap();
}
