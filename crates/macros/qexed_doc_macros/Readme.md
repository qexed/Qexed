# qexed doc macros

`Doc` / `DocValue` 派生宏：从 doc comment 中的 ```autodoc``` 块生成文档站 schema、TypeScript（可写）与 Rust 值校验。

## autodoc 块语法

```rust
#[derive(qexed_doc_macros::Doc)]
/// ```autodoc
/// <Name>qexed.crates.log.config.LogConfig</Name>
/// <Attr name="writable" />
/// ```
pub struct LogConfig {
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.level</Name>
    /// <Value name="system">LogLevel::default()</Value>
    /// <Default>LogLevel::Debug</Default>
    /// ```
    pub level: LogLevel,
}
```

**属性值规范**：`<xxx aaa=bbb>` 里的 `bbb` 如果是普通字符串，**必须带引号**：
  `aaa="bbb"`（双引号或单引号均可）。裸值如 `<Value name=system>` 现在会编译报错。

- `<Name>`：i18n 翻译键（rust_i18n 格式 locale 文件里对应条目）。
- `<Value name="x">expr</Value>`：i18n 插值来源。expr 是 Rust 表达式，
  在文档生成时**执行**并序列化进 schema 的 `values[x]`，
  供模板里的 `%{x}` 占位符替换（可写多个）。
- `<Default>expr</Default>`：字段默认值，**与翻译键无关**。
  未定义时回退为第一个 `<Value>` 的执行结果。输出的是执行结果，
  不是表达式原文。
- `<Attr name="writable" />`：可写声明。结构体级声明字段默认继承，
  字段级声明可覆盖。

## 生成的产物

- `schema()` / `schema_json()`：`{ name, key, values, fields }`，
  不含 description —— 文本 = t(key) 模板 + %{name} 插值。
- `typescript()`：TS 源码（类型、接口、默认值常量、validate 函数）。
- `typescript_json()`：同内容的结构化 JSON。
- `validate_struct()` / `validate_json()` / `DocValidate::validate()`：值校验。
- 枚举加 `#[derive(DocValue)]`：注册类型与变体，参与校验与 TS 生成。

## 结构体级块是可选的

被 `<Attr name="sub" />` 引用的类型**不需要**自己的 `<Name>` 块——它的字段各自带块即可；
`with_sub` 会把空 key 回填为宿主字段的 key。

## 值校验补全：<Select> / <Min> / <Max>

块内支持 XML 注释 `<!-- ... -->`（解析前剥离）。

### <Select>：候选值

```rust
/// <Select>
///     <Select.Value>Trace</Select.Value>
///     <Select.Value>Debug</Select.Value>
/// </Select>
```

- schema 输出 `"select": ["Trace", "Debug", ...]`，文档站渲染下拉框；
- **值校验补全**：优先级低于 `<Check>`（自定义函数）和类型自带变体
  （`DocValue` 枚举）；普通类型（如 `String`）没有变体校验时，Select 兜底
  校验值必须 ∈ 候选集，错误信息 `expected one of fast, slow`；
- **可推断**：字段类型是 `DocValue` 枚举时不写 `<Select>` 也自动用变体表
  当候选值；写了就保留用户的。

### <Min> / <Max>：数值边界（含端点）

```rust
/// <Min>1</Min>
/// <Max>65535</Max>
```

schema 输出 `"min": 1.0` / `"max": 65535.0`；校验非数值不触发，数值越界报
`must be >= 1` / `must be <= 65535`。

**可推断**：整数类型（`u8`..`u64`/`i8`..`i64`/`usize`/`isize`，含
`Option<...>`）不写 `<Min>/<Max>` 时按位宽自动给全范围边界（如 `u16` →
`0.0..65535.0`）；写了（哪怕只写一个）就保留用户的，另一个不推。
浮点不推边界。

### <CheckErrorTip>

自定义校验失败提示，替换默认消息（`Check` 路径与内置类型检查路径都生效），
嵌入 TS 字符串前自动转义。

**可推断**：枚举字段没写 `<Check>` 也没写 tip 时，自动生成
`"check_error_tip": "expected one of Trace, Debug, ..."`（与默认校验消息
一致）；用户写了任意一个就保留用户的。非枚举不推。

## 机密字段：<Secret> / <IsPassword> / <Secrets> / 占位符豁免

### 字段级 <Secret />：机密成员（推荐）

```rust
/// <Secret />
pub api_token: String,
```

三合一：

- schema 输出 `"password": true`（文档站脱敏渲染）；
- 字段 serde 名进 schema `secrets` 模式表；
- 类型挂 `app_config` 时自动进 `SECRETS` 常量（保存时抽到 .secrets）；
- 机密字段在主配置文件**可缺省**（加载时从 .secrets 回填），值校验对
  `<stored in .secrets>` 占位符豁免。

### 字段级 <IsPassword />

```rust
/// <IsPassword />
```

只做脱敏标记（`"password": true`），不进 `SECRETS`。

### 结构体级 <Secrets>：嵌套/通配机密模式

嵌套路径与通配模式写这里（换行或逗号分隔）：

```rust
/// <Secrets>
/// token
/// auth.password
/// servers.*.api_key
/// </Secrets>
```

- schema 输出 `"secrets": [...]` 模式表与 `"secret_placeholder": "<stored in .secrets>"`；
- 模式命中的字段自动标 `password: true`（`a.b` 精确路径，`*` 通配单层）；
- **前端必须注意**：用户 secrets 文件里可能有 `SECRETS` 之外的自定义机密字段。
  脱敏集合 = schema `secrets` 模式 ∪ 用户 .secrets 文件实际出现的字段路径，
  两者求并集后再渲染，只信 schema 会漏。

### <stored in .secrets> 占位符豁免

主配置文件里机密字段的值是占位符（`qexed_config` 的约定），不是真实值。
`validate_json` / `validate_json_value` 对占位符值**跳过值校验**
（Select/Min/Max/类型检查都不触发），避免文档站上传预览时误报；
占位符的存在即表示"字段已提供"，缺省检查照常。

## app_config 宏：生成 impl Config（qexed_config_macros）

代替手写 `impl qexed_config::Config`：

```rust
#[qexed_config_macros::app_config("/", "log")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct LogConfig { ... }
```

生成：

```rust
impl qexed_config::Config for LogConfig {
    const PATH: &'static str = "/";
    const NAME: &'static str = "log";
    const SECRETS: &'static [&'static str] = &/* 从 autodoc 标签收集 */[...];
}
```

**SECRETS 不是手写的**：宏读取类型 autodoc 里的机密标签自动收集：

- 字段级 `<Secret />`：裸字段名进模式表（serde rename 后的键名优先）；
- 结构体级 `<Secrets>`：嵌套/通配模式（`auth.password`、`servers.*.api_key`）；
- 两者合并去重，同时进 `SECRETS` 常量与 schema `secrets` 模式表——一处声明。

注意：`app_config` 必须写在 `#[derive(... Doc)]` **上方**：属性宏自上而下
执行，先于 Derive 运行（它只生成 impl Config，不改动 doc）。


## serde 兼容（文档 = 真实行为）

宏直接读 `#[serde(...)]` 属性，文档描述的键名与缺省规则就是反序列化的真实行为：

| serde 属性 | Doc 行为 |
|---|---|
| `rename = "x"` | schema `path` 用 serde 名；校验只认 serde 名 |
| 容器 `rename_all = "..."` | 字段名/变体名按规则转换（snake/kebab/camel/Pascal/SCREAMING/lower/UPPER） |
| `alias = "old"` | schema `aliases` 记录；校验时别名键同样接受 |
| `skip` / `skip_serializing` / `skip_deserializing` | 字段不进文档 |
| `flatten` | 等同 `<Attr name="sub" />`；JSON 校验时子类型直接校验同一对象 |
| `default`（裸） | 默认值 = `T::default()` 求值结果；字段缺省合法 |
| `default = "path"` | 默认值 = `path()` 求值结果；字段缺省合法 |
| 容器 `default` | 所有字段缺省合法 |
| `Option<T>` | `optional: true`，缺省/null 合法，校验内层 `T`；元数据也取内层 |

`DocValue` 枚举同样吃 `rename` / `rename_all` / 变体 `skip`：`VARIANTS` 与
`try_from_name` 用的就是磁盘上的真实取值，Rust 原名反而会被拒绝。

## 嵌套与递归（<Attr name="sub" />）

字段类型是另一个派生了 `Doc` 的结构体时，加 `<Attr name="sub" />` 把子类型的
完整 schema 递归嵌进该字段的 `sub` 字段：

```rust
#[derive(Debug, Serialize, Deserialize, Doc)]
/// ```autodoc
/// <Name>config.qexed_warden.data</Name>
/// ```
pub struct Data {
    /// ```autodoc
    /// <Name>config.qexed_warden.data.mysql</Name>
    /// <Attr name="sub" />
    /// ```
    pub mysql: MysqlConfig,   // MysqlConfig 也派生 Doc
}
```

要点：

- **schema 是树**：`DocField.sub: Option<Box<DocSchema>>` 层层嵌套，无深度限制。
- **flatten()**：`DocSchema::flatten()` 按需拍平成点分路径行列表
  （`mysql` → `mysql.host`、`mysql.port`），结构体行本身保留、带 `sub`，
  对应旧版文档站的扁平 `configFields` 数组。
- **值校验递归**：`validate_struct` / `validate_json` 对 sub 字段委托
  子类型的校验实现，错误信息带路径前缀（如 `mysql: port: expected number`）。
- 子类型自身仍是独立 schema，可单独导出。