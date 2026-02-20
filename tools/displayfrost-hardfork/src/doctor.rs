use crate::util;
use std::process::ExitCode;

struct Check {
    name: &'static str,
    hint: &'static str,
    required: bool,
    kind: CheckKind,
}

enum CheckKind {
    Command(&'static str),
    ShellStatus(&'static str),
}

const CHECKS: &[Check] = &[
    Check {
        name: "Capture (Wayland)",
        hint: "wf-recorder",
        required: true,
        kind: CheckKind::Command("wf-recorder"),
    },
    Check {
        name: "Transcoding/packaging",
        hint: "ffmpeg",
        required: true,
        kind: CheckKind::Command("ffmpeg"),
    },
    Check {
        name: "Chromecast discovery",
        hint: "avahi-browse",
        required: true,
        kind: CheckKind::Command("avahi-browse"),
    },
    Check {
        name: "Avahi daemon",
        hint: "systemctl is-active avahi-daemon",
        required: false,
        kind: CheckKind::ShellStatus(
            "command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet avahi-daemon",
        ),
    },
    Check {
        name: "PipeWire sanity",
        hint: "pw-cli",
        required: false,
        kind: CheckKind::Command("pw-cli"),
    },
    Check {
        name: "Miracast native control",
        hint: "wpa_cli",
        required: false,
        kind: CheckKind::Command("wpa_cli"),
    },
    Check {
        name: "Miracast network manager",
        hint: "nmcli",
        required: false,
        kind: CheckKind::Command("nmcli"),
    },
    Check {
        name: "Miracast wireless probe",
        hint: "iw",
        required: false,
        kind: CheckKind::Command("iw"),
    },
    Check {
        name: "Rust cast backend",
        hint: "rust_caster",
        required: false,
        kind: CheckKind::Command("rust_caster"),
    },
    Check {
        name: "Catt cast backend",
        hint: "catt",
        required: false,
        kind: CheckKind::Command("catt"),
    },
    Check {
        name: "Python cast backend",
        hint: "python3 + pychromecast",
        required: false,
        kind: CheckKind::ShellStatus(
            "command -v python3 >/dev/null 2>&1 && python3 -c 'import pychromecast' >/dev/null 2>&1",
        ),
    },
];

pub fn run() -> ExitCode {
    println!("DisplayFrost doctor");
    println!("Checking runtime tools for MVP strategy...\n");

    let mut missing_required = false;
    for check in CHECKS {
        let ok = match check.kind {
            CheckKind::Command(command) => util::command_exists(command),
            CheckKind::ShellStatus(cmd) => util::shell_status_ok(cmd),
        };
        let marker = if ok { "OK" } else { "MISS" };
        println!("{marker:4} {:26} ({})", check.name, check.hint);
        if !ok && check.required {
            missing_required = true;
        }
    }

    if missing_required {
        println!("\nRequired tools are missing. Install them before starting MVP implementation.");
        ExitCode::from(1)
    } else {
        println!("\nRequired tools are present.");
        ExitCode::SUCCESS
    }
}
