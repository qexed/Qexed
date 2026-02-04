use anyhow::Result;
use wasmtime::{Caller, Linker};

pub fn env(linker: &mut Linker<()>) -> Result<()> {
    // 使用静态引用或零大小类型
    linker.func_wrap(
        "log",
        "Trace",
        move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
            log::trace!("{}", load_string(caller, ptr, len)?);
            Ok(())
        },
    )?;
    linker.func_wrap(
        "log",
        "Info",
        move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
            log::info!("{}", load_string(caller, ptr, len)?);
            Ok(())
        },
    )?;
    linker.func_wrap(
        "log",
        "Debug",
        move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
            log::debug!("{}", load_string(caller, ptr, len)?);
            Ok(())
        },
    )?;
    linker.func_wrap(
        "log",
        "Warn",
        move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
            log::warn!("{}", load_string(caller, ptr, len)?);
            Ok(())
        },
    )?;
    linker.func_wrap(
        "log",
        "Error",
        move |caller: Caller<'_, ()>, ptr: i32, len: i32| {
            log::error!("{}", load_string(caller, ptr, len)?);
            Ok(())
        },
    )?;

    Ok(())
}
fn load_string(mut caller: Caller<'_, ()>, ptr: i32, len: i32) -> anyhow::Result<String> {
    let memory = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => {
            log::error!("无法获取WASM内存");
            return Err(anyhow::anyhow!("无法获取WASM内存"));
        }
    };

    // 从WASM内存中读取字符串
    let mut buffer = vec![0u8; len as usize];
    if memory.read(&caller, ptr as usize, &mut buffer).is_err() {
        log::error!("读取WASM内存失败");
        return Err(anyhow::anyhow!("读取WASM内存失败"));
    }

    // 将字节转换为字符串（这里假设是UTF-8）
    match String::from_utf8(buffer) {
        Ok(s) => {
            // 写入标准输出
            return Ok(s);
        }
        Err(e) => {
            log::error!("{}", e);
            return Err(e.into());
        }
    }
}
