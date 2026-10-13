//! 原生加载集成测试：把 qexed-plugin-sample 的 cdylib 产物拷进临时插件目录，
//! 走完整 init() 流程验证 dll/so 加载、on_enable 与事件订阅真实生效。
//!
//! 该测试依赖 `cargo test` 同批构建的样例插件产物（运行前先构建一次样例包，
//! 或直接 `cargo test --workspace` 让 cargo 自行排布构建顺序后运行）。

use std::path::PathBuf;

fn sample_dylib_path() -> Option<PathBuf> {
    // 产物位置跟随 target 目录：优先 CARGO_TARGET_DIR（CI/本仓常用重定向），
    // 回退 workspace 默认 <root>/target。cdylib 落在 <target>/<profile>/ 下。
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
        candidates.push(PathBuf::from(&target).join("debug"));
        candidates.push(PathBuf::from(&target).join("release"));
    }
    let manifest = env!("CARGO_MANIFEST_DIR");
    candidates.push(PathBuf::from(manifest).join("../../target/debug"));
    candidates.push(PathBuf::from(manifest).join("../../target/release"));
    for c in candidates {
        let dir = PathBuf::from(manifest).join(c);
        let names = if cfg!(windows) {
            vec!["qexed_plugin_sample.dll", "qexed-plugin-sample.dll"]
        } else if cfg!(target_os = "macos") {
            vec!["libqexed_plugin_sample.dylib", "libqexed-plugin-sample.dylib"]
        } else {
            vec!["libqexed_plugin_sample.so", "libqexed-plugin-sample.so"]
        };
        for name in names {
            let p = dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

#[tokio::test]
async fn loads_sample_dylib_and_fires_join_event() {
    let Some(dylib) = sample_dylib_path() else {
        eprintln!("sample cdylib 未构建，跳过（先 cargo build -p qexed-plugin-sample）");
        return;
    };

    // 独立临时目录模拟服务端 plugins/
    let dir = std::env::temp_dir().join(format!("qexed_plugin_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join(dylib.file_name().unwrap());
    std::fs::copy(&dylib, &dest).unwrap();

    // 测试进程内的全局状态是首份：set + init 幂等可控
    qexed_plugin::set_plugins_path(&dir).expect("set plugins path");
    qexed_plugin::init().await.expect("plugin init");

    let mgr = qexed_plugin::try_plugin_manager().expect("manager");
    let summaries = mgr.plugin_summaries();
    assert_eq!(summaries, vec!["Sample Greeter v0.1.0".to_string()]);

    // 玩家进服事件：handler 只打日志，不应报错/取消
    let info = qexed_plugin_api::PlayerInfo::new(
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Notch",
        0,
        "minecraft:overworld",
    );
    let result = mgr.fire(&qexed_plugin_api::PlayerJoinEvent::new(info)).await;
    assert!(result.is_ok(), "join handler errors: {:?}", result.errors);
    assert!(!result.is_cancelled());

    // 聊天事件：SpamFilter 对含 "spam" 的消息 cancel
    let result = mgr
        .fire(&qexed_plugin_api::PlayerChatEvent::new(
            qexed_plugin_api::PlayerInfo::new("u", "Notch", 0, "minecraft:overworld"),
            "buy my spam",
        ))
        .await;
    assert!(result.is_cancelled(), "spam message should be cancelled");

    let result = mgr
        .fire(&qexed_plugin_api::PlayerChatEvent::new(
            qexed_plugin_api::PlayerInfo::new("u", "Notch", 0, "minecraft:overworld"),
            "hello world",
        ))
        .await;
    assert!(!result.is_cancelled());

    // 关闭：on_disable → 卸库（验证逆序卸载路径）
    qexed_plugin::shutdown().await;
    assert!(mgr.plugin_summaries().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}