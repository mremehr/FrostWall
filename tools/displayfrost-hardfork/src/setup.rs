use crate::util;
use std::process::{Command, ExitCode};

struct CheckResult {
    name: &'static str,
    ok: bool,
    details: String,
}

pub fn run(apply: bool) -> ExitCode {
    println!("DisplayFrost setup");
    println!("Mode: {}\n", if apply { "apply" } else { "check" });

    let wf = util::command_exists("wf-recorder");
    let ffmpeg = util::command_exists("ffmpeg");
    let avahi_browse = util::command_exists("avahi-browse");
    let wpa_cli = util::command_exists("wpa_cli");
    let nmcli = util::command_exists("nmcli");
    let iw = util::command_exists("iw");
    let rust_caster = util::command_exists("rust_caster");
    let catt = util::command_exists("catt");
    let python_cast_backend = util::shell_status_ok(
        "command -v python3 >/dev/null 2>&1 && python3 -c 'import pychromecast' >/dev/null 2>&1",
    );
    let systemctl = util::command_exists("systemctl");
    let avahi_active = if systemctl {
        Some(util::shell_status_ok(
            "systemctl is-active --quiet avahi-daemon",
        ))
    } else {
        None
    };

    let checks = vec![
        CheckResult {
            name: "wf-recorder",
            ok: wf,
            details: "capture binary".to_string(),
        },
        CheckResult {
            name: "ffmpeg",
            ok: ffmpeg,
            details: "transcoder binary".to_string(),
        },
        CheckResult {
            name: "avahi-browse",
            ok: avahi_browse,
            details: "Chromecast discovery tool".to_string(),
        },
        CheckResult {
            name: "wpa_cli",
            ok: wpa_cli,
            details: "native Miracast P2P control (optional)".to_string(),
        },
        CheckResult {
            name: "nmcli",
            ok: nmcli,
            details: "NetworkManager control (optional)".to_string(),
        },
        CheckResult {
            name: "iw",
            ok: iw,
            details: "wireless capability probe (optional)".to_string(),
        },
        CheckResult {
            name: "avahi-daemon",
            ok: avahi_active.unwrap_or(false),
            details: if systemctl {
                "system service".to_string()
            } else {
                "systemctl unavailable".to_string()
            },
        },
        CheckResult {
            name: "rust_caster",
            ok: rust_caster,
            details: "Rust cast controller (optional)".to_string(),
        },
        CheckResult {
            name: "catt",
            ok: catt,
            details: "Python cast CLI fallback (optional)".to_string(),
        },
        CheckResult {
            name: "python+pychromecast",
            ok: python_cast_backend,
            details: "Python cast backend (optional)".to_string(),
        },
    ];

    for check in checks {
        let mark = if check.ok { "OK" } else { "MISS" };
        println!("{mark:4} {:16} ({})", check.name, check.details);
    }
    println!();

    if apply {
        if avahi_active == Some(false) {
            if is_root() {
                println!("Applying: starting avahi-daemon...");
                match Command::new("systemctl")
                    .args(["enable", "--now", "avahi-daemon"])
                    .status()
                {
                    Ok(status) if status.success() => println!("OK   avahi-daemon started"),
                    Ok(status) => println!(
                        "MISS could not start avahi-daemon (exit code {})",
                        status.code().unwrap_or(-1)
                    ),
                    Err(err) => println!("MISS could not run systemctl: {err}"),
                }
            } else {
                println!("INFO avahi-daemon requires sudo; run:");
                println!("     sudo systemctl enable --now avahi-daemon");
            }
        }

        if !wf || !ffmpeg || !avahi_browse {
            println!("INFO install missing system packages with:");
            println!("     {}", package_install_hint());
        }
        if !rust_caster {
            println!("INFO optional rust cast backend:");
            println!(
                "     cargo install --git https://github.com/azasypkin/rust-cast --example rust_caster"
            );
        }
        if !catt {
            println!("INFO optional catt backend:");
            println!("     {}", catt_install_hint());
        }
        if !python_cast_backend {
            println!("INFO optional python cast backend:");
            println!("     {}", pychromecast_install_hint());
        }
        if !wpa_cli || !nmcli || !iw {
            println!("INFO optional native Miracast prerequisites:");
            println!("     {}", miracast_native_install_hint());
        }

        println!("\nRe-run checks:");
        println!("  displayfrost doctor");
        println!("  displayfrost setup");
        return ExitCode::SUCCESS;
    }

    if !wf || !ffmpeg || !avahi_browse || avahi_active == Some(false) {
        println!("Next step:");
        println!("  displayfrost setup --apply");

        if avahi_active == Some(false) {
            println!("  sudo systemctl enable --now avahi-daemon");
        }

        if !wf || !ffmpeg || !avahi_browse {
            println!("  {}", package_install_hint());
        }
        if !rust_caster {
            println!(
                "  cargo install --git https://github.com/azasypkin/rust-cast --example rust_caster"
            );
        }
        if !catt {
            println!("  {}", catt_install_hint());
        }
        if !python_cast_backend {
            println!("  {}", pychromecast_install_hint());
        }
        if !wpa_cli || !nmcli || !iw {
            println!("  {}", miracast_native_install_hint());
        }

        ExitCode::from(1)
    } else {
        println!("Environment looks ready.");
        if !rust_caster {
            println!(
                "Optional: cargo install --git https://github.com/azasypkin/rust-cast --example rust_caster"
            );
        }
        if !catt {
            println!("Optional: {}", catt_install_hint());
        }
        if !python_cast_backend {
            println!("Optional: {}", pychromecast_install_hint());
        }
        if !wpa_cli || !nmcli || !iw {
            println!("Optional: {}", miracast_native_install_hint());
        }
        ExitCode::SUCCESS
    }
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

fn package_install_hint() -> String {
    if util::command_exists("pacman") {
        return "sudo pacman -S --needed wf-recorder ffmpeg avahi".to_string();
    }
    if util::command_exists("apt") {
        return "sudo apt install wf-recorder ffmpeg avahi-daemon avahi-utils".to_string();
    }
    if util::command_exists("dnf") {
        return "sudo dnf install wf-recorder ffmpeg avahi avahi-tools".to_string();
    }
    "Install wf-recorder, ffmpeg, and avahi tools with your package manager".to_string()
}

fn catt_install_hint() -> String {
    if util::command_exists("pipx") {
        return "pipx install catt".to_string();
    }
    if util::command_exists("pip3") {
        return "pip3 install --user catt".to_string();
    }
    if util::command_exists("python3") {
        return "python3 -m pip install --user catt".to_string();
    }
    "Install catt with pipx or pip".to_string()
}

fn pychromecast_install_hint() -> String {
    if util::command_exists("pip3") {
        return "pip3 install --user pychromecast".to_string();
    }
    if util::command_exists("python3") {
        return "python3 -m pip install --user pychromecast".to_string();
    }
    "Install python3 and pychromecast".to_string()
}

fn miracast_native_install_hint() -> String {
    if util::command_exists("pacman") {
        return "sudo pacman -S --needed wpa_supplicant networkmanager iw".to_string();
    }
    if util::command_exists("apt") {
        return "sudo apt install wpasupplicant network-manager iw".to_string();
    }
    if util::command_exists("dnf") {
        return "sudo dnf install wpa_supplicant NetworkManager iw".to_string();
    }
    "Install wpa_cli, nmcli and iw with your package manager".to_string()
}
