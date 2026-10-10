#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Mojang 数据缓存未就绪 / 缓存读取失败，由 qexed_mojang_data 抛出。
    #[error("Mojang 数据缓存错误: {0}")]
    MojangData(#[from] qexed_mojang_data::error::MojangDataError),

    #[error("读取目录失败 {path}: {source}")]
    ReadDir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("读取 JSON 文件失败 {path}: {source}")]
    ReadJsonFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON 格式错误 {path}: {source}")]
    JsonSyntax {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("路径前缀错误: {0}")]
    Path(#[from] std::path::StripPrefixError),

    #[error("JSON 数字超出 NBT long 范围: {0}")]
    NbtNumberOverflow(String),

    #[error("JSON float 不是有限数: {0}")]
    NbtFloatNotFinite(f32),

    #[error("不支持的 JSON 数字: {0}")]
    UnsupportedJsonNumber(String),

    #[error("注册表报告根节点不是对象")]
    RegistryReportNotObject,

    #[error("未在静态注册表报告中找到注册表: {0}")]
    RegistryNotFound(String),

    #[error("标签文件缺少 values 数组: {0}")]
    TagMissingValues(String),

    #[error("标签引用形成循环: {0}")]
    TagCycle(String),

    #[error("无法补全 minecraft:damage_type/minecraft:is_fire：damage_type 注册表缺少火焰伤害类型")]
    DamageTypeFireTagMissing,
}