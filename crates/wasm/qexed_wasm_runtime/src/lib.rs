// src/main.rs
use wasmtime::{Engine, Instance, Module, Store, Memory, MemoryType, Linker, Extern, Val};
use wasmtime::AsContextMut;
use std::error::Error;

fn main() -> anyhow::Result<()> { 
    // 1. 初始化 WASM 引擎
    let engine = Engine::default();
    
    // 2. 创建链接器，用于定义WASM可以调用的主机函数
    let mut linker = Linker::new(&engine);
    
    // 3. 为WASM模块定义write函数
    linker.func_wrap("env", "write", |mut caller: wasmtime::Caller<'_, ()>, fd: i32, buf_ptr: i32, len: i32| {
        // 获取WASM模块的内存
        let memory = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => {
                eprintln!("无法获取WASM内存");
                return -1;
            }
        };
        
        // 从WASM内存中读取字符串
        let mut buffer = vec![0u8; len as usize];
        if memory.read(&caller, buf_ptr as usize, &mut buffer).is_err() {
            eprintln!("读取WASM内存失败");
            return -1;
        }
        
        // 将字节转换为字符串（这里假设是UTF-8）
        match String::from_utf8(buffer) {
            Ok(s) => {
                // 写入标准输出
                print!("{}", s);
                0 // 成功返回0
            }
            Err(e) => {
                eprintln!("字符串编码错误: {}", e);
                -1
            }
        }
    })?;
    
    // 4. 加载 WASM 模块
    let wasm_bytes = include_bytes!("a.wasm");
    let module = Module::from_binary(&engine, wasm_bytes)?;
    
    // 5. 创建存储
    let mut store = Store::new(&engine, ());
    
    // 6. 实例化模块，使用链接器
    let instance = linker.instantiate(&mut store, &module)?;
    
    // 7. 获取 print 函数
    let print_func = instance.get_typed_func::<(), ()>(&mut store, "print")?;
    
    // 8. 调用 print 函数
    println!("准备调用 WASM 模块的 print 函数...");
    print_func.call(&mut store, ())?;
    
    println!("\n成功调用了 WASM 模块的 print 函数!");
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn test() -> anyhow::Result<()> {
        main()
    }
}