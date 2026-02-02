// build.rs
use std::env;
use std::fs;
use std::path::PathBuf;
use toml::Value;
use toml::map::Map;

// 默认配置结构体（对应 [package.metadata.log_defaults]）
#[derive(Debug, Default)]
struct DefaultConfig {
    level: String,
    console: bool,
    formatter: String,  // 模板：含 {module} 占位符
    file_path: String,  // 模板：含 {} 占位符
    file_mode: String,  // "DAY"/"HOUR"/"SIZE"
    keep_days: u32,
    max_size: u32,      // SIZE模式专用
    append: bool,
}

// 模块配置结构体（对应 [package.metadata.log_modules.xxx]）
#[derive(Debug)]
struct ModuleConfig {
    original_name: String,  // 配置中的键名（如 "tcp_connect"）
    display_name: String,   // 显示名称（rename 或 original_name）
    level: String,
    console: bool,
    formatter: String,      // 替换 {module} 后的实际格式
    file_path: String,       // 替换 {} 后的实际路径
    file_mode: String,
    keep_days: u32,
    max_size: u32,
    append: bool,
}

fn main() {
    // 1. 解析默认配置
    let defaults = parse_defaults();
    
    // 2. 解析模块配置（含重命名和模板替换）
    let modules = parse_modules(&defaults);
    if modules.is_empty() {
        panic!("错误：未在 Cargo.toml 的 [package.metadata.log_modules] 中配置模块");
    }
    
    // 3. 生成 Rust 初始化代码
    let generated_code = generate_module_code(&modules);
    
    // 4. 写入输出文件（$OUT_DIR/generated_log_config.rs）
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR 环境变量未设置");
    let dest_path = PathBuf::from(out_dir).join("generated_log_config.rs");
    fs::write(&dest_path, generated_code).expect("无法写入生成的代码文件");
    
    // 5. 通知 Cargo 监视配置文件变化
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
}

// 解析 [package.metadata.log_defaults] 配置
fn parse_defaults() -> DefaultConfig {
    let cargo_toml = fs::read_to_string("Cargo.toml").unwrap_or_default();
    
    let value: Value = toml::from_str(&cargo_toml)
        .unwrap_or_else(|_| Value::Table(Map::new()));
    
    // 提取 log_defaults 表
    let defaults_table = value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("log_defaults"))
        .and_then(|d| d.as_table())
        .cloned()
        .unwrap_or_else(|| Map::new());
    
    // 从表中读取值（带默认值）
    DefaultConfig {
        level: get_string(&defaults_table, "level").unwrap_or_else(|| "INFO".to_string()),
        console: get_bool(&defaults_table, "console").unwrap_or(true),
        formatter: get_string(&defaults_table, "formatter")
            .unwrap_or_else(|| "[{module}] {level} {time} {file}:{line} {message}\n".to_string()),
        file_path: get_string(&defaults_table, "file_path")
            .unwrap_or_else(|| "./logs/{}.log".to_string()),
        file_mode: get_string(&defaults_table, "file_mode").unwrap_or_else(|| "DAY".to_string()),
        keep_days: get_int(&defaults_table, "keep_days").map(|i| i as u32).unwrap_or(7),
        max_size: get_int(&defaults_table, "max_size").map(|i| i as u32).unwrap_or(10),
        append: get_bool(&defaults_table, "append").unwrap_or(true),
    }
}

// 解析 [package.metadata.log_modules] 配置（含重命名和模板替换）
fn parse_modules(defaults: &DefaultConfig) -> Vec<ModuleConfig> {
    let cargo_toml = fs::read_to_string("Cargo.toml").unwrap_or_default();
    
    let value: Value = toml::from_str(&cargo_toml)
        .unwrap_or_else(|_| Value::Table(Map::new()));
    
    // 提取 log_modules 表
    let log_modules_table = value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("log_modules"))
        .and_then(|lm| lm.as_table())
        .cloned()
        .unwrap_or_else(|| Map::new());
    
    let mut modules = Vec::new();
    
    // 遍历所有模块配置项
    for (original_name, config_val) in log_modules_table {
        if let Some(config_table) = config_val.as_table() {
            // 1. 处理重命名（优先用 rename 字段，否则用原始名称）
            let display_name = get_string(config_table, "rename")
                .unwrap_or_else(|| original_name.clone());
            
            // 2. 解析基础配置（缺省时用默认值）
            let level = get_string(config_table, "level")
                .unwrap_or_else(|| defaults.level.clone());
            let console = get_bool(config_table, "console")
                .unwrap_or(defaults.console);
            
            // 3. 智能模板替换：formatter 中的 {module} → display_name
            let formatter = get_string(config_table, "formatter")
                .unwrap_or_else(|| defaults.formatter.clone());
            //let formatter = ""
            // 4. 智能模板替换：file_path 中的 {} → display_name
            let path_template = get_string(config_table, "file_path")
                .unwrap_or_else(|| defaults.file_path.clone());
            let file_path = path_template;
            
            // 5. 解析其他配置项
            let file_mode = get_string(config_table, "file_mode")
                .unwrap_or_else(|| defaults.file_mode.clone());
            let keep_days = get_int(config_table, "keep_days")
                .map(|i| i as u32)
                .unwrap_or(defaults.keep_days);
            let max_size = get_int(config_table, "max_size")
                .map(|i| i as u32)
                .unwrap_or(defaults.max_size);
            let append = get_bool(config_table, "append")
                .unwrap_or(defaults.append);
            
            // 6. 添加到模块列表
            modules.push(ModuleConfig {
                original_name,
                display_name,
                level,
                console,
                formatter,
                file_path,
                file_mode,
                keep_days,
                max_size,
                append,
            });
        }
    }
    
    modules
}

// 生成 Rust 初始化代码（包含 module 异步函数）
fn generate_module_code(modules: &[ModuleConfig]) -> String {
    let mut code = String::new();
    
    // 头部：仅添加注释
    code.push_str("// 自动生成的日志配置代码（由 build.rs 生成）\n");
    code.push_str("// 注意：请勿手动编辑此文件\n\n");
    
    // 生成 module 异步函数
    code.push_str("/// 初始化所有日志模块\n");
    code.push_str("/// 参数 level 为全局默认级别（当前未使用，保留扩展）\n");
    code.push_str("pub async fn module() {\n");
    
    // 为每个模块生成 set_mod_option 调用
    for module in modules {
        code.push_str(&format!(
            "    // 模块: {} (原始名: {})\n",
            module.display_name, module.original_name
        ));
        
        // 1. 生成文件模式枚举（DAY/HOUR/SIZE）
        let mode_enum = match module.file_mode.to_uppercase().as_str() {
            "HOUR" => "::tklog::MODE::HOUR",
            "SIZE" => "::tklog::MODE::SIZE",
            _ => "::tklog::MODE::DAY",  // 默认按天轮转
        };
        
        // 2. 生成 fileoption 代码（根据文件模式）
        let file_option = if module.file_mode.to_uppercase() == "SIZE" {
            // SIZE 模式：new_size(路径, 最大MB, 保留文件数, 追加模式)
            format!(
                "Some(::std::boxed::Box::new(::tklog::handle::FileTimeMode::new_size(\n        &format!(\"{}\",&rust_i18n::t!(\"{}\")),\n        {},\n        {},\n        {}\n    )))",
                escape_rust_string(&module.file_path), 
                &module.display_name,
                module.max_size, 
                module.keep_days, 
                module.append
            )
        } else {
            // DAY/HOUR 模式：new(路径, 模式, 保留天数, 追加模式)
            format!(
                "Some(::std::boxed::Box::new(::tklog::handle::FileTimeMode::new(\n        &format!(\"{}\",&rust_i18n::t!(\"{}\")),\n        {},\n        {},\n        {}\n    )))",
                escape_rust_string(&module.file_path), 
                &module.display_name,
                mode_enum, 
                module.keep_days, 
                module.append
            )
        };
        
        // 3. 生成完整配置代码
        code.push_str(&format!(
            "    ::tklog::ASYNC_LOG.set_mod_option(\n        \"{}\",\n        ::tklog::LogOption {{\n",
            module.original_name  // 用原始名作为模块标识
        ));
        code.push_str(&format!("            level: Some(::tklog::LEVEL::{}),\n", match module.level.to_uppercase().as_str() {
            "TRACE" => "Trace",
            "INFO"=>"Info",
            "DEBUG" => "Debug",
            "WARN" => "Warn",
            "ERROR" => "Error",
            "FATAL" => "Fatal",
            "OFF" => "Off",
            _ => "Info",
        }));
        
        code.push_str(&format!("            console: Some({}),\n", module.console));
        code.push_str("            format: None,\n");
        code.push_str(&format!(
            "            formatter: Some(\"{}\".replace(\"{{module}}\", &rust_i18n::t!(\"{}\"))),\n",
            escape_rust_string(&module.formatter),&module.display_name  // 转义特殊字符
        ));
        code.push_str(&format!("            fileoption: {},\n", file_option));
        // code.push_str("        }\n    ).await {\n");
        // code.push_str(&format!(
        //     "        ::std::eprintln!(\"[日志初始化错误] 模块 {} 失败: {{}}\", e);\n",
        //     escape_rust_string(&module.display_name)
        // ));
        code.push_str("    }).await;\n\n");
    }
    
    code.push_str("}\n");
    code
}

// ------------------------------ 辅助函数 ------------------------------

// 从 TOML 表读取值（字符串）
fn get_string(table: &Map<String, Value>, key: &str) -> Option<String> {
    table.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// 从 TOML 表读取值（布尔值）
fn get_bool(table: &Map<String, Value>, key: &str) -> Option<bool> {
    table.get(key)
        .and_then(|v| v.as_bool())
}

// 从 TOML 表读取值（整数）
fn get_int(table: &Map<String, Value>, key: &str) -> Option<i64> {
    table.get(key)
        .and_then(|v| v.as_integer())
}

// 转义 Rust 字符串中的特殊字符（\、"、换行等）
fn escape_rust_string(s: &str) -> String {
    s.replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
}