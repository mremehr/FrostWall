mod castv2;
mod chromecast;
mod config;
mod doctor;
mod miracast;
mod report;
mod setup;
mod tui;
mod util;

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "displayfrost",
    version,
    about = "Research-driven baseline for wireless desktop streaming (Chromecast + Miracast)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print the recommended solution strategy for DisplayFrost
    Recommend,
    /// Check local runtime dependencies for the MVP approach
    Doctor,
    /// Print implementation roadmap
    Roadmap,
    /// Print source links used in research
    Sources,
    /// Chromecast workflows
    Chromecast {
        #[command(subcommand)]
        command: Box<ChromecastCommand>,
    },
    /// Miracast workflows (native Rust control plane, experimental)
    Miracast {
        #[command(subcommand)]
        command: MiracastCommand,
    },
    /// Launch terminal UI for device selection and streaming
    Tui,
    /// Guided environment setup and optional auto-fixes
    Setup {
        /// Attempt safe automatic fixes (no package manager commands)
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ChromecastCommand {
    /// List Chromecast devices on the local network
    List,
    /// Start streaming desktop to a Chromecast device
    Start(Box<ChromecastStartArgs>),
    /// Stop media playback on a Chromecast device
    Stop(ChromecastStopArgs),
}

#[derive(Subcommand, Debug)]
enum MiracastCommand {
    /// Discover Miracast peers over Wi-Fi Direct (P2P)
    Discover(MiracastDiscoverArgs),
    /// Start Wi-Fi Direct (P2P) connect flow to a Miracast peer
    Connect(MiracastConnectArgs),
    /// Start native Miracast RTSP control session
    Start(MiracastStartArgs),
    /// Stop native Miracast session (teardown + clear local state)
    Stop,
    /// Show native Miracast session status
    Status,
}

#[derive(Args, Debug)]
struct MiracastStartArgs {
    /// Miracast sink IP address (reachable over current network path)
    #[arg(long, value_name = "HOST")]
    host: String,
    /// Miracast sink RTSP port
    #[arg(long, default_value_t = 7236)]
    rtsp_port: u16,
    /// Connect/read/write timeout for control channel
    #[arg(long, default_value_t = 4)]
    timeout_secs: u64,
    /// Replace existing active local session state
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Args, Debug)]
struct MiracastDiscoverArgs {
    /// Wireless interface for wpa_cli (auto-detected when omitted)
    #[arg(long)]
    interface: Option<String>,
    /// Discovery duration in seconds
    #[arg(long, default_value_t = 8)]
    timeout_secs: u64,
}

#[derive(Args, Debug)]
struct MiracastConnectArgs {
    /// P2P peer device address (from `miracast discover`)
    #[arg(long, value_name = "PEER")]
    peer: String,
    /// Wireless interface for wpa_cli (auto-detected when omitted)
    #[arg(long)]
    interface: Option<String>,
    /// Wait time for connection completion
    #[arg(long, default_value_t = 20)]
    timeout_secs: u64,
    /// PIN to use instead of push-button mode
    #[arg(long)]
    pin: Option<String>,
    /// Flush stale P2P state before connecting
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Args, Debug)]
struct ChromecastStartArgs {
    /// Chromecast friendly name (positional)
    #[arg(value_name = "DEVICE")]
    device_name: Option<String>,
    /// Chromecast friendly name (legacy flag)
    #[arg(long = "device", conflicts_with = "device_name")]
    device: Option<String>,
    /// Wayland output to capture (e.g. DP-1, DP-2)
    #[arg(long)]
    output: Option<String>,
    /// Network interface to source local IP from (optional)
    #[arg(long)]
    interface: Option<String>,
    /// Stream locally only (skip Chromecast discovery/attach)
    #[arg(long, default_value_t = false)]
    stream_only: bool,
    /// Force compatibility video path (1080p/30 re-encode)
    #[arg(long, default_value_t = false)]
    compat_video: bool,
    /// Local HTTP stream port
    #[arg(long, default_value_t = 8888)]
    port: u16,
    /// Capture framerate
    #[arg(long)]
    framerate: Option<u32>,
    /// x264 CRF value
    #[arg(long)]
    crf: Option<u32>,
    /// x264 preset
    #[arg(long)]
    preset: Option<String>,
    /// Encoder strategy
    #[arg(long, value_enum, default_value_t = EncoderArg::Auto)]
    encoder: EncoderArg,
    /// Streaming profile preset
    #[arg(long, value_enum, default_value_t = ProfileArg::Balanced)]
    profile: ProfileArg,
    /// CPU threshold for auto switch to hardware encoder
    #[arg(long)]
    auto_threshold: Option<u32>,
    /// Consecutive high-CPU samples before switching in auto mode
    #[arg(long)]
    auto_samples: Option<u32>,
    /// Seconds between CPU samples in auto mode
    #[arg(long)]
    auto_interval: Option<u64>,
    /// Disable audio capture
    #[arg(long, default_value_t = false)]
    no_audio: bool,
    /// Explicit wf-recorder audio device name
    #[arg(long)]
    audio_device: Option<String>,
    /// Audio backend for wf-recorder
    #[arg(long, default_value = "pipewire")]
    audio_backend: String,
    /// Cast backend priority order (repeat flag or pass comma-separated values)
    #[arg(long = "cast-backend", value_enum, value_delimiter = ',')]
    cast_backends: Vec<CastBackendArg>,
    /// Seconds to wait for pipeline warmup before health check
    #[arg(long)]
    pipeline_warmup: Option<u64>,
}

#[derive(Args, Debug)]
struct ChromecastStopArgs {
    /// Chromecast friendly name (positional)
    #[arg(value_name = "DEVICE", required_unless_present = "device")]
    device_name: Option<String>,
    /// Chromecast friendly name (legacy flag)
    #[arg(long = "device", conflicts_with = "device_name")]
    device: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum EncoderArg {
    Auto,
    #[value(name = "libx264")]
    Libx264,
    #[value(name = "h264_vaapi")]
    H264Vaapi,
    #[value(name = "h264_nvenc")]
    H264Nvenc,
    #[value(name = "h264_amf")]
    H264Amf,
}

impl From<EncoderArg> for chromecast::EncoderStrategy {
    fn from(value: EncoderArg) -> Self {
        match value {
            EncoderArg::Auto => chromecast::EncoderStrategy::Auto,
            EncoderArg::Libx264 => chromecast::EncoderStrategy::Libx264,
            EncoderArg::H264Vaapi => chromecast::EncoderStrategy::H264Vaapi,
            EncoderArg::H264Nvenc => chromecast::EncoderStrategy::H264Nvenc,
            EncoderArg::H264Amf => chromecast::EncoderStrategy::H264Amf,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "low-latency")]
    LowLatency,
    Balanced,
    Quality,
}

impl From<ProfileArg> for chromecast::StreamProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::LowLatency => chromecast::StreamProfile::LowLatency,
            ProfileArg::Balanced => chromecast::StreamProfile::Balanced,
            ProfileArg::Quality => chromecast::StreamProfile::Quality,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CastBackendArg {
    Python,
    #[value(name = "rust_caster")]
    RustCaster,
    Catt,
    Castv2,
}

impl From<CastBackendArg> for chromecast::CastBackendKind {
    fn from(value: CastBackendArg) -> Self {
        match value {
            CastBackendArg::Python => chromecast::CastBackendKind::Python,
            CastBackendArg::RustCaster => chromecast::CastBackendKind::RustCaster,
            CastBackendArg::Catt => chromecast::CastBackendKind::Catt,
            CastBackendArg::Castv2 => chromecast::CastBackendKind::Castv2,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Recommend) {
        Commands::Recommend => {
            println!("{}", report::recommendation());
            ExitCode::SUCCESS
        }
        Commands::Doctor => doctor::run(),
        Commands::Roadmap => {
            println!("{}", report::roadmap());
            ExitCode::SUCCESS
        }
        Commands::Sources => {
            println!("{}", report::sources());
            ExitCode::SUCCESS
        }
        Commands::Chromecast { command } => run_chromecast(*command),
        Commands::Miracast { command } => run_miracast(command),
        Commands::Tui => run_tui(),
        Commands::Setup { apply } => setup::run(apply),
    }
}

fn run_chromecast(command: ChromecastCommand) -> ExitCode {
    let result: Result<()> = match command {
        ChromecastCommand::List => chromecast::list_devices_cmd(),
        ChromecastCommand::Start(args) => (|| -> Result<()> {
            let ChromecastStartArgs {
                device_name,
                device,
                output,
                interface,
                stream_only,
                compat_video,
                port,
                framerate,
                crf,
                preset,
                encoder,
                profile,
                auto_threshold,
                auto_samples,
                auto_interval,
                no_audio,
                audio_device,
                audio_backend,
                cast_backends,
                pipeline_warmup,
            } = *args;

            let resolved_name = if stream_only {
                let provided = match (device_name, device) {
                    (Some(name), None) => Some(name),
                    (None, Some(name)) => Some(name),
                    (Some(_), Some(_)) => {
                        return Err(anyhow!(
                            "Provide device either as positional argument or --device, not both"
                        ));
                    }
                    (None, None) => None,
                };

                if let Some(name) = provided {
                    let resolved = config::resolve_device_alias(&name)?;
                    if resolved.used_alias {
                        println!(
                            "Using device alias '{}' -> '{}'",
                            resolved.input, resolved.resolved
                        );
                    }
                    resolved.resolved
                } else {
                    "stream-only".to_string()
                }
            } else {
                let device = resolve_device_input(device_name, device)?;
                let resolved_device = config::resolve_device_alias(&device)?;
                if resolved_device.used_alias {
                    println!(
                        "Using device alias '{}' -> '{}'",
                        resolved_device.input, resolved_device.resolved
                    );
                }
                resolved_device.resolved
            };

            let mut options = chromecast::StartOptions::defaults_for_device(resolved_name);
            options.apply_profile(profile.into());
            options.output = output.or_else(|| config::default_output().ok().flatten());
            options.interface = interface;
            options.stream_only = stream_only;
            options.compat_video = compat_video;
            options.port = port;
            options.audio = !no_audio;
            options.audio_device = audio_device;
            options.audio_backend = audio_backend;
            options.encoder = encoder.into();

            if let Some(fps) = framerate {
                options.framerate = fps;
            }
            if let Some(crf_value) = crf {
                options.crf = crf_value;
            }
            if let Some(preset_value) = preset {
                options.preset = preset_value;
            }
            if let Some(threshold) = auto_threshold {
                options.auto_threshold = threshold;
            }
            if let Some(samples) = auto_samples {
                options.auto_samples = samples;
            }
            if let Some(interval) = auto_interval {
                options.auto_interval_secs = interval;
            }
            if let Some(warmup) = pipeline_warmup {
                options.pipeline_warmup_secs = warmup;
            }
            if !cast_backends.is_empty() {
                options.cast_backend_order = cast_backends.into_iter().map(Into::into).collect();
            }

            chromecast::start_cmd(options)
        })(),
        ChromecastCommand::Stop(args) => (|| -> Result<()> {
            let ChromecastStopArgs {
                device_name,
                device,
            } = args;
            let device = resolve_device_input(device_name, device)?;
            let resolved_device = config::resolve_device_alias(&device)?;
            if resolved_device.used_alias {
                println!(
                    "Using device alias '{}' -> '{}'",
                    resolved_device.input, resolved_device.resolved
                );
            }
            chromecast::stop_cmd(&resolved_device.resolved)
        })(),
    };

    if let Err(err) = result {
        eprintln!("Error: {err:#}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn resolve_device_input(
    device_name: Option<String>,
    device_flag: Option<String>,
) -> Result<String> {
    match (device_name, device_flag) {
        (Some(name), None) => Ok(name),
        (None, Some(name)) => Ok(name),
        (Some(_), Some(_)) => Err(anyhow!(
            "Provide device either as positional argument or --device, not both"
        )),
        (None, None) => {
            // Fall back to default device from config
            match config::default_device() {
                Ok(Some(default)) => {
                    println!("Using default device from config: '{default}'");
                    Ok(default)
                }
                Ok(None) => Err(anyhow!(
                    "Missing device argument (no default_device set in config)"
                )),
                Err(err) => Err(anyhow!("Missing device argument (config error: {err})")),
            }
        }
    }
}

fn run_tui() -> ExitCode {
    if let Err(err) = tui::run() {
        eprintln!("Error: {err:#}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run_miracast(command: MiracastCommand) -> ExitCode {
    let result: Result<()> = match command {
        MiracastCommand::Discover(args) => {
            miracast::discover_cmd(args.interface.as_deref(), args.timeout_secs)
        }
        MiracastCommand::Connect(args) => miracast::connect_cmd(
            &args.peer,
            args.interface.as_deref(),
            args.timeout_secs,
            args.pin.as_deref(),
            args.force,
        ),
        MiracastCommand::Start(args) => {
            miracast::start_cmd(&args.host, args.rtsp_port, args.timeout_secs, args.force)
        }
        MiracastCommand::Stop => miracast::stop_cmd(),
        MiracastCommand::Status => miracast::status_cmd(),
    };

    if let Err(err) = result {
        eprintln!("Error: {err:#}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
