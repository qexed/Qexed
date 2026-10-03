use std::{process::Command, time::{SystemTime, UNIX_EPOCH}};

fn command_output(args: &[&str]) -> Option<String> {
    let output = Command::new(args[0]).args(&args[1..]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    if text.is_empty() { None } else { Some(text.to_string()) }
}

fn build_date() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0);
    let days = seconds / 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{year:04}-{m:02}-{d:02}")
}

fn main() {
    let hash = command_output(&["git", "rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    let date = command_output(&["git", "show", "-s", "--format=%cs", "HEAD"]).unwrap_or_else(build_date);
    println!("cargo:rustc-env=QEXED_GIT_HASH={hash}");
    println!("cargo:rustc-env=QEXED_BUILD_DATE={date}");
    println!("cargo:rerun-if-changed=build.rs");
}
