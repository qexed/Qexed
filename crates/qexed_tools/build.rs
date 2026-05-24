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

    let attrs = tauri_build::Attributes::new()
        .app_manifest(app_manifest)
        .windows_attributes(
            tauri_build::WindowsAttributes::new().window_icon_path("../qexed/logo.ico"),
        );

    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
