use wasmtime::{Linker, Caller};
use anyhow::Result;

pub fn env(linker: &mut Linker<()>) -> Result<()> {
    // 使用静态引用或零大小类型
    let logger = Logger;
    
    linker.func_wrap("log", "info", move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
        logger.log(caller, ptr, len, 1)
    })?;
    
    linker.func_wrap("log", "warning", move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
        logger.log(caller, ptr, len, 2)
    })?;
    
    linker.func_wrap("log", "debug", move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
        logger.log(caller, ptr, len, 0)
    })?;
    
    linker.func_wrap("log", "error", move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
        logger.log(caller, ptr, len, 3)
    })?;
    
    Ok(())
}

// 为 Logger 实现 Copy
#[derive(Clone, Copy)]
struct Logger;
impl Logger {
    fn log(self, mut caller: Caller<'_, ()>, ptr: i32, len: i32, level: u8) -> i32 {
        match self.read_string(&mut caller, ptr, len) {
            Ok(message) => {
                self.output(&message, level);
                0
            }
            Err(err_code) => {
                eprintln!("[{}] 日志记录失败: 错误码 {}", level, err_code);
                err_code
            }
        }
    }
    
    fn read_string(&self, caller: &mut Caller<'_, ()>, ptr: i32, len: i32) -> Result<String, i32> {
        // 参数验证
        if ptr < 0 || len < 0 {
            return Err(-1);
        }
        
        // 获取内存 - 使用 reborrow 避免移动 caller
        let memory = match caller.get_export("memory") {
            Some(wasmtime::Extern::Memory(mem)) => mem,
            _ => return Err(-2),
        };
        
        // 边界检查
        let data_size = memory.data_size(caller);
        if (ptr as usize) + (len as usize) > data_size {
            return Err(-3);
        }
        
        // 读取内存
        let mut buffer = vec![0u8; len as usize];
        if let Err(_) = memory.read(caller, ptr as usize, &mut buffer) {
            return Err(-4);
        }
        
        // 转换为字符串
        String::from_utf8(buffer).map_err(|_| -5)
    }
    
    fn output(&self, message: &str, level: u8) {
        match level {
            3 => eprintln!("[ERROR] {}", message),
            2 => eprintln!("[WARN] {}", message),
            1 => println!("[INFO] {}", message),
            0 => println!("[DEBUG] {}", message),
            _ => {}
        }
    }
}