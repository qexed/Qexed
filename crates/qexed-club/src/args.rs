use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
#[command(name = "qexed", about = "Qexed server")]
pub struct ServerArgs {
    /// 初始化配置后退出。
    #[arg(long, default_value_t = false)]
    pub init_settings: bool,

    /// 覆盖配置文件中的语言。
    #[arg(long)]
    pub language: Option<String>,

    /// 指定配置目录。
    #[arg(long)]
    pub config_path: Option<std::path::PathBuf>,

    /// 指定插件目录，相对路径基于 Qexed 可执行文件所在目录。
    #[arg(long, default_value = "plugins")]
    pub plugins_path: std::path::PathBuf,

    /// 输出文档客户端使用的版本和启动参数定义。
    #[arg(long, default_value_t = false)]
    pub doc_api_version: bool,

    /// 传给服务端的其余参数。
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgField {
    pub name: &'static str,
    pub kind: &'static str,
    pub description: &'static str,
    pub optional: bool,
    pub default_value: Option<&'static str>,
}

pub fn arg_fields() -> Vec<ArgField> {
    vec![
        ArgField {
            name: "init-settings",
            kind: "bool",
            description: "初始化配置后退出",
            optional: false,
            default_value: Some("false"),
        },
        ArgField {
            name: "language",
            kind: "string",
            description: "覆盖配置文件中的语言",
            optional: true,
            default_value: None,
        },
        ArgField {
            name: "config-path",
            kind: "path",
            description: "指定配置目录",
            optional: true,
            default_value: None,
        },
        ArgField {
            name: "plugins-path",
            kind: "path",
            description: "指定插件目录，相对路径基于 Qexed 可执行文件所在目录",
            optional: false,
            default_value: Some("plugins"),
        },
        ArgField {
            name: "doc-api-version",
            kind: "bool",
            description: "输出文档客户端使用的版本和启动参数定义",
            optional: false,
            default_value: Some("false"),
        },
    ]
}
