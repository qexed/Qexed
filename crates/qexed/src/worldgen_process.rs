use std::{
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct WorldgenProcess {
    child: Child,
    endpoint: String,
}

impl WorldgenProcess {
    pub fn spawn() -> anyhow::Result<Self> {
        let port = free_local_port()?;
        let project_dir = worldgen_project_dir()?;
        compile_worldgen(&project_dir)?;
        let mut command = java_command(&project_dir)?;
        command
            .arg("dev.qexed.worldgen.WorldgenServer")
            .arg(port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let mut process = Self {
            child: command.spawn().map_err(|err| {
                anyhow::anyhow!(
                    "failed to start Java worldgen service in {}: {err}",
                    project_dir.display()
                )
            })?,
            endpoint: format!("http://127.0.0.1:{port}/"),
        };
        process.wait_until_ready(Duration::from_secs(30))?;
        Ok(process)
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn wait_until_ready(&mut self, timeout: Duration) -> anyhow::Result<()> {
        let port = self
            .endpoint
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse::<u16>()?;
        let started = Instant::now();
        while started.elapsed() < timeout {
            if let Some(status) = self.child.try_wait()? {
                anyhow::bail!("Java worldgen service exited during startup: {status}");
            }
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        anyhow::bail!("Java worldgen service did not become ready in {timeout:?}");
    }
}

impl Drop for WorldgenProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn free_local_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn gradle_command(project_dir: &Path) -> Command {
    let gradlew = project_dir.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    });
    if gradlew.exists() {
        Command::new(gradlew)
    } else if cfg!(windows) && Path::new("C:/gradle/gradle-9.5.1/bin/gradle.bat").exists() {
        Command::new("C:/gradle/gradle-9.5.1/bin/gradle.bat")
    } else {
        Command::new("gradle")
    }
}

fn compile_worldgen(project_dir: &Path) -> anyhow::Result<()> {
    let output = gradle_command(project_dir)
        .arg("compileJava")
        .arg("runtimeClasspathCopy")
        .current_dir(project_dir)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to compile Java worldgen service in {}: {err}",
                project_dir.display()
            )
        })?;
    if !output.status.success() {
        anyhow::bail!(
            "Java worldgen service compile failed: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn java_command(project_dir: &Path) -> anyhow::Result<Command> {
    let classpath = worldgen_classpath(project_dir)?;
    let mut command = Command::new("java");
    command.arg("-cp").arg(classpath);
    command.current_dir(project_dir);
    Ok(command)
}

fn worldgen_classpath(project_dir: &Path) -> anyhow::Result<String> {
    let classes = project_dir.join("build/classes/java/main");
    let dependencies = project_dir.join("build/runtime-libs");
    let mut entries = vec![classes];
    if dependencies.exists() {
        for entry in std::fs::read_dir(&dependencies)? {
            let path = entry?.path();
            if path.extension().is_some_and(|extension| extension == "jar") {
                entries.push(path);
            }
        }
    } else {
        let cache = project_dir.join("build");
        for path in jar_files(&cache)? {
            entries.push(path);
        }
    }

    let separator = if cfg!(windows) { ";" } else { ":" };
    Ok(entries
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(separator))
}

fn jar_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    if !root.exists() {
        return Ok(result);
    }
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            result.extend(jar_files(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "jar") {
            result.push(path);
        }
    }
    Ok(result)
}

fn worldgen_project_dir() -> anyhow::Result<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("failed to resolve workspace root"))?;
    Ok(root.join("tools").join("qexed-vanilla-worldgen"))
}
