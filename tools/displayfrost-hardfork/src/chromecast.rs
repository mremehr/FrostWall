use crate::castv2;
use crate::util;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CAST_RETRY_DELAY_SECS: u64 = 5;
const PLAYBACK_CONFIRM_TIMEOUT_SECS: u64 = 20;
const PLAYBACK_CONFIRM_POLL_MS: u64 = 700;
const STOP_PROCESS_TIMEOUT_SECS: u64 = 3;
const PLAYLIST_READY_TIMEOUT_SECS: u64 = 45;
const PLAYLIST_READY_POLL_MS: u64 = 200;
const STREAM_PLAYLIST_NAME: &str = "stream.m3u8";
const STREAM_CONTENT_TYPE: &str = "application/vnd.apple.mpegurl";
const STREAM_TYPE: &str = "LIVE";

const PY_FALLBACK_PLAY: &str = r#"
import sys
import pychromecast

name = sys.argv[1]
host = sys.argv[2]
url = sys.argv[3]

chromecasts, browser = pychromecast.get_listed_chromecasts(
    friendly_names=[name],
    known_hosts=[host],
    discovery_timeout=8,
)
if not chromecasts:
    chromecasts, browser = pychromecast.get_chromecasts(known_hosts=[host])

target = None
for c in chromecasts:
    c_host = getattr(c, "host", None)
    if not c_host:
        info = getattr(c, "cast_info", None)
        c_host = getattr(info, "host", None)
    if c_host == host or c.name == name:
        target = c
        break

if target is None:
    browser.stop_discovery()
    raise SystemExit("No matching chromecast in python fallback")

target.wait()
mc = target.media_controller
mc.play_media(url, "application/vnd.apple.mpegurl", stream_type="LIVE")
mc.block_until_active()
browser.stop_discovery()
"#;

const PY_FALLBACK_STOP: &str = r#"
import sys
import pychromecast

name = sys.argv[1]
host = sys.argv[2]

chromecasts, browser = pychromecast.get_listed_chromecasts(
    friendly_names=[name],
    known_hosts=[host],
    discovery_timeout=8,
)
if not chromecasts:
    chromecasts, browser = pychromecast.get_chromecasts(known_hosts=[host])

target = None
for c in chromecasts:
    c_host = getattr(c, "host", None)
    if not c_host:
        info = getattr(c, "cast_info", None)
        c_host = getattr(info, "host", None)
    if c_host == host or c.name == name:
        target = c
        break

if target is not None:
    target.wait()
    target.media_controller.stop()

browser.stop_discovery()
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EncoderStrategy {
    Auto,
    Libx264,
    H264Vaapi,
    H264Nvenc,
    H264Amf,
}

impl EncoderStrategy {
    pub fn label(self) -> &'static str {
        match self {
            EncoderStrategy::Auto => "auto",
            EncoderStrategy::Libx264 => "libx264",
            EncoderStrategy::H264Vaapi => "h264_vaapi",
            EncoderStrategy::H264Nvenc => "h264_nvenc",
            EncoderStrategy::H264Amf => "h264_amf",
        }
    }

    pub fn next(self) -> Self {
        match self {
            EncoderStrategy::Auto => EncoderStrategy::Libx264,
            EncoderStrategy::Libx264 => EncoderStrategy::H264Vaapi,
            EncoderStrategy::H264Vaapi => EncoderStrategy::H264Nvenc,
            EncoderStrategy::H264Nvenc => EncoderStrategy::H264Amf,
            EncoderStrategy::H264Amf => EncoderStrategy::Auto,
        }
    }

    fn is_hardware(self) -> bool {
        matches!(
            self,
            EncoderStrategy::H264Vaapi | EncoderStrategy::H264Nvenc | EncoderStrategy::H264Amf
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamProfile {
    LowLatency,
    Balanced,
    Quality,
}

impl StreamProfile {
    pub fn label(self) -> &'static str {
        match self {
            StreamProfile::LowLatency => "low-latency",
            StreamProfile::Balanced => "balanced",
            StreamProfile::Quality => "quality",
        }
    }

    pub fn next(self) -> Self {
        match self {
            StreamProfile::LowLatency => StreamProfile::Balanced,
            StreamProfile::Balanced => StreamProfile::Quality,
            StreamProfile::Quality => StreamProfile::LowLatency,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CastBackendKind {
    Python,
    RustCaster,
    Catt,
    Castv2,
}

impl CastBackendKind {
    pub fn label(self) -> &'static str {
        match self {
            CastBackendKind::Python => "python",
            CastBackendKind::RustCaster => "rust_caster",
            CastBackendKind::Catt => "catt",
            CastBackendKind::Castv2 => "castv2",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StartOptions {
    pub device: String,
    pub stream_only: bool,
    pub compat_video: bool,
    pub output: Option<String>,
    pub interface: Option<String>,
    pub port: u16,
    pub framerate: u32,
    pub crf: u32,
    pub preset: String,
    pub audio: bool,
    pub audio_device: Option<String>,
    pub audio_backend: String,
    pub encoder: EncoderStrategy,
    pub profile: StreamProfile,
    pub cast_backend_order: Vec<CastBackendKind>,
    pub auto_threshold: u32,
    pub auto_samples: u32,
    pub auto_interval_secs: u64,
    pub pipeline_warmup_secs: u64,
}

impl StartOptions {
    pub fn defaults_for_device(device: String) -> Self {
        let mut options = Self {
            device,
            stream_only: false,
            compat_video: false,
            output: None,
            interface: None,
            port: 8888,
            framerate: 60,
            crf: 18,
            preset: "veryfast".to_string(),
            audio: true,
            audio_device: None,
            audio_backend: "pipewire".to_string(),
            encoder: EncoderStrategy::Auto,
            profile: StreamProfile::Balanced,
            cast_backend_order: default_cast_backend_order(),
            auto_threshold: 75,
            auto_samples: 3,
            auto_interval_secs: 2,
            pipeline_warmup_secs: 1,
        };
        options.apply_profile(StreamProfile::Balanced);
        options
    }

    pub fn apply_profile(&mut self, profile: StreamProfile) {
        self.profile = profile;
        match profile {
            StreamProfile::LowLatency => {
                self.framerate = 60;
                self.crf = 24;
                self.preset = "ultrafast".to_string();
                self.auto_threshold = 65;
                self.auto_samples = 2;
                self.auto_interval_secs = 1;
            }
            StreamProfile::Balanced => {
                self.framerate = 60;
                self.crf = 18;
                self.preset = "veryfast".to_string();
                self.auto_threshold = 75;
                self.auto_samples = 3;
                self.auto_interval_secs = 2;
            }
            StreamProfile::Quality => {
                self.framerate = 60;
                self.crf = 15;
                self.preset = "faster".to_string();
                self.auto_threshold = 82;
                self.auto_samples = 4;
                self.auto_interval_secs = 2;
            }
        }
    }
}

fn default_cast_backend_order() -> Vec<CastBackendKind> {
    vec![
        CastBackendKind::Castv2,
        CastBackendKind::RustCaster,
        CastBackendKind::Python,
        CastBackendKind::Catt,
    ]
}

#[derive(Clone, Debug)]
pub struct CastDevice {
    pub name: String,
    pub id: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
}

struct Pipeline {
    wf: Child,
    ffmpeg: Child,
    http: HlsHttpServer,
    stream_dir: PathBuf,
}

struct HlsHttpServer {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl HlsHttpServer {
    fn start(port: u16, root_dir: &Path) -> Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))
            .with_context(|| format!("Failed to bind HLS HTTP server on 0.0.0.0:{port}"))?;
        listener
            .set_nonblocking(true)
            .with_context(|| "Failed to set HLS HTTP listener as non-blocking")?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let root = root_dir.to_path_buf();
        let debug = debug_stream_logs_enabled();

        let handle = thread::spawn(move || {
            while !stop_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let root = root.clone();
                        if let Err(err) = handle_http_connection(stream, &root, debug)
                            && debug
                            && !is_benign_http_disconnect(&err)
                        {
                            eprintln!("HLS HTTP connection error: {err:#}");
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        if debug {
                            eprintln!("HLS HTTP accept error: {err}");
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });

        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }

    fn is_running(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for HlsHttpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Default)]
struct EncoderCapabilities {
    vaapi: bool,
    nvenc: bool,
    amf: bool,
}

impl EncoderCapabilities {
    fn supports(&self, encoder: EncoderStrategy) -> bool {
        match encoder {
            EncoderStrategy::Libx264 | EncoderStrategy::Auto => true,
            EncoderStrategy::H264Vaapi => self.vaapi,
            EncoderStrategy::H264Nvenc => self.nvenc,
            EncoderStrategy::H264Amf => self.amf,
        }
    }

    fn available_hardware_list(&self) -> Vec<EncoderStrategy> {
        let mut list = Vec::new();
        if self.vaapi {
            list.push(EncoderStrategy::H264Vaapi);
        }
        if self.nvenc {
            list.push(EncoderStrategy::H264Nvenc);
        }
        if self.amf {
            list.push(EncoderStrategy::H264Amf);
        }
        list
    }
}

#[derive(Clone, Debug)]
struct CastBackends {
    python: bool,
    rust_caster: Option<PathBuf>,
    catt: Option<PathBuf>,
}

impl CastBackends {
    fn detect() -> Self {
        Self {
            python: python_backend_available(),
            rust_caster: rust_caster_binary(),
            catt: catt_binary(),
        }
    }

    fn supports(&self, backend: CastBackendKind) -> bool {
        match backend {
            CastBackendKind::Python => self.python,
            CastBackendKind::RustCaster => self.rust_caster.is_some(),
            CastBackendKind::Catt => self.catt.is_some(),
            CastBackendKind::Castv2 => true,
        }
    }

    fn labels(&self) -> Vec<&'static str> {
        [
            CastBackendKind::Python,
            CastBackendKind::RustCaster,
            CastBackendKind::Catt,
            CastBackendKind::Castv2,
        ]
        .into_iter()
        .filter(|backend| self.supports(*backend))
        .map(|backend| backend.label())
        .collect()
    }
}

fn resolve_cast_backend_order(
    requested: &[CastBackendKind],
    backends: &CastBackends,
) -> Vec<CastBackendKind> {
    let mut deduped: Vec<CastBackendKind> = Vec::new();
    let base = if requested.is_empty() {
        default_cast_backend_order()
    } else {
        requested.to_vec()
    };

    for backend in base {
        if !deduped.contains(&backend) {
            deduped.push(backend);
        }
    }

    let mut resolved: Vec<CastBackendKind> = deduped
        .into_iter()
        .filter(|backend| backends.supports(*backend))
        .collect();

    if !resolved.contains(&CastBackendKind::Castv2) && backends.supports(CastBackendKind::Castv2) {
        resolved.push(CastBackendKind::Castv2);
    }

    resolved
}

pub fn list_devices_cmd() -> Result<()> {
    ensure_command("avahi-browse")?;

    let devices = discover_devices()?;
    if devices.is_empty() {
        println!("No Chromecast devices found.");
        return Ok(());
    }

    for (idx, device) in devices.iter().enumerate() {
        let location = match (&device.ip, device.port) {
            (Some(ip), Some(port)) => format!("{ip}:{port}"),
            (Some(ip), None) => ip.clone(),
            _ => "unresolved".to_string(),
        };
        let id_note = device
            .id
            .as_ref()
            .map(|id| format!(" [id={id}]"))
            .unwrap_or_default();
        println!("{}. {}{} ({location})", idx + 1, device.name, id_note);
    }
    Ok(())
}

pub fn start_cmd(options: StartOptions) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = Arc::clone(&stop);

    ctrlc::set_handler(move || {
        stop_for_handler.store(true, Ordering::SeqCst);
    })
    .with_context(|| "Failed to register Ctrl+C handler")?;

    let (status_tx, status_rx) = mpsc::channel::<String>();
    let printer = thread::spawn(move || {
        for line in status_rx {
            println!("{line}");
        }
    });

    let result = run_session(options, stop, Some(status_tx));
    let _ = printer.join();
    result
}

pub fn stop_cmd(device: &str) -> Result<()> {
    let discovered = discover_devices()?;
    let target = resolve_device_selection(&discovered, device)?;
    let host = target
        .ip
        .ok_or_else(|| anyhow!("Chromecast '{device}' did not expose an IP in discovery"))?;
    let backends = CastBackends::detect();
    let order = resolve_cast_backend_order(&[], &backends);
    cast_stop(device, &host, &backends, &order)
}

pub fn parse_avahi_output(text: &str) -> Vec<CastDevice> {
    let mut resolved: BTreeMap<String, CastDevice> = BTreeMap::new();
    let mut unresolved: BTreeMap<String, CastDevice> = BTreeMap::new();

    for line in text.lines() {
        if !line.starts_with('=') && !line.starts_with('+') {
            continue;
        }

        let parts: Vec<&str> = line.split(';').collect();
        if parts.len() < 4 || parts[2] != "IPv4" {
            continue;
        }

        let name = parts[3].trim();
        if name.is_empty() {
            continue;
        }

        let id = parse_device_id_from_parts(&parts);

        if line.starts_with('=') {
            let ip = if parts.len() > 7 && !parts[7].trim().is_empty() {
                Some(parts[7].trim().to_string())
            } else {
                None
            };
            let port = if parts.len() > 8 {
                parts[8].trim().parse::<u16>().ok()
            } else {
                None
            };
            let key = id
                .as_ref()
                .map(|v| format!("id:{v}"))
                .or_else(|| {
                    ip.as_ref()
                        .map(|value| format!("name-ip:{}:{value}", name.to_ascii_lowercase()))
                })
                .unwrap_or_else(|| format!("name:{}", name.to_ascii_lowercase()));

            let entry = resolved.entry(key).or_insert_with(|| CastDevice {
                name: name.to_string(),
                id: id.clone(),
                ip: ip.clone(),
                port,
            });
            if entry.id.is_none() {
                entry.id = id.clone();
            }
            if parts.len() > 7 && !parts[7].trim().is_empty() {
                entry.ip = Some(parts[7].trim().to_string());
            }
            if parts.len() > 8 {
                entry.port = parts[8].trim().parse::<u16>().ok();
            }
            unresolved.remove(&name.to_ascii_lowercase());
            continue;
        }

        let unresolved_key = name.to_ascii_lowercase();
        if resolved
            .values()
            .any(|device| device.name.eq_ignore_ascii_case(name))
        {
            continue;
        }

        unresolved
            .entry(unresolved_key)
            .or_insert_with(|| CastDevice {
                name: name.to_string(),
                id,
                ip: None,
                port: None,
            });
    }

    let mut devices: Vec<CastDevice> = resolved.into_values().collect();
    devices.extend(unresolved.into_values());
    devices.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.ip.cmp(&b.ip))
    });
    devices
}

pub fn discover_devices() -> Result<Vec<CastDevice>> {
    ensure_command("avahi-browse")?;

    let output = Command::new("avahi-browse")
        .args(["-rtp", "_googlecast._tcp"])
        .output()
        .with_context(|| "Failed to run avahi-browse")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr_lower = stderr.to_lowercase();
        let daemon_hint = if stderr_lower.contains("daemon not running")
            || stderr_lower.contains("demonen är inte igång")
            || stderr_lower.contains("failed to create client object")
            || stderr_lower.contains("misslyckades med att skapa klientobjekt")
        {
            " | starta Avahi: `sudo systemctl enable --now avahi-daemon`"
        } else {
            ""
        };
        bail!(
            "avahi-browse failed with status {}{}{}",
            output.status.code().unwrap_or(-1),
            if stderr.is_empty() {
                "".to_string()
            } else {
                format!(": {stderr}")
            },
            daemon_hint
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_avahi_output(&text))
}

pub fn run_session(
    options: StartOptions,
    stop: Arc<AtomicBool>,
    status_tx: Option<mpsc::Sender<String>>,
) -> Result<()> {
    let status_tx = status_tx.as_ref();

    ensure_command("wf-recorder")?;
    ensure_command("ffmpeg")?;
    ensure_command("ip")?;
    let target_host = resolve_target_host(&options)?;

    let local_ip = detect_local_ip(options.interface.as_deref(), target_host.as_deref())
        .with_context(|| "Unable to detect local IPv4 address for stream URL")?;
    let stream_url = format!(
        "http://{local_ip}:{}/{}",
        options.port, STREAM_PLAYLIST_NAME
    );

    let capabilities = detect_encoder_capabilities();
    let (cast_backends, cast_backend_order) = resolve_cast_runtime(&options, status_tx);
    let available_hw = capabilities
        .available_hardware_list()
        .iter()
        .map(|e| e.label())
        .collect::<Vec<_>>();
    emit(
        status_tx,
        format!(
            "Profile={} | hardware encoders: {}",
            options.profile.label(),
            if available_hw.is_empty() {
                "none".to_string()
            } else {
                available_hw.join(", ")
            }
        ),
    );
    if options.stream_only {
        emit(
            status_tx,
            "Stream-only mode enabled (cast attach disabled)".to_string(),
        );
    }

    if options.encoder.is_hardware() && !capabilities.supports(options.encoder) {
        bail!(
            "Requested encoder '{}' not available in ffmpeg encoder list",
            options.encoder.label()
        );
    }

    let mut active_encoder = match options.encoder {
        EncoderStrategy::Auto => EncoderStrategy::Libx264,
        other => other,
    };

    emit(
        status_tx,
        if options.stream_only {
            "Starting local stream (stream-only mode)".to_string()
        } else {
            format!("Starting stream for '{}'", options.device)
        },
    );

    let mut pipeline = start_pipeline(&options, active_encoder)
        .with_context(|| format!("Failed to start pipeline with {}", active_encoder.label()))?;

    emit(
        status_tx,
        format!(
            "Stream URL: {stream_url} (initial encoder: {})",
            active_encoder.label()
        ),
    );

    let mut cast_active = false;
    let mut cast_attempts = 1u32;
    let mut last_cast_attempt = Instant::now();

    emit(
        status_tx,
        format!(
            "Waiting for {} to be ready (timeout {}s)",
            STREAM_PLAYLIST_NAME, PLAYLIST_READY_TIMEOUT_SECS
        ),
    );
    if let Err(err) = wait_for_playlist_ready(
        pipeline.stream_dir.as_path(),
        Duration::from_secs(PLAYLIST_READY_TIMEOUT_SECS),
    ) {
        emit_playlist_not_ready(status_tx, options.stream_only, "yet", &err);
    }

    if options.stream_only {
        emit(status_tx, format!("Stream ready (no cast): {stream_url}"));
    } else {
        cast_active = try_cast_attach(
            &options,
            cast_runtime_refs(&target_host, &cast_backends, &cast_backend_order),
            &stream_url,
            status_tx,
            None,
        );
    }
    emit(status_tx, "Press Ctrl+C (CLI) or stop in TUI".to_string());

    let mut cpu = CpuTracker::new();
    let mut over_threshold_streak = 0u32;
    let mut tried_hardware: HashSet<EncoderStrategy> = HashSet::new();
    let mut auto_exhausted = false;

    let run_result = loop {
        if stop.load(Ordering::SeqCst) {
            break Ok(());
        }

        thread::sleep(Duration::from_secs(options.auto_interval_secs.max(1)));

        if stop.load(Ordering::SeqCst) {
            break Ok(());
        }

        if let Some(status) = pipeline
            .ffmpeg
            .try_wait()
            .with_context(|| "ffmpeg status check failed")?
        {
            if stop.load(Ordering::SeqCst) {
                break Ok(());
            }
            break Err(anyhow!("ffmpeg exited unexpectedly: {status}"));
        }

        if let Some(status) = pipeline
            .wf
            .try_wait()
            .with_context(|| "wf-recorder status check failed")?
        {
            if stop.load(Ordering::SeqCst) {
                break Ok(());
            }
            break Err(anyhow!("wf-recorder exited unexpectedly: {status}"));
        }

        if !pipeline.http.is_running() {
            if stop.load(Ordering::SeqCst) {
                break Ok(());
            }
            break Err(anyhow!("HLS HTTP server exited unexpectedly"));
        }

        if !options.stream_only
            && !cast_active
            && last_cast_attempt.elapsed() >= Duration::from_secs(CAST_RETRY_DELAY_SECS)
        {
            cast_attempts = cast_attempts.saturating_add(1);
            emit(
                status_tx,
                format!("Retrying cast playback (attempt {cast_attempts})"),
            );

            cast_active = try_cast_attach(
                &options,
                cast_runtime_refs(&target_host, &cast_backends, &cast_backend_order),
                &stream_url,
                status_tx,
                Some(cast_attempts),
            );

            last_cast_attempt = Instant::now();
        }

        if options.encoder == EncoderStrategy::Auto
            && active_encoder == EncoderStrategy::Libx264
            && !auto_exhausted
            && let Some(usage) = cpu.sample_pct()
        {
            if usage >= options.auto_threshold {
                over_threshold_streak += 1;
            } else {
                over_threshold_streak = 0;
            }

            if over_threshold_streak >= options.auto_samples.max(1) {
                if let Some(target_encoder) =
                    pick_next_hardware_encoder(&capabilities, &tried_hardware)
                {
                    tried_hardware.insert(target_encoder);
                    emit(
                        status_tx,
                        format!(
                            "CPU {}% >= {}% for {} samples, trying {}",
                            usage,
                            options.auto_threshold,
                            over_threshold_streak,
                            target_encoder.label()
                        ),
                    );

                    stop_pipeline(&mut pipeline);
                    match start_pipeline(&options, target_encoder) {
                        Ok(mut new_pipeline) => {
                            if let Err(err) = wait_for_playlist_ready(
                                new_pipeline.stream_dir.as_path(),
                                Duration::from_secs(PLAYLIST_READY_TIMEOUT_SECS),
                            ) {
                                emit_playlist_not_ready(
                                    status_tx,
                                    options.stream_only,
                                    "after encoder switch",
                                    &err,
                                );
                            }
                            if let Err(err) = rearm_cast_after_pipeline_change(
                                &options,
                                cast_runtime_refs(
                                    &target_host,
                                    &cast_backends,
                                    &cast_backend_order,
                                ),
                                &stream_url,
                                status_tx,
                                "Encoder switched in stream-only mode",
                                "Cast playback re-armed via",
                                "Failed to re-arm cast playback after encoder switch",
                            ) {
                                stop_pipeline(&mut new_pipeline);
                                break Err(err);
                            }
                            active_encoder = target_encoder;
                            pipeline = new_pipeline;
                            over_threshold_streak = 0;
                            emit(status_tx, format!("Switched to {}", target_encoder.label()));
                        }
                        Err(hw_error) => {
                            emit(
                                status_tx,
                                format!(
                                    "{} failed: {hw_error}. Falling back to libx264",
                                    target_encoder.label()
                                ),
                            );
                            pipeline = start_pipeline(&options, EncoderStrategy::Libx264)
                                .with_context(
                                    || "Could not restart libx264 after failed hardware switch",
                                )?;
                            if let Err(err) = wait_for_playlist_ready(
                                pipeline.stream_dir.as_path(),
                                Duration::from_secs(PLAYLIST_READY_TIMEOUT_SECS),
                            ) {
                                emit_playlist_not_ready(
                                    status_tx,
                                    options.stream_only,
                                    "after fallback",
                                    &err,
                                );
                            }
                            if let Err(err) = rearm_cast_after_pipeline_change(
                                &options,
                                cast_runtime_refs(
                                    &target_host,
                                    &cast_backends,
                                    &cast_backend_order,
                                ),
                                &stream_url,
                                status_tx,
                                "Encoder fallback in stream-only mode",
                                "Cast playback re-armed via",
                                "Failed to re-arm cast playback after fallback",
                            ) {
                                stop_pipeline(&mut pipeline);
                                break Err(err);
                            }
                            over_threshold_streak = 0;
                        }
                    }
                } else {
                    auto_exhausted = true;
                    emit(
                        status_tx,
                        "Auto mode has no remaining hardware encoder to try".to_string(),
                    );
                }
            }
        }
    };

    emit(status_tx, "Stopping stream".to_string());
    stop_cast_if_needed(
        &options,
        cast_active,
        cast_runtime_refs(&target_host, &cast_backends, &cast_backend_order),
        status_tx,
    );
    stop_pipeline(&mut pipeline);
    emit(status_tx, "Stopped".to_string());

    run_result
}

fn resolve_target_host(options: &StartOptions) -> Result<Option<String>> {
    if options.stream_only {
        return Ok(None);
    }

    ensure_command("avahi-browse")?;
    let discovered = discover_devices()?;
    let target = resolve_device_selection(&discovered, &options.device)?;
    let host = target.ip.ok_or_else(|| {
        anyhow!(
            "Chromecast '{}' found but no IP could be resolved from Avahi",
            options.device
        )
    })?;
    Ok(Some(host))
}

fn resolve_cast_runtime(
    options: &StartOptions,
    status_tx: Option<&mpsc::Sender<String>>,
) -> (Option<CastBackends>, Option<Vec<CastBackendKind>>) {
    if options.stream_only {
        return (None, None);
    }

    let backends = CastBackends::detect();
    let order = resolve_cast_backend_order(&options.cast_backend_order, &backends);
    emit(
        status_tx,
        format!("Cast backends available: {}", backends.labels().join(", ")),
    );
    emit(
        status_tx,
        format!(
            "Cast backend order: {}",
            order
                .iter()
                .map(|backend| backend.label())
                .collect::<Vec<_>>()
                .join(" -> ")
        ),
    );

    (Some(backends), Some(order))
}

fn cast_runtime_refs<'a>(
    target_host: &'a Option<String>,
    cast_backends: &'a Option<CastBackends>,
    cast_backend_order: &'a Option<Vec<CastBackendKind>>,
) -> Option<(&'a str, &'a CastBackends, &'a [CastBackendKind])> {
    Some((
        target_host.as_deref()?,
        cast_backends.as_ref()?,
        cast_backend_order.as_deref()?,
    ))
}

fn playlist_retry_hint(stream_only: bool) -> &'static str {
    if stream_only {
        "Stream keeps running and receiver can retry."
    } else {
        "Continuing with cast retries."
    }
}

fn emit_playlist_not_ready(
    status_tx: Option<&mpsc::Sender<String>>,
    stream_only: bool,
    stage: &str,
    err: &anyhow::Error,
) {
    emit(
        status_tx,
        format!(
            "Warning: {} not ready {} ({err}). {}",
            STREAM_PLAYLIST_NAME,
            stage,
            playlist_retry_hint(stream_only)
        ),
    );
}

fn try_cast_attach(
    options: &StartOptions,
    cast_refs: Option<(&str, &CastBackends, &[CastBackendKind])>,
    stream_url: &str,
    status_tx: Option<&mpsc::Sender<String>>,
    retry_attempt: Option<u32>,
) -> bool {
    if options.stream_only {
        return true;
    }

    let Some((target_host, cast_backends, cast_backend_order)) = cast_refs else {
        return false;
    };

    match cast_play(
        &options.device,
        target_host,
        stream_url,
        cast_backends,
        cast_backend_order,
    ) {
        Ok(backend) => {
            if let Some(attempt) = retry_attempt {
                emit(
                    status_tx,
                    format!(
                        "Cast playback active via {} (attempt {attempt})",
                        backend.label()
                    ),
                );
            } else {
                emit(
                    status_tx,
                    format!("Streaming started via {}", backend.label()),
                );
            }
            true
        }
        Err(err) => {
            if let Some(attempt) = retry_attempt {
                emit(status_tx, format!("Cast attempt {attempt} failed: {err:#}"));
            } else {
                emit(
                    status_tx,
                    format!(
                        "Warning: cast start failed: {err:#}. Stream stays alive at {stream_url}; retrying every {CAST_RETRY_DELAY_SECS}s."
                    ),
                );
            }
            false
        }
    }
}

fn rearm_cast_after_pipeline_change(
    options: &StartOptions,
    cast_refs: Option<(&str, &CastBackends, &[CastBackendKind])>,
    stream_url: &str,
    status_tx: Option<&mpsc::Sender<String>>,
    stream_only_note: &str,
    success_prefix: &str,
    error_context: &'static str,
) -> Result<()> {
    if options.stream_only {
        emit(status_tx, stream_only_note.to_string());
        return Ok(());
    }

    let (target_host, cast_backends, cast_backend_order) = cast_refs
        .ok_or_else(|| anyhow!("Cast runtime context unavailable during pipeline transition"))?;
    let backend = cast_play(
        &options.device,
        target_host,
        stream_url,
        cast_backends,
        cast_backend_order,
    )
    .with_context(|| error_context)?;

    emit(status_tx, format!("{success_prefix} {}", backend.label()));
    Ok(())
}

fn stop_cast_if_needed(
    options: &StartOptions,
    cast_active: bool,
    cast_refs: Option<(&str, &CastBackends, &[CastBackendKind])>,
    status_tx: Option<&mpsc::Sender<String>>,
) {
    if options.stream_only {
        emit(
            status_tx,
            "Stream-only mode: no cast stop command needed".to_string(),
        );
        return;
    }

    let Some((target_host, cast_backends, cast_backend_order)) = cast_refs else {
        emit(
            status_tx,
            "Note: cast runtime unavailable during shutdown; skipping cast stop".to_string(),
        );
        return;
    };

    let stop_note = if cast_active {
        "Sending stop command to cast device".to_string()
    } else {
        "Stop requested before cast confirmed; sending stop command anyway".to_string()
    };
    emit(status_tx, stop_note);
    if let Err(err) = cast_stop(
        &options.device,
        target_host,
        cast_backends,
        cast_backend_order,
    ) {
        emit(
            status_tx,
            format!("Note: could not stop cast media cleanly: {err}"),
        );
    }
}

fn pick_next_hardware_encoder(
    capabilities: &EncoderCapabilities,
    tried_hardware: &HashSet<EncoderStrategy>,
) -> Option<EncoderStrategy> {
    [
        EncoderStrategy::H264Vaapi,
        EncoderStrategy::H264Nvenc,
        EncoderStrategy::H264Amf,
    ]
    .into_iter()
    .find(|encoder| capabilities.supports(*encoder) && !tried_hardware.contains(encoder))
}

fn detect_encoder_capabilities() -> EncoderCapabilities {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output();

    let Ok(output) = output else {
        return EncoderCapabilities::default();
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let haystack = format!("{stdout}\n{stderr}");

    EncoderCapabilities {
        vaapi: haystack.contains("h264_vaapi"),
        nvenc: haystack.contains("h264_nvenc"),
        amf: haystack.contains("h264_amf"),
    }
}

fn start_pipeline(options: &StartOptions, encoder: EncoderStrategy) -> Result<Pipeline> {
    let stream_dir = create_stream_dir()?;
    let mut http = start_hls_http_server(options.port, &stream_dir)?;

    thread::sleep(Duration::from_millis(150));
    if !http.is_running() {
        let _ = fs::remove_dir_all(&stream_dir);
        bail!("HLS HTTP server exited early");
    }

    let mut wf = build_wf_recorder_command(options, encoder);
    let mut wf_child = wf.spawn().with_context(|| "Failed to start wf-recorder")?;
    let wf_stdout = wf_child
        .stdout
        .take()
        .with_context(|| "Failed to connect wf-recorder output to ffmpeg input")?;

    let mut ffmpeg =
        build_ffmpeg_command(&stream_dir, wf_stdout, options.audio, options.compat_video);
    let mut ffmpeg_child = ffmpeg.spawn().with_context(|| "Failed to start ffmpeg")?;

    thread::sleep(Duration::from_secs(options.pipeline_warmup_secs));
    if let Some(status) = ffmpeg_child
        .try_wait()
        .with_context(|| "ffmpeg status check failed")?
    {
        stop_process(&mut wf_child);
        http.stop();
        let _ = fs::remove_dir_all(&stream_dir);
        bail!("ffmpeg exited early: {status}");
    }
    if let Some(status) = wf_child
        .try_wait()
        .with_context(|| "wf-recorder status check failed")?
    {
        stop_process(&mut ffmpeg_child);
        http.stop();
        let _ = fs::remove_dir_all(&stream_dir);
        bail!("wf-recorder exited early: {status}");
    }
    if !http.is_running() {
        stop_process(&mut ffmpeg_child);
        stop_process(&mut wf_child);
        http.stop();
        let _ = fs::remove_dir_all(&stream_dir);
        bail!("HLS HTTP server exited unexpectedly");
    }

    Ok(Pipeline {
        wf: wf_child,
        ffmpeg: ffmpeg_child,
        http,
        stream_dir,
    })
}

fn stop_pipeline(pipeline: &mut Pipeline) {
    stop_process(&mut pipeline.ffmpeg);
    stop_process(&mut pipeline.wf);
    pipeline.http.stop();
    let _ = fs::remove_dir_all(&pipeline.stream_dir);
}

fn create_stream_dir() -> Result<PathBuf> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    let dir =
        std::env::temp_dir().join(format!("displayfrost-hls-{}-{now_ms}", std::process::id()));
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create stream dir {}", dir.display()))?;
    Ok(dir)
}

fn start_hls_http_server(port: u16, stream_dir: &Path) -> Result<HlsHttpServer> {
    HlsHttpServer::start(port, stream_dir)
}

fn handle_http_connection(mut stream: TcpStream, root_dir: &Path, debug: bool) -> Result<()> {
    let peer = stream.peer_addr().ok();
    let reader_stream = stream
        .try_clone()
        .with_context(|| "Failed to clone HTTP client stream")?;
    let mut reader = BufReader::new(reader_stream);

    let mut request_line = String::new();
    let bytes = reader
        .read_line(&mut request_line)
        .with_context(|| "Failed to read HTTP request line")?;
    if bytes == 0 {
        return Ok(());
    }

    loop {
        let mut header_line = String::new();
        let n = reader
            .read_line(&mut header_line)
            .with_context(|| "Failed to read HTTP header line")?;
        if n == 0 || header_line == "\r\n" || header_line == "\n" {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    let request_path = target.split('?').next().unwrap_or("/");

    if method.eq_ignore_ascii_case("OPTIONS") {
        write_http_response(&mut stream, 204, "No Content", "text/plain", b"", false)?;
        log_http_request(debug, peer, method, request_path, 204);
        return Ok(());
    }

    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        let body = b"Method Not Allowed\n";
        write_http_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            body,
            !method.eq_ignore_ascii_case("HEAD"),
        )?;
        log_http_request(debug, peer, method, request_path, 405);
        return Ok(());
    }

    let include_body = method.eq_ignore_ascii_case("GET");
    if let Some(relative) = sanitize_request_path(request_path) {
        let file_path = root_dir.join(relative);
        if file_path.is_file() {
            match fs::read(&file_path) {
                Ok(body) => {
                    let content_type = infer_content_type(&file_path);
                    write_http_response(&mut stream, 200, "OK", content_type, &body, include_body)?;
                    log_http_request(debug, peer, method, request_path, 200);
                    return Ok(());
                }
                Err(err) => {
                    let body = format!("Failed to read file: {err}\n");
                    write_http_response(
                        &mut stream,
                        500,
                        "Internal Server Error",
                        "text/plain; charset=utf-8",
                        body.as_bytes(),
                        include_body,
                    )?;
                    log_http_request(debug, peer, method, request_path, 500);
                    return Ok(());
                }
            }
        }
    } else {
        let body = b"Bad Request\n";
        write_http_response(
            &mut stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            body,
            include_body,
        )?;
        log_http_request(debug, peer, method, request_path, 400);
        return Ok(());
    }

    let body = b"Not Found\n";
    write_http_response(
        &mut stream,
        404,
        "Not Found",
        "text/plain; charset=utf-8",
        body,
        include_body,
    )?;
    log_http_request(debug, peer, method, request_path, 404);
    Ok(())
}

fn sanitize_request_path(request_path: &str) -> Option<String> {
    let stripped = request_path.trim_start_matches('/');
    let normalized = if stripped.is_empty() {
        STREAM_PLAYLIST_NAME
    } else {
        stripped
    };

    if normalized.contains('\\') {
        return None;
    }
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
    }
    Some(normalized.to_string())
}

fn infer_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "m3u8" => STREAM_CONTENT_TYPE,
        "ts" => "video/mp2t",
        "m4s" => "video/iso.segment",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    content_type: &str,
    body: &[u8],
    include_body: bool,
) -> Result<()> {
    write!(stream, "HTTP/1.1 {status_code} {status_text}\r\n")
        .with_context(|| "Failed to write HTTP status line")?;
    write!(stream, "Content-Type: {content_type}\r\n")
        .with_context(|| "Failed to write HTTP Content-Type header")?;
    write!(stream, "Content-Length: {}\r\n", body.len())
        .with_context(|| "Failed to write HTTP Content-Length header")?;
    write!(stream, "Access-Control-Allow-Origin: *\r\n")
        .with_context(|| "Failed to write CORS origin header")?;
    write!(
        stream,
        "Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n"
    )
    .with_context(|| "Failed to write CORS methods header")?;
    write!(stream, "Access-Control-Allow-Headers: *\r\n")
        .with_context(|| "Failed to write CORS headers header")?;
    write!(
        stream,
        "Cache-Control: no-store, no-cache, must-revalidate, max-age=0\r\n"
    )
    .with_context(|| "Failed to write Cache-Control header")?;
    write!(stream, "Connection: close\r\n\r\n")
        .with_context(|| "Failed to write HTTP header terminator")?;
    if include_body && !body.is_empty() {
        stream
            .write_all(body)
            .with_context(|| "Failed to write HTTP response body")?;
    }
    stream
        .flush()
        .with_context(|| "Failed to flush HTTP response stream")?;
    Ok(())
}

fn log_http_request(
    debug: bool,
    peer: Option<std::net::SocketAddr>,
    method: &str,
    path: &str,
    status_code: u16,
) {
    if !debug {
        return;
    }
    let remote = peer
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("{remote} \"{method} {path} HTTP/1.1\" {status_code}");
}

fn wait_for_playlist_ready(stream_dir: &Path, timeout: Duration) -> Result<()> {
    let playlist_path = stream_dir.join(STREAM_PLAYLIST_NAME);
    let deadline = Instant::now() + timeout;
    let mut last_size = 0usize;

    while Instant::now() < deadline {
        if let Ok(content) = fs::read_to_string(&playlist_path) {
            last_size = content.len();
            let has_segment = content
                .lines()
                .map(str::trim)
                .any(|line| line.ends_with(".ts") || line.ends_with(".m4s"));
            if has_segment {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(PLAYLIST_READY_POLL_MS));
    }

    bail!(
        "{} was not ready within {}s (last_size={} bytes)",
        playlist_path.display(),
        timeout.as_secs(),
        last_size
    )
}

fn is_benign_http_disconnect(err: &anyhow::Error) -> bool {
    let text = format!("{err:#}");
    text.contains("Broken pipe")
        || text.contains("Connection reset by peer")
        || text.contains("connection reset by peer")
}

fn ensure_command(command: &str) -> Result<()> {
    if util::command_exists(command) {
        Ok(())
    } else {
        bail!("Required command missing: {command}")
    }
}

fn detect_local_ip(interface: Option<&str>, target_host: Option<&str>) -> Result<String> {
    if let Some(interface) = interface {
        return detect_local_ip_from_addr(Some(interface))
            .with_context(|| format!("No IPv4 found on interface {interface}"));
    }

    if let Some(target_host) = target_host
        && let Some(ip) = detect_local_ip_from_route(target_host)
    {
        return Ok(ip);
    }

    if let Some(ip) = detect_local_ip_from_route("1.1.1.1") {
        return Ok(ip);
    }

    detect_local_ip_from_addr(None).with_context(|| "No non-loopback IPv4 address found")
}

fn detect_local_ip_from_route(target: &str) -> Option<String> {
    let output = Command::new("ip")
        .args(["route", "get", target])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        if word == "src" {
            return words.next().map(ToString::to_string);
        }
    }
    None
}

fn detect_local_ip_from_addr(interface: Option<&str>) -> Result<String> {
    let mut command = Command::new("ip");
    command.args(["-4", "-o", "addr", "show"]);
    if let Some(interface) = interface {
        command.args(["dev", interface]);
    }

    let output = command
        .output()
        .with_context(|| "Failed to run `ip -4 -o addr show`")?;
    if !output.status.success() {
        bail!(
            "`ip` command failed with status {}",
            output.status.code().unwrap_or(-1)
        );
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut words = line.split_whitespace();
        while let Some(word) = words.next() {
            if word == "inet"
                && let Some(ip_with_mask) = words.next()
            {
                let ip = ip_with_mask.split('/').next().unwrap_or_default();
                if !ip.is_empty() && !ip.starts_with("127.") {
                    return Ok(ip.to_string());
                }
            }
        }
    }
    bail!("No IPv4 address found")
}

fn build_wf_recorder_command(options: &StartOptions, encoder: EncoderStrategy) -> Command {
    let mut command = Command::new("wf-recorder");
    let wf_stderr = if debug_stream_logs_enabled() {
        Stdio::inherit()
    } else {
        Stdio::null()
    };
    command
        .arg("-r")
        .arg(options.framerate.to_string())
        .args(["-D", "-b", "0", "-y"])
        .args(["-f", "/dev/stdout", "-m", "nut"])
        .stdout(Stdio::piped())
        .stderr(wf_stderr);

    match encoder {
        EncoderStrategy::Auto => {}
        EncoderStrategy::Libx264 => {
            command
                .arg("--codec=libx264")
                .arg("-p")
                .arg(format!("preset={}", options.preset))
                .args(["-p", "tune=zerolatency"])
                .args(["-p", "g=30"])
                .args([
                    "-p",
                    "x264-params=repeat-headers=1:min-keyint=30:scenecut=0",
                ])
                .arg("-p")
                .arg(format!("crf={}", options.crf));
        }
        EncoderStrategy::H264Vaapi => {
            command
                .arg("--codec=h264_vaapi")
                .args(["-p", "rc_mode=ICQ"])
                .args(["-p", "qp=20"]);
        }
        EncoderStrategy::H264Nvenc => {
            command.arg("--codec=h264_nvenc");
        }
        EncoderStrategy::H264Amf => {
            command.arg("--codec=h264_amf");
        }
    }

    if let Some(output) = &options.output {
        command.args(["-o", output]);
    }

    if options.audio {
        if let Some(device) = &options.audio_device {
            command.arg(format!("--audio={device}"));
        } else {
            command.arg("--audio");
        }
        command.arg(format!("--audio-backend={}", options.audio_backend));
    }

    command
}

fn build_ffmpeg_command(
    stream_dir: &Path,
    stdin: std::process::ChildStdout,
    include_audio: bool,
    compat_video: bool,
) -> Command {
    let mut command = Command::new("ffmpeg");
    let ffmpeg_loglevel = if debug_stream_logs_enabled() {
        "warning"
    } else {
        "error"
    };
    let ffmpeg_stderr = if debug_stream_logs_enabled() {
        Stdio::inherit()
    } else {
        Stdio::null()
    };
    command
        .args(["-hide_banner", "-loglevel", ffmpeg_loglevel])
        .args(["-fflags", "+genpts+nobuffer+discardcorrupt"])
        .args(["-probesize", "1M", "-analyzeduration", "1000000"])
        .args(["-i", "-"]);

    command.args(["-map", "0:v:0"]);
    if include_audio {
        command.args(["-map", "0:a:0?", "-c:a", "aac", "-b:a", "128k"]);
    }

    if compat_video {
        command
            .args([
                "-vf",
                "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black,fps=30,format=yuv420p",
            ])
            .args(["-c:v", "libx264"])
            .args(["-preset", "veryfast", "-tune", "zerolatency"])
            .args(["-profile:v", "high", "-level:v", "4.1"])
            .args(["-crf", "21"])
            .args(["-g", "60", "-keyint_min", "60", "-sc_threshold", "0"])
            .args(["-pix_fmt", "yuv420p"]);
    } else {
        command.args(["-c:v", "copy", "-bsf:v", "h264_mp4toannexb", "-copyinkf"]);
    }

    let segment_pattern = stream_dir.join("seg%05d.ts");
    let playlist_path = stream_dir.join(STREAM_PLAYLIST_NAME);

    command
        .args(["-f", "hls"])
        .args(["-hls_time", "0.5"])
        .args(["-hls_list_size", "6"])
        .args(["-hls_segment_type", "mpegts"])
        .args(["-hls_flags", "omit_endlist+independent_segments"])
        .arg("-hls_segment_filename")
        .arg(&segment_pattern)
        .arg(&playlist_path)
        .args(["-flush_packets", "1"])
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::null())
        .stderr(ffmpeg_stderr);
    command
}

fn debug_stream_logs_enabled() -> bool {
    match std::env::var("DISPLAYFROST_DEBUG") {
        Ok(value) => {
            let v = value.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no")
        }
        Err(_) => false,
    }
}

fn cast_play(
    device: &str,
    host: &str,
    url: &str,
    backends: &CastBackends,
    order: &[CastBackendKind],
) -> Result<CastBackendKind> {
    let mut errors: Vec<String> = Vec::new();

    for backend in order {
        let result = match backend {
            CastBackendKind::Python => python_play_media(device, host, url),
            CastBackendKind::RustCaster => {
                let Some(binary) = backends.rust_caster.as_deref() else {
                    errors.push("rust_caster: binary not found".to_string());
                    continue;
                };
                rust_caster_play_media(binary, host, url)
            }
            CastBackendKind::Catt => {
                let Some(binary) = backends.catt.as_deref() else {
                    errors.push("catt: binary not found".to_string());
                    continue;
                };
                catt_play_media(binary, device, host, url)
            }
            CastBackendKind::Castv2 => {
                castv2::play_media(host, url, STREAM_CONTENT_TYPE, STREAM_TYPE)
            }
        };

        match result {
            Ok(()) => match wait_for_playing_state(
                host,
                Duration::from_secs(PLAYBACK_CONFIRM_TIMEOUT_SECS),
            ) {
                Ok(_) => return Ok(*backend),
                Err(err) => {
                    if should_accept_unconfirmed_playback(*backend, &err) {
                        return Ok(*backend);
                    }
                    errors.push(format!(
                        "{}: attach ok but playback not confirmed: {err}",
                        backend.label()
                    ))
                }
            },
            Err(err) => errors.push(format!("{}: {err}", backend.label())),
        }
    }

    Err(anyhow!("All cast backends failed: {}", errors.join(" | ")))
        .with_context(|| format!("Cast playback failed for device '{device}' at {host}"))
}

fn should_accept_unconfirmed_playback(backend: CastBackendKind, err: &anyhow::Error) -> bool {
    if backend != CastBackendKind::Python {
        return false;
    }

    let lower = format!("{err:#}").to_ascii_lowercase();
    lower.contains("last_state=unknown")
        && (lower.contains("failed to fetch receiver status before media get_status")
            || lower.contains("failed to fetch receiver status"))
}

fn wait_for_playing_state(host: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_state: Option<String> = None;
    let mut last_error: Option<String> = None;

    while Instant::now() < deadline {
        match castv2::media_player_state(host) {
            Ok(Some(state)) => {
                let normalized = state.to_ascii_uppercase();
                last_state = Some(normalized.clone());
                if normalized == "PLAYING" {
                    return Ok(());
                }
            }
            Ok(None) => {}
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        thread::sleep(Duration::from_millis(PLAYBACK_CONFIRM_POLL_MS));
    }

    let state_note = last_state.unwrap_or_else(|| "unknown".to_string());
    let error_note = last_error.unwrap_or_else(|| "none".to_string());
    bail!(
        "state did not reach PLAYING within {}s (last_state={}, last_error={})",
        timeout.as_secs(),
        state_note,
        error_note
    )
}

fn cast_stop(
    device: &str,
    host: &str,
    backends: &CastBackends,
    order: &[CastBackendKind],
) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    for backend in order {
        let result = match backend {
            CastBackendKind::Python => python_stop_media(device, host),
            CastBackendKind::RustCaster => {
                let Some(binary) = backends.rust_caster.as_deref() else {
                    errors.push("rust_caster: binary not found".to_string());
                    continue;
                };
                rust_caster_stop_media(binary, host)
            }
            CastBackendKind::Catt => {
                let Some(binary) = backends.catt.as_deref() else {
                    errors.push("catt: binary not found".to_string());
                    continue;
                };
                catt_stop_media(binary, device, host)
            }
            CastBackendKind::Castv2 => castv2::stop_media(host),
        };

        match result {
            Ok(()) => return Ok(()),
            Err(err) => errors.push(format!("{}: {err}", backend.label())),
        }
    }

    Err(anyhow!(
        "All cast stop backends failed: {}",
        errors.join(" | ")
    ))
    .with_context(|| format!("Cast stop failed for device '{device}' at {host}"))
}

fn rust_caster_play_media(binary: &Path, host: &str, url: &str) -> Result<()> {
    let output = Command::new(binary)
        .args([
            "-a",
            host,
            "-m",
            url,
            "--media-type",
            STREAM_CONTENT_TYPE,
            "--media-stream-type",
            "live",
        ])
        .output()
        .with_context(|| "Failed to execute rust_caster play command")?;

    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "rust_caster play failed (code {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn rust_caster_stop_media(binary: &Path, host: &str) -> Result<()> {
    let output = Command::new(binary)
        .args(["-a", host, "--stop-current"])
        .output()
        .with_context(|| "Failed to execute rust_caster stop command")?;

    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "rust_caster stop failed (code {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn rust_caster_binary() -> Option<PathBuf> {
    if let Some(path) = util::resolve_binary("rust_caster") {
        return Some(path);
    }

    let home = std::env::var("HOME").ok()?;
    let fallback = PathBuf::from(home).join(".cargo/bin/rust_caster");
    if fallback.is_file() {
        Some(fallback)
    } else {
        None
    }
}

fn catt_binary() -> Option<PathBuf> {
    if let Some(path) = util::resolve_binary("catt") {
        return Some(path);
    }

    let home = std::env::var("HOME").ok()?;
    let fallback = PathBuf::from(home).join(".local/bin/catt");
    if fallback.is_file() {
        Some(fallback)
    } else {
        None
    }
}

fn python_backend_available() -> bool {
    if !util::command_exists("python3") {
        return false;
    }

    Command::new("python3")
        .arg("-c")
        .arg("import pychromecast")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn catt_play_media(binary: &Path, device: &str, host: &str, url: &str) -> Result<()> {
    let attempts: Vec<Vec<&str>> = vec![
        vec!["-d", device, "cast", url],
        vec!["-a", host, "cast", url],
    ];

    let mut errors = Vec::new();
    for args in attempts {
        let mode = if args[0] == "-d" { "device" } else { "host" };
        match run_external_command(binary, &args, "catt play") {
            Ok(()) => return Ok(()),
            Err(err) => errors.push(format!("{mode} mode: {err}")),
        }
    }

    bail!("catt play failed: {}", errors.join(" | "))
}

fn catt_stop_media(binary: &Path, device: &str, host: &str) -> Result<()> {
    let attempts: Vec<Vec<&str>> = vec![
        vec!["-d", device, "stop"],
        vec!["-a", host, "stop"],
        vec!["-d", device, "quit_app"],
        vec!["-a", host, "quit_app"],
    ];

    let mut errors = Vec::new();
    for args in attempts {
        let mode = if args[0] == "-d" { "device" } else { "host" };
        match run_external_command(binary, &args, "catt stop") {
            Ok(()) => return Ok(()),
            Err(err) => errors.push(format!("{mode} mode: {err}")),
        }
    }

    bail!("catt stop failed: {}", errors.join(" | "))
}

fn run_external_command(binary: &Path, args: &[&str], action: &str) -> Result<()> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute {action} command"))?;
    if output.status.success() {
        return Ok(());
    }

    bail!(
        "{action} failed (code {}): {}",
        output.status.code().unwrap_or(-1),
        command_output_summary(&output)
    )
}

fn command_output_summary(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    "no output".to_string()
}

fn python_play_media(device: &str, host: &str, url: &str) -> Result<()> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(PY_FALLBACK_PLAY)
        .arg(device)
        .arg(host)
        .arg(url)
        .output()
        .with_context(|| "Failed to execute python fallback play command")?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "python fallback play failed (code {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn python_stop_media(device: &str, host: &str) -> Result<()> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(PY_FALLBACK_STOP)
        .arg(device)
        .arg(host)
        .output()
        .with_context(|| "Failed to execute python fallback stop command")?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "python fallback stop failed (code {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

fn stop_process(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {}
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
    }

    // Try graceful SIGTERM first on Unix
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(STOP_PROCESS_TIMEOUT_SECS);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(_) => break,
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

fn emit(status_tx: Option<&mpsc::Sender<String>>, msg: String) {
    if let Some(tx) = status_tx {
        let _ = tx.send(msg);
    }
}

fn parse_device_id(txt: &str) -> Option<String> {
    for record in parse_txt_records(txt) {
        let mut split = record.splitn(2, '=');
        let Some(key) = split.next() else {
            continue;
        };
        let Some(value) = split.next() else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("id") {
            let parsed = value.trim();
            if !parsed.is_empty() {
                return Some(parsed.to_string());
            }
        }
    }
    None
}

fn parse_device_id_from_parts(parts: &[&str]) -> Option<String> {
    parts.iter().find_map(|part| parse_device_id(part))
}

fn parse_txt_records(txt: &str) -> Vec<String> {
    let mut records = Vec::new();
    let mut in_quote = false;
    let mut current = String::new();

    for ch in txt.chars() {
        match ch {
            '"' if in_quote => {
                in_quote = false;
                if !current.is_empty() {
                    records.push(current.clone());
                }
                current.clear();
            }
            '"' => {
                in_quote = true;
                current.clear();
            }
            _ if in_quote => current.push(ch),
            _ => {}
        }
    }

    if records.is_empty() {
        for token in txt.split_whitespace() {
            let cleaned = token.trim_matches('"').to_string();
            if !cleaned.is_empty() {
                records.push(cleaned);
            }
        }
    }

    records
}

fn resolve_device_selection(devices: &[CastDevice], query: &str) -> Result<CastDevice> {
    let normalized = query.trim();
    if normalized.is_empty() {
        bail!("Chromecast device query was empty");
    }

    let id_matches: Vec<CastDevice> = devices
        .iter()
        .filter(|d| d.id.as_deref() == Some(normalized))
        .cloned()
        .collect();
    if id_matches.len() == 1 {
        return Ok(id_matches[0].clone());
    }

    let exact_name_matches: Vec<CastDevice> = devices
        .iter()
        .filter(|d| d.name == normalized)
        .cloned()
        .collect();
    if exact_name_matches.len() == 1 {
        return Ok(exact_name_matches[0].clone());
    }
    if exact_name_matches.len() > 1 {
        bail!(
            "Chromecast name '{}' is ambiguous. Matches: {}. Use a unique device id instead.",
            normalized,
            format_device_candidates(&exact_name_matches)
        );
    }

    let ci_name_matches: Vec<CastDevice> = devices
        .iter()
        .filter(|d| d.name.eq_ignore_ascii_case(normalized))
        .cloned()
        .collect();
    if ci_name_matches.len() == 1 {
        return Ok(ci_name_matches[0].clone());
    }
    if ci_name_matches.len() > 1 {
        bail!(
            "Chromecast name '{}' is ambiguous (case-insensitive). Matches: {}. Use a unique device id instead.",
            normalized,
            format_device_candidates(&ci_name_matches)
        );
    }

    bail!(
        "Chromecast '{}' not found. Available: {}",
        normalized,
        available_device_list(devices)
    )
}

fn format_device_candidates(devices: &[CastDevice]) -> String {
    let labels: Vec<String> = devices
        .iter()
        .map(|d| {
            let id = d.id.as_deref().unwrap_or("unknown-id");
            match &d.ip {
                Some(ip) => format!("{} [id={id}] ({ip})", d.name),
                None => format!("{} [id={id}] (unresolved)", d.name),
            }
        })
        .collect();
    labels.join(", ")
}

fn available_device_list(devices: &[CastDevice]) -> String {
    if devices.is_empty() {
        return "none".to_string();
    }

    let labels: Vec<String> = devices
        .iter()
        .map(|d| {
            if let Some(id) = &d.id {
                format!("{} [id={}]", d.name, id)
            } else {
                d.name.clone()
            }
        })
        .collect();
    labels.join(", ")
}

struct CpuTracker {
    previous_total: Option<u64>,
    previous_idle: Option<u64>,
}

impl CpuTracker {
    fn new() -> Self {
        Self {
            previous_total: None,
            previous_idle: None,
        }
    }

    fn sample_pct(&mut self) -> Option<u32> {
        let (total, idle) = read_cpu_totals()?;

        let pct = match (self.previous_total, self.previous_idle) {
            (Some(prev_total), Some(prev_idle)) => {
                let total_diff = total.saturating_sub(prev_total);
                let idle_diff = idle.saturating_sub(prev_idle);
                if total_diff == 0 {
                    None
                } else {
                    Some((((total_diff - idle_diff) * 100) / total_diff) as u32)
                }
            }
            _ => None,
        };

        self.previous_total = Some(total);
        self.previous_idle = Some(idle);

        pct
    }
}

fn read_cpu_totals() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().next()?;
    let mut parts = line.split_whitespace();
    if parts.next()? != "cpu" {
        return None;
    }

    let values: Vec<u64> = parts.filter_map(|p| p.parse::<u64>().ok()).collect();
    if values.len() < 4 {
        return None;
    }

    let total = values.iter().sum::<u64>();
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_avahi_single_device() {
        let input = "=;wlp5s0;IPv4;Living Room TV;_googlecast._tcp;local;host.local;192.168.1.10;8009;\"id=abc123\"";
        let devices = parse_avahi_output(input);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Living Room TV");
        assert_eq!(devices[0].id.as_deref(), Some("abc123"));
        assert_eq!(devices[0].ip.as_deref(), Some("192.168.1.10"));
        assert_eq!(devices[0].port, Some(8009));
    }

    #[test]
    fn parse_avahi_browse_line_no_ip() {
        let input = "+;wlp5s0;IPv4;Kitchen Speaker;_googlecast._tcp;local;\"id=speaker-id\"";
        let devices = parse_avahi_output(input);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Kitchen Speaker");
        assert_eq!(devices[0].id.as_deref(), Some("speaker-id"));
        assert!(devices[0].ip.is_none());
        assert!(devices[0].port.is_none());
    }

    #[test]
    fn parse_avahi_skips_ipv6() {
        let input = "=;wlp5s0;IPv6;SomeDevice;_googlecast._tcp;local;host.local;::1;8009;txt";
        let devices = parse_avahi_output(input);
        assert!(devices.is_empty());
    }

    #[test]
    fn parse_avahi_multiple_devices() {
        let input = "\
+;wlp5s0;IPv4;TV A;_googlecast._tcp;local
=;wlp5s0;IPv4;TV A;_googlecast._tcp;local;a.local;10.0.0.1;8009;\"id=tv-a\"
+;wlp5s0;IPv4;TV B;_googlecast._tcp;local
=;wlp5s0;IPv4;TV B;_googlecast._tcp;local;b.local;10.0.0.2;8009;\"id=tv-b\"";
        let devices = parse_avahi_output(input);
        assert_eq!(devices.len(), 2);
        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"TV A"));
        assert!(names.contains(&"TV B"));
    }

    #[test]
    fn parse_avahi_dedup_by_name() {
        let input = "\
+;wlp5s0;IPv4;My TV;_googlecast._tcp;local
=;wlp5s0;IPv4;My TV;_googlecast._tcp;local;host.local;10.0.0.5;8009;\"id=mytv\"
+;eth0;IPv4;My TV;_googlecast._tcp;local";
        let devices = parse_avahi_output(input);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id.as_deref(), Some("mytv"));
        assert_eq!(devices[0].ip.as_deref(), Some("10.0.0.5"));
    }

    #[test]
    fn parse_avahi_empty_input() {
        assert!(parse_avahi_output("").is_empty());
    }

    #[test]
    fn parse_avahi_skips_malformed() {
        let input = "some random line\n;only;two;parts";
        assert!(parse_avahi_output(input).is_empty());
    }

    #[test]
    fn parse_avahi_skips_empty_name() {
        let input = "=;wlp5s0;IPv4;;_googlecast._tcp;local;host.local;10.0.0.1;8009;txt";
        assert!(parse_avahi_output(input).is_empty());
    }

    #[test]
    fn parse_avahi_keeps_duplicate_names_with_distinct_ids() {
        let input = "\
=;wlp5s0;IPv4;Living Room TV;_googlecast._tcp;local;tv-a.local;10.0.0.10;8009;\"id=aaa111\"\n\
=;wlp5s0;IPv4;Living Room TV;_googlecast._tcp;local;tv-b.local;10.0.0.11;8009;\"id=bbb222\"";
        let devices = parse_avahi_output(input);
        assert_eq!(devices.len(), 2);
        let ids: Vec<&str> = devices.iter().filter_map(|d| d.id.as_deref()).collect();
        assert!(ids.contains(&"aaa111"));
        assert!(ids.contains(&"bbb222"));
    }

    #[test]
    fn resolve_device_selection_by_id() {
        let devices = vec![
            CastDevice {
                name: "Living Room TV".to_string(),
                id: Some("tv-1".to_string()),
                ip: Some("10.0.0.1".to_string()),
                port: Some(8009),
            },
            CastDevice {
                name: "Living Room TV".to_string(),
                id: Some("tv-2".to_string()),
                ip: Some("10.0.0.2".to_string()),
                port: Some(8009),
            },
        ];
        let resolved = resolve_device_selection(&devices, "tv-2").unwrap();
        assert_eq!(resolved.ip.as_deref(), Some("10.0.0.2"));
    }

    #[test]
    fn resolve_device_selection_name_ambiguous() {
        let devices = vec![
            CastDevice {
                name: "Living Room TV".to_string(),
                id: Some("tv-1".to_string()),
                ip: Some("10.0.0.1".to_string()),
                port: Some(8009),
            },
            CastDevice {
                name: "Living Room TV".to_string(),
                id: Some("tv-2".to_string()),
                ip: Some("10.0.0.2".to_string()),
                port: Some(8009),
            },
        ];
        let err = resolve_device_selection(&devices, "Living Room TV")
            .expect_err("name should be ambiguous");
        assert!(
            err.to_string()
                .contains("Chromecast name 'Living Room TV' is ambiguous")
        );
    }

    #[test]
    fn resolve_backend_order_default() {
        let backends = CastBackends {
            python: true,
            rust_caster: Some(PathBuf::from("/usr/bin/rust_caster")),
            catt: Some(PathBuf::from("/usr/bin/catt")),
        };
        let order = resolve_cast_backend_order(&[], &backends);
        assert_eq!(
            order,
            vec![
                CastBackendKind::Castv2,
                CastBackendKind::RustCaster,
                CastBackendKind::Python,
                CastBackendKind::Catt,
            ]
        );
    }

    #[test]
    fn resolve_backend_order_filters_unavailable() {
        let backends = CastBackends {
            python: false,
            rust_caster: None,
            catt: None,
        };
        let order = resolve_cast_backend_order(&[], &backends);
        assert_eq!(order, vec![CastBackendKind::Castv2]);
    }

    #[test]
    fn resolve_backend_order_deduplicates() {
        let backends = CastBackends {
            python: true,
            rust_caster: None,
            catt: None,
        };
        let order = resolve_cast_backend_order(
            &[
                CastBackendKind::Python,
                CastBackendKind::Python,
                CastBackendKind::Castv2,
            ],
            &backends,
        );
        assert_eq!(
            order,
            vec![CastBackendKind::Python, CastBackendKind::Castv2]
        );
    }

    #[test]
    fn resolve_backend_order_appends_castv2() {
        let backends = CastBackends {
            python: true,
            rust_caster: None,
            catt: None,
        };
        let order = resolve_cast_backend_order(&[CastBackendKind::Python], &backends);
        assert!(order.contains(&CastBackendKind::Castv2));
    }
}
