use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

// schema 不携带 description：文本由 key 查 i18n 得模板，再用 values 做 %{name} 插值。
pub use serde_json;

/// <Tip>/<Warn> 的值载体：i18n 键或直接文本，二者取其一。
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct DocI18nText {
    /// "i18n"（翻译键，文档站经 t(key) 渲染）或 "text"（已定文本，原样渲染）。
    pub kind: I18nTextKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum I18nTextKind {
    I18n,
    Text,
}

/// 启发式分类：与 rust_i18n 键形态一致（点分、无空白、无中文）判为 i18n 键；
/// 其余（含空格、中文、标点句子）判为直接文本。可用 "text:" / "i18n:" 前缀显式覆盖。
pub fn classify_i18n_text(raw: &str) -> DocI18nText {
    let value = raw.trim();
    if let Some(rest) = value.strip_prefix("i18n:") {
        return DocI18nText { kind: I18nTextKind::I18n, value: rest.trim().to_string() };
    }
    if let Some(rest) = value.strip_prefix("text:") {
        return DocI18nText { kind: I18nTextKind::Text, value: rest.trim().to_string() };
    }
    let looks_like_key =
        !value.is_empty() && value.contains('.') && value.split('.').all(|seg| {
            !seg.is_empty()
                && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    let kind = if looks_like_key { I18nTextKind::I18n } else { I18nTextKind::Text };
    DocI18nText { kind, value: value.to_string() }
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct DocField {
    pub path: String,
    /// i18n key（文档站/翻译文件用），例如 qexed.crates.log.config.LogConfig.level
    pub key: String,
    pub value_type: String,
    /// 字段是否可写（文档站表单可编辑提交）。
    pub writable: bool,
    /// autodoc 中 <Value name=x>expr</Value> 的执行结果，供 i18n %{x} 插值。
    pub values: BTreeMap<String, Value>,
    /// <Default> 字面量；未定义时取 <Value> 表达式的执行结果。
    pub default: Option<Value>,
    /// 枚举全部变体；非枚举类型为空。文档站渲染表单与校验用。
    pub variants: Vec<String>,
    /// <Check> 内的 TypeScript 匿名校验函数原文，文档站直接当函数体使用。
    pub check: Option<String>,
    /// 校验失败提示（<CheckErrorTip>），缺省用内置消息。
    pub check_error_tip: Option<String>,
    /// <Attr name=sub /> 标记的嵌套字段：子类型的完整 schema（递归表示）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<Box<DocSchema>>,
    /// #[serde(alias = "...")]：配置文件里等价接受的备用键。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Option 字段：缺省/null 都合法。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[serde(default)]
    pub optional: bool,
    /// #[serde(flatten)]：子字段与父级同级展开。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[serde(default)]
    pub flattened: bool,
    /// <Select> 候选值：文档站下拉框；无 Check/变体时兼任值校验。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub select: Vec<String>,
    /// <Min>/<Max> 数值边界（含端点）。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub min: Option<f64>,
    /// <Max> 数值边界（含端点）。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub max: Option<f64>,
    /// 机密字段（<IsPassword> 或被 SECRETS 模式命中）：文档站脱敏渲染。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[serde(default)]
    pub password: bool,
    /// <Tip>：字段补充说明（i18n 键或直接文本）。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub tip: Option<DocI18nText>,
    /// <Warn>：字段警告（i18n 键或直接文本）。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub warn: Option<DocI18nText>,
    /// 翻译后的字段文本：`t(key)` 渲染模板 + `values` 做 %{name} 插值。
    /// None 表示尚未翻译，由 `qexed_language::translate_schema` 回填。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub document: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct DocSchema {
    pub name: String,
    /// i18n key；无结构体级块时为空串（被 sub 引用时回填宿主字段 key）。
    pub key: String,
    /// <Secrets> 声明的机密字段模式（对应 impl Config 的 SECRETS），
    /// 如 ["token", "auth.password", "servers.*.api_key"]。
    /// 前端需与用户 secrets 文件里的自定义字段求并集后再脱敏。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
    /// 机密值占位符：主配置文件中该字段的非真实内容。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub secret_placeholder: String,
    /// 结构体级 <Value> 求值结果（i18n %{x} 插值用）。
    pub values: BTreeMap<String, Value>,
    pub fields: Vec<DocField>,
    /// 翻译后的 schema 文本：`t(key)` 渲染模板 + `values` 做 %{name} 插值。
    /// None 表示尚未翻译，由 `qexed_language::translate_schema` 回填。
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub document: Option<String>,
}

/// 由 `Doc` 派生实现：从类型取回它的 schema，供嵌套字段递归展开。
pub trait DocSchemaOf {
    fn schema() -> DocSchema;
    fn schema_json() -> String {
        Self::schema().to_json()
    }
}

/// 主配置文件中机密字段的占位符文本（qexed_config::Config 约定）。
pub const SECRET_PLACEHOLDER: &str = "<stored in .secrets>";

impl DocSchema {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 拍平成点分路径行列表（旧版文档格式）：嵌套字段以
    /// `data.mysql.table_prefix` 这样的路径出现，结构体行本身保留。
    pub fn flatten(&self) -> Vec<DocField> {
        let mut out = Vec::new();
        fn walk(prefix: &str, fields: &[DocField], out: &mut Vec<DocField>) {
            for field in fields {
                let mut row = field.clone();
                if !prefix.is_empty() {
                    row.path = format!("{prefix}.{}", field.path);
                }
                let sub = field.sub.clone();
                let row_path = row.path.clone();
                out.push(row);
                if let Some(child) = sub {
                    walk(&row_path, &child.fields, out);
                }
            }
        }
        walk("", &self.fields, &mut out);
        out
    }
}

impl DocField {
    /// 用翻译函数渲染本地化描述：t(key) 返回模板，再按 %{name} 插值。
    pub fn describe<F>(&self, t: F) -> String
    where
        F: Fn(&str) -> String,
    {
        interpolate(&t(&self.key), &self.values)
    }
}

/// rust_i18n 风格 %{name} 插值。
pub fn interpolate(template: &str, values: &BTreeMap<String, Value>) -> String {
    let mut out = template.to_string();
    for (name, value) in values {
        let placeholder = format!("%{{{name}}}");
        let text = match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out = out.replace(&placeholder, &text);
    }
    out
}

/// 值类型元数据：由 `Doc` / `DocValue` 派生自动实现，
/// 供 Rust 侧值校验，并把类型/变体信息写进 schema。
pub trait DocValue {
    /// Rust 类型名，例如 "LogLevel"、"i32"。
    const TYPE: &'static str;
    /// 枚举全部变体；非枚举类型为空。
    const VARIANTS: &'static [&'static str];
    /// schema value_type 展示名。基础类型即 TYPE；容器类型（map 等）
    /// 覆盖此方法递归拼接内层类型名（const 无法泛型拼接，故走函数）。
    fn type_label() -> String {
        Self::TYPE.to_string()
    }
    /// 校验一个可写值。
    fn validate(value: &Value) -> Result<(), String>;
}

/// 结构体级值校验：由 `Doc` 派生自动实现。
pub trait DocValidate {
    fn validate(&self) -> Result<(), String>;
}

macro_rules! doc_value_primitive {
    ($($t:ty => $kind:expr),* $(,)?) => {$(
        impl DocValue for $t {
            const TYPE: &'static str = stringify!($t);
            const VARIANTS: &'static [&'static str] = &[];
            fn validate(value: &Value) -> Result<(), String> {
                let ok = match $kind {
                    "string" => value.is_string(),
                    "number" => value.is_number(),
                    "boolean" => value.is_boolean(),
                    _ => false,
                };
                if ok { Ok(()) } else { Err(format!("expected {}", $kind)) }
            }
        }
    )*};
}

doc_value_primitive!(
    bool => "boolean",
    String => "string",
    i8 => "number", i16 => "number", i32 => "number", i64 => "number",
    isize => "number",
    u8 => "number", u16 => "number", u32 => "number", u64 => "number",
    usize => "number",
    f32 => "number", f64 => "number",
);

/// 泛型字符串键映射：值类型递归套用 DocValue，schema 的 value_type 随之嵌套
/// （如 map<string,string>、map<string,map<string,string>>），可写值校验逐值穿透。
// macro_rules 的 ty 片段拼泛型参数不合法，直接手写两个泛型实现：
impl<V: DocValue> DocValue for std::collections::HashMap<String, V> {
    const TYPE: &'static str = "map";
    const VARIANTS: &'static [&'static str] = V::VARIANTS;
    fn type_label() -> String {
        format!("map<string,{}>", V::type_label())
    }
    fn validate(value: &Value) -> Result<(), String> {
        let Some(obj) = value.as_object() else {
            return Err(format!("expected map<string,{}>", V::TYPE));
        };
        for (k, v) in obj {
            V::validate(v).map_err(|e| format!("map value for {k}: {e}"))?;
        }
        Ok(())
    }
}

impl<V: DocValue> DocValue for std::collections::BTreeMap<String, V> {
    const TYPE: &'static str = "map";
    const VARIANTS: &'static [&'static str] = V::VARIANTS;
    fn type_label() -> String {
        format!("map<string,{}>", V::type_label())
    }
    fn validate(value: &Value) -> Result<(), String> {
        let Some(obj) = value.as_object() else {
            return Err(format!("expected map<string,{}>", V::TYPE));
        };
        for (k, v) in obj {
            V::validate(v).map_err(|e| format!("map value for {k}: {e}"))?;
        }
        Ok(())
    }
}

/// 类型元数据由宏按字段种类传入：普通字段来自 DocValue，嵌套字段为
/// 类型名 + 空变体表（schema 详情在其 sub 里）。
#[doc(hidden)]
pub fn doc_field(
    path: &'static str,
    key: &'static str,
    writable: bool,
    values: BTreeMap<String, Value>,
    default: Option<Value>,
    value_type: String,
    variants: &'static [&'static str],
    check: Option<&'static str>,
    check_error_tip: Option<String>,
    aliases: &'static [&'static str],
    password: bool,
) -> DocField {
    DocField {
        path: path.to_string(),
        key: key.to_string(),
        value_type,
        writable,
        values,
        default,
        variants: variants.iter().map(|v| v.to_string()).collect(),
        check: check.map(|c| c.to_string()),
        check_error_tip,
        sub: None,
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
        optional: false,
        flattened: false,
        select: Vec::new(),
        min: None,
        max: None,
        password,
        tip: None,
        warn: None,
        // 由 qexed_language::translate_schema 在渲染阶段回填。
        document: None,
    }
}

impl DocField {
    /// 标记为嵌套字段，嵌入子类型的 schema（宏在 <Attr name=sub /> 时生成）。
    /// 要求字段类型实现 DocSchemaOf（即派生了 `Doc`）。
    #[doc(hidden)]
    pub fn with_sub(self, schema: DocSchema) -> Self {
        // 子类型没有自己的 <Name> 块时，key 沿用宿主字段的 key。
        let mut this = self;
        let mut schema = schema;
        if schema.key.is_empty() {
            schema.key = this.key.clone();
        }
        this.sub = Some(Box::new(schema));
        this
    }

    /// 标记 Option 字段（宏从类型推导，serde 语义：缺省/null 合法）。
    #[doc(hidden)]
    pub fn with_optional(self, optional: bool) -> Self {
        Self { optional, ..self }
    }

    /// 标记 #[serde(flatten)] 字段。
    #[doc(hidden)]
    pub fn with_flattened(self, flattened: bool) -> Self {
        Self { flattened, ..self }
    }

    /// <Select> 候选值。
    #[doc(hidden)]
    pub fn with_select(self, select: &'static [&'static str]) -> Self {
        Self { select: select.iter().map(|s| s.to_string()).collect(), ..self }
    }

    /// <Min>/<Max> 数值边界（含端点）。
    #[doc(hidden)]
    pub fn with_min_max(self, min: Option<f64>, max: Option<f64>) -> Self {
        Self { min, max, ..self }
    }

    /// <Tip>：i18n 键或直接文本，宏侧已分类。
    #[doc(hidden)]
    pub fn with_tip(self, tip: Option<DocI18nText>) -> Self {
        Self { tip, ..self }
    }

    /// <Warn>：i18n 键或直接文本，宏侧已分类。
    #[doc(hidden)]
    pub fn with_warn(self, warn: Option<DocI18nText>) -> Self {
        Self { warn, ..self }
    }
}

/// 宏生成用：(name, 求值结果) 对转 i18n 插值表。
#[doc(hidden)]
pub fn doc_values(pairs: &[(String, Value)]) -> BTreeMap<String, Value> {
    pairs.iter().cloned().collect()
}

/// 在文档生成时执行 Rust 表达式并序列化，而不是把表达式原文写进文档。
#[doc(hidden)]
pub fn eval_default<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value)
        .unwrap_or_else(|e| panic!("qexed_doc: 无法序列化表达式求值结果: {e}"))
}

/// `<Default>` 字面量转 JSON 值；非 JSON 文本按字符串处理。
#[doc(hidden)]
pub fn default_from_literal(literal: &str) -> Value {
    serde_json::from_str(literal).unwrap_or(Value::String(literal.to_string()))
}

/// 值是否为机密占位符（`<stored in .secrets>`）。
/// 占位符不是真实值：文档站上传预览时跳过值校验，避免占位符误报。
#[doc(hidden)]
pub fn is_secret_placeholder(value: &Value) -> bool {
    value.as_str() == Some(SECRET_PLACEHOLDER)
}

/// 值校验补全：`<Select>` 候选值与 `<Min>/<Max>` 边界。
/// 返回错误消息列表（空 = 通过），由宏拼进字段校验错误。
#[doc(hidden)]
pub fn validate_doc_hints(
    value: &Value,
    select: &[&str],
    min: Option<f64>,
    max: Option<f64>,
) -> Vec<String> {
    let mut errors = Vec::new();
    // Select：字符串值做候选检查（文档站同时拿它渲染下拉框）。
    if !select.is_empty() {
        if let Some(s) = value.as_str() {
            if !select.contains(&s) {
                errors.push(format!("expected one of {}", select.join(", ")));
            }
        }
    }
    // Min/Max：数值边界（含端点）。
    if let Some(n) = value.as_f64() {
        if let Some(min) = min {
            if n < min {
                errors.push(format!("must be >= {min}"));
            }
        }
        if let Some(max) = max {
            if n > max {
                errors.push(format!("must be <= {max}"));
            }
        }
    }
    errors
}