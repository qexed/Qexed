use std::{env, fs, path::PathBuf};

const WINDOW_ICON: &[u8] = include_bytes!("../qexed_config/qexed_doc_logo.ico");

fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "plugin_list",
        "plugin_install",
        "plugin_enable",
        "plugin_disable",
        "plugin_remove",
        "install_package",
        "create_package",
        "convert_favicon",
        "apply_favicon",
        "load_code_of_conduct",
        "save_code_of_conduct",
        "generate_config_docs",
    ]);
    let icon_path = write_embedded_window_icon();

    let attrs = tauri_build::Attributes::new()
        .app_manifest(app_manifest)
        .windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path(icon_path));

    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}

fn write_embedded_window_icon() -> PathBuf {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let icon_path = out_dir.join("qexed_tools_logo.ico");
    fs::write(&icon_path, WINDOW_ICON).expect("failed to write embedded qexed tools icon");
    println!("cargo:rerun-if-changed=../qexed_config/qexed_doc_logo.ico");
    icon_path
}
