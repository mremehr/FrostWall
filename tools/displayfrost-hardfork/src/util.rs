use std::path::PathBuf;
use std::process::Command;

pub fn command_exists(command: &str) -> bool {
    if command == "rust_caster" && cargo_bin_exists(command) {
        return true;
    }
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn cargo_bin_exists(command: &str) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    std::path::Path::new(&home)
        .join(".cargo/bin")
        .join(command)
        .is_file()
}

pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    let path = Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {name}"))
        .output()
        .ok()?;
    if !path.status.success() {
        return None;
    }
    let resolved = String::from_utf8_lossy(&path.stdout).trim().to_string();
    if resolved.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(resolved);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

pub fn shell_status_ok(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
