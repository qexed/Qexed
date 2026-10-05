/// 一个要导出的配置 schema 条目。
pub struct ExportItem {
    /// 相对 config 根的路径，如 "/" 表示 config 根、"server" 表示 config/server。
    /// 语义与 qexed_config::config_file 的 path 参数一致：前导 '/' 视为相对根。
    pub path: &'static str,
    /// 末段路由名（不含扩展名）。
    pub name: &'static str,
    /// schema JSON 文本（运行时从各配置 crate 拿）。
    pub schema: qexed_doc::DocSchema,
    /// 配置文件自身默认内容
    pub data: String,
}

impl ExportItem {
    pub fn of<T>() -> Result<Self, qexed_config::error::ConfigError>
    where
        T: qexed_config::Config + qexed_doc::DocSchemaOf,
    {
        Ok(Self {
            path: T::PATH,
            name: T::NAME,
            schema: T::schema(),
            data: T::to_toml_string(&T::default())?,
        })
    }
}
