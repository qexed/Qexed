use shadow_rs::ShadowBuilder;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    ShadowBuilder::builder().build().unwrap();

    if std::env::var("TARGET").unwrap().contains("windows") {
        compile_qexed_config_to_mdx_resource().expect("无法编译 Windows 资源文件");
    }
}

fn compile_qexed_config_to_mdx_resource() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let icon = manifest_dir.join("qexed_doc_logo.ico");
    let rc = out_dir.join("qexed_config_to_mdx.rc");
    let res = out_dir.join("qexed_config_to_mdx.res");

    fs::write(
        &rc,
        format!(
            r#"1 ICON "{}"

1 VERSIONINFO
FILEVERSION 0,1,0,0
PRODUCTVERSION 0,1,0,0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "FileDescription", "Qexed Config Docs\0"
            VALUE "ProductName", "Qexed AutoDoc\0"
            VALUE "FileVersion", "0.1.0\0"
            VALUE "ProductVersion", "0.1.0\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#,
            escape_rc_path(&icon)
        ),
    )?;

    let rc_exe = find_rc_exe()?;
    let status = Command::new(&rc_exe)
        .arg(format!("/I{}", manifest_dir.display()))
        .arg(format!("/fo{}", res.display()))
        .arg(&rc)
        .status()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("rc.exe failed: {}", rc_exe.display()),
        ));
    }

    println!("cargo:rerun-if-changed={}", icon.display());
    println!(
        "cargo:rustc-link-arg-bin=qexed_config_to_mdx={}",
        res.display()
    );
    Ok(())
}

fn find_rc_exe() -> io::Result<PathBuf> {
    if let Some(path) = find_program_in_path("rc.exe") {
        return Ok(path);
    }

    let kits_root = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let arch_dir = if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "x86" {
        "x86"
    } else {
        "x64"
    };
    if kits_root.exists() {
        let mut candidates = fs::read_dir(&kits_root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join(arch_dir).join("rc.exe"))
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        candidates.sort();
        if let Some(path) = candidates.pop() {
            return Ok(path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "找不到 Windows SDK rc.exe",
    ))
}

fn find_program_in_path(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(name))
        .find(|path| path.exists())
}

fn escape_rc_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
