pub mod config;
pub mod modern;
// src/main.rs

use std::path::PathBuf;

use wasmtime::{Engine, Linker, Module, Store};
rust_i18n::i18n!("../../../locales");
pub fn new() -> anyhow::Result<()>{
    // log::info!("插件运行环境初始化中");
    // let files = load_plugin_list()?;
    // log::info!("版本依赖检查中");
    Ok(())
}
pub fn load_plugin_config(path:PathBuf)->anyhow::Result<config::Plugin>{
    let engine = Engine::default();
    let mut linker = Linker::new(&engine);
    let module = Module::from_file(&engine, path.clone())?;
    let mut store = Store::new(&engine, ());
    for import in module.imports() {
        let module_name = import.module().to_string(); // 转换为String
        let name = import.name().to_string(); // 转换为String

        // 将import.ty()获取的引用转换为拥有的值
        let ty = import.ty().clone();

        match ty {
            wasmtime::ExternType::Func(func_ty) => {
                // 克隆func_ty以便在闭包中使用
                let func_ty_clone = func_ty.clone();

                // 创建一个存根函数，使用move关键字转移所有权
                let func = wasmtime::Func::new(&mut store, func_ty, move |_, _params, results| {
                    println!("stub function called: {}::{}", module_name, name);
                    // 根据返回类型设置返回值
                    for (i, r) in results.iter_mut().enumerate() {
                        match func_ty_clone.results().nth(i).unwrap() {
                            wasmtime::ValType::I32 => *r = wasmtime::Val::I32(0),
                            wasmtime::ValType::I64 => *r = wasmtime::Val::I64(0),
                            wasmtime::ValType::F32 => *r = wasmtime::Val::F32(0),
                            wasmtime::ValType::F64 => *r = wasmtime::Val::F64(0),
                            _ => panic!("unexpected return type"),
                        }
                    }
                    Ok(())
                });

                linker.define(&store, import.module(), import.name(), func)?;
            }
            _ => {
                // 暂时忽略其他类型的导入
                panic!("unexpected import type: {:?}", ty);
            }
        }
    }
    let instance = linker.instantiate(&mut store, &module)?;
    let plugin_config_func = instance.get_typed_func::<(), u32>(&mut store, "get_config_data")?;
    let plugin_get_config_len= instance.get_typed_func::<(), u32>(&mut store, "get_config_len")?;
    // let plugin_display_name_func = instance.get_typed_func::<*mut c_char, *mut c_char>(&mut store, "get_plugin_display_name")?;
    let data_ptr = plugin_config_func.call(&mut store, ())?;
    let len = plugin_get_config_len.call(&mut store, ())?;
    let memory = instance.get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("memory not found"))?;
    let mut data = vec![0u8; len as usize];
    memory.read(&store, data_ptr as usize, &mut data)?;
    let mut config :config::Plugin = toml::from_slice(&data)?;
    config.path = Some(path);
    Ok(config)
}
