# Qexed Tools

Qexed 的工具集合。GUI 版本已切换为 Tauri，实现方式是内联 HTML + `window.__TAURI__.core.invoke`，不再走本地 HTTP 服务。

## 可执行文件

- `qexed_favicon_cli` / `qexed_favicon_gui`
  - PNG 转 Qexed `server.favicon`
- `qexed_code_of_conduct_cli` / `qexed_code_of_conduct_gui`
  - 编辑 `enable-code-of-conduct/<语言>.txt`，并切换 `[server].code_of_conduct`
- `qexed_plugin_manager_cli` / `qexed_plugin_manager_gui`
  - 管理 `plugins` 目录下的 `.wasm` 插件
- `qexed_config_to_mdx_gui`
  - 可视化调用 `qexed_config_to_mdx`
- `qexed_installer_cli` / `qexed_installer_gui`
  - 创建和解压轻量安装包 `.qxpack`

## Tauri 约定

- 主窗口 capability: `capabilities/main.json`
- GUI 页面通过自定义协议 `qexed-tools://localhost/` 提供
- 运行时命令由 `build.rs` 显式登记到 Tauri ACL
