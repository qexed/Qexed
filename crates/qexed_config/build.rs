use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    write_build_info().expect("failed to write build info");

    if std::env::var("TARGET").unwrap().contains("windows") {
        compile_qexed_config_to_mdx_resource().expect("无法编译 Windows 资源文件");
    }
}

fn write_build_info() -> io::Result<()> {
    print_git_rerun_paths();
    println!("cargo:rerun-if-changed=Cargo.toml");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let commit_hash = run_command("git", &["rev-parse", "HEAD"])
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let build_time = run_command("git", &["show", "-s", "--format=%cI", "HEAD"])
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    fs::write(
        out_dir.join("build_info.rs"),
        format!(
            r#"
pub mod build {{
    pub const COMMIT_HASH: &str = "{}";
    pub const BUILD_TIME: &str = "{}";
}}

pub const COMMIT_HASH: &str = build::COMMIT_HASH;
pub const BUILD_TIME: &str = build::BUILD_TIME;
"#,
            commit_hash.trim(),
            build_time.trim()
        ),
    )
}

fn print_git_rerun_paths() {
    if let Ok(head_path) = run_command("git", &["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={}", head_path.trim());
    }

    if let Ok(head_ref) = run_command("git", &["symbolic-ref", "-q", "HEAD"]) {
        let head_ref = head_ref.trim();
        if !head_ref.is_empty() {
            if let Ok(ref_path) = run_command("git", &["rev-parse", "--git-path", head_ref]) {
                println!("cargo:rerun-if-changed={}", ref_path.trim());
            }
        }
    }

    if let Ok(packed_refs) = run_command("git", &["rev-parse", "--git-path", "packed-refs"]) {
        println!("cargo:rerun-if-changed={}", packed_refs.trim());
    }
}

fn run_command(cmd: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(cmd).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{cmd} failed with status {}", output.status),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
