use crate::castv2;
use crate::chromecast::{self, EncoderStrategy, StartOptions, StreamProfile};
use crate::config;
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

pub fn run() -> Result<()> {
    enable_raw_mode().with_context(|| "failed to enable raw terminal mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).with_context(|| "failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).with_context(|| "failed to initialize TUI")?;

    let mut app = App::new();
    let result = app.run(&mut terminal);

    restore_terminal(&mut terminal)?;
    result
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().with_context(|| "failed to disable raw terminal mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .with_context(|| "failed to leave alternate screen")?;
    terminal
        .show_cursor()
        .with_context(|| "failed to show cursor")?;
    Ok(())
}

enum DiscoveryState {
    Idle,
    Running(mpsc::Receiver<Result<Vec<chromecast::CastDevice>>>),
}

enum VolumeRequest {
    Get { host: String },
    Set { host: String, level: f32 },
}

enum VolumeEvent {
    Current {
        host: String,
        level: Option<f32>,
    },
    SetApplied {
        host: String,
        level: f32,
    },
    Failed {
        host: String,
        action: &'static str,
        message: String,
    },
}

struct App {
    devices: Vec<chromecast::CastDevice>,
    selected: usize,
    logs: Vec<String>,
    error: Option<String>,
    audio: bool,
    encoder: EncoderStrategy,
    profile: StreamProfile,
    framerate: u32,
    crf: u32,
    port: u16,
    output: Option<String>,
    interface: Option<String>,
    should_quit: bool,
    quit_after_stream_stops: bool,
    stream: Option<StreamState>,
    discovery: DiscoveryState,
    volume: Option<f32>,
    volume_target: Option<f32>,
    volume_tx: mpsc::Sender<VolumeRequest>,
    volume_events: mpsc::Receiver<VolumeEvent>,
}

struct StreamState {
    device: String,
    host: Option<String>,
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    status_rx: mpsc::Receiver<String>,
    done_rx: mpsc::Receiver<Result<(), String>>,
}

impl App {
    fn new() -> Self {
        let (volume_tx, volume_events) = spawn_volume_worker();
        let mut app = Self {
            devices: Vec::new(),
            selected: 0,
            logs: vec!["DisplayFrost TUI ready".to_string()],
            error: None,
            audio: true,
            encoder: EncoderStrategy::Auto,
            profile: StreamProfile::Balanced,
            framerate: 60,
            crf: 18,
            port: 8888,
            output: config::default_output().ok().flatten(),
            interface: None,
            should_quit: false,
            quit_after_stream_stops: false,
            stream: None,
            discovery: DiscoveryState::Idle,
            volume: None,
            volume_target: None,
            volume_tx,
            volume_events,
        };
        app.start_discovery();
        app
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        while !self.should_quit {
            self.pump_stream_events();
            self.pump_discovery();
            self.pump_volume_events();
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(120)).with_context(|| "event poll failed")?
                && let Event::Key(key) = event::read().with_context(|| "event read failed")?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code);
            }
        }

        if let Some(stream) = self.stream.take() {
            stream.stop.store(true, Ordering::SeqCst);
            let _ = stream.handle.join();
        }

        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        if self.stream.is_some() {
            match code {
                KeyCode::Char('s') => self.stop_stream(false),
                KeyCode::Char('q') => self.stop_stream(true),
                KeyCode::Char('v') => self.adjust_volume(-0.05),
                KeyCode::Char('V') => self.adjust_volume(0.05),
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('r') => self.start_discovery(),
            KeyCode::Char('a') => {
                self.audio = !self.audio;
                self.log(format!(
                    "Audio {}",
                    if self.audio { "enabled" } else { "disabled" }
                ));
            }
            KeyCode::Char('e') => {
                self.encoder = self.encoder.next();
                self.log(format!("Encoder mode set to {}", self.encoder.label()));
            }
            KeyCode::Char('p') => {
                self.profile = self.profile.next();
                self.apply_profile_to_session();
                self.log(format!("Profile set to {}", self.profile.label()));
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.framerate = (self.framerate + 5).min(144);
                self.log(format!("Framerate set to {} fps", self.framerate));
            }
            KeyCode::Char('-') => {
                self.framerate = self.framerate.saturating_sub(5).max(15);
                self.log(format!("Framerate set to {} fps", self.framerate));
            }
            KeyCode::Char(']') => {
                self.crf = (self.crf + 1).min(30);
                self.log(format!("CRF set to {}", self.crf));
            }
            KeyCode::Char('[') => {
                self.crf = self.crf.saturating_sub(1).max(10);
                self.log(format!("CRF set to {}", self.crf));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.devices.is_empty() {
                    self.selected = (self.selected + 1) % self.devices.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.devices.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.devices.len() - 1
                    } else {
                        self.selected - 1
                    };
                }
            }
            KeyCode::Enter => self.start_stream(),
            _ => {}
        }
    }

    fn start_discovery(&mut self) {
        if matches!(self.discovery, DiscoveryState::Running(_)) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(chromecast::discover_devices());
        });
        self.discovery = DiscoveryState::Running(rx);
        self.log("Discovering devices...".to_string());
    }

    fn pump_discovery(&mut self) {
        let result = match &self.discovery {
            DiscoveryState::Running(rx) => match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("Discovery thread panicked")))
                }
            },
            DiscoveryState::Idle => None,
        };

        if let Some(result) = result {
            self.discovery = DiscoveryState::Idle;
            match result {
                Ok(devices) => {
                    self.devices = devices;
                    if self.selected >= self.devices.len() {
                        self.selected = 0;
                    }
                    self.error = None;
                    self.log(format!("Found {} Chromecast device(s)", self.devices.len()));
                    self.select_default_device();
                }
                Err(err) => {
                    self.error = Some(format!("Discovery failed: {err}"));
                    self.devices.clear();
                }
            }
        }
    }

    fn select_default_device(&mut self) {
        if self.devices.is_empty() {
            return;
        }
        let default = match config::default_device() {
            Ok(Some(name)) => name,
            _ => return,
        };
        // Try alias resolution
        let resolved = config::resolve_device_alias(&default)
            .map(|r| r.resolved)
            .unwrap_or(default);
        if let Some(idx) = self.devices.iter().position(|d| {
            d.id.as_deref() == Some(resolved.as_str()) || d.name.eq_ignore_ascii_case(&resolved)
        }) {
            self.selected = idx;
        }
    }

    fn start_stream(&mut self) {
        let Some(device) = self.devices.get(self.selected).cloned() else {
            self.error = Some("No Chromecast selected".to_string());
            return;
        };
        let volume_host = device.ip.clone();

        let mut options = StartOptions::defaults_for_device(device.name.clone());
        options.apply_profile(self.profile);
        options.audio = self.audio;
        options.encoder = self.encoder;
        options.framerate = self.framerate;
        options.crf = self.crf;
        options.port = self.port;
        options.output = self.output.clone();
        options.interface = self.interface.clone();

        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);

        let (status_tx, status_rx) = mpsc::channel::<String>();
        let (done_tx, done_rx) = mpsc::channel::<Result<(), String>>();

        self.log(format!("Starting stream to '{}'", device.name));

        let handle = thread::spawn(move || {
            let result = chromecast::run_session(options, stop_for_thread, Some(status_tx))
                .map_err(|err| format!("{err:#}"));
            let _ = done_tx.send(result);
        });

        self.stream = Some(StreamState {
            device: device.name,
            host: device.ip,
            stop,
            handle,
            status_rx,
            done_rx,
        });
        self.error = None;
        self.volume = None;
        self.volume_target = None;
        if let Some(host) = volume_host {
            let _ = self.volume_tx.send(VolumeRequest::Get { host });
        }
    }

    fn stop_stream(&mut self, quit_after: bool) {
        if let Some(stream) = &self.stream {
            stream.stop.store(true, Ordering::SeqCst);
            self.quit_after_stream_stops = quit_after;
            self.log("Stop requested".to_string());
        }
    }

    fn adjust_volume(&mut self, delta: f32) {
        let host = self
            .stream
            .as_ref()
            .and_then(|stream| stream.host.clone())
            .unwrap_or_default();
        if host.is_empty() {
            return;
        }

        let current = self.volume_target.or(self.volume).unwrap_or(0.5);
        let new_level = (current + delta).clamp(0.0, 1.0);
        self.volume_target = Some(new_level);
        if self
            .volume_tx
            .send(VolumeRequest::Set {
                host,
                level: new_level,
            })
            .is_ok()
        {
            self.log(format!("Setting volume to {}%", (new_level * 100.0) as u32));
        } else {
            self.volume_target = None;
            self.log("Volume worker unavailable".to_string());
        }
    }

    fn pump_volume_events(&mut self) {
        while let Ok(event) = self.volume_events.try_recv() {
            match event {
                VolumeEvent::Current { host, level } => {
                    if !self.host_matches_active_stream(&host) {
                        continue;
                    }
                    self.volume = level;
                    if let Some(value) = level {
                        self.log(format!("Current volume {}%", (value * 100.0) as u32));
                    }
                }
                VolumeEvent::SetApplied { host, level } => {
                    if !self.host_matches_active_stream(&host) {
                        continue;
                    }
                    self.volume = Some(level);
                    self.volume_target = None;
                    self.log(format!("Volume set to {}%", (level * 100.0) as u32));
                }
                VolumeEvent::Failed {
                    host,
                    action,
                    message,
                } => {
                    if !self.host_matches_active_stream(&host) {
                        continue;
                    }
                    self.volume_target = None;
                    self.log(format!("Volume {action} failed: {message}"));
                }
            }
        }
    }

    fn host_matches_active_stream(&self, host: &str) -> bool {
        self.stream
            .as_ref()
            .and_then(|stream| stream.host.as_deref())
            .is_some_and(|active| active == host)
    }

    fn pump_stream_events(&mut self) {
        let mut pending_logs: Vec<String> = Vec::new();
        let mut stream_result: Option<Result<(), String>> = None;
        let mut finished = false;

        if let Some(stream) = &mut self.stream {
            while let Ok(line) = stream.status_rx.try_recv() {
                pending_logs.push(line);
            }

            if let Ok(result) = stream.done_rx.try_recv() {
                finished = true;
                stream_result = Some(result);
            }
        }

        for line in pending_logs {
            self.log(line);
        }

        if finished {
            if let Some(result) = stream_result {
                match result {
                    Ok(()) => self.log("Stream finished".to_string()),
                    Err(err) => self.error = Some(err),
                }
            }
            if let Some(stream) = self.stream.take() {
                let _ = stream.handle.join();
            }
            self.volume = None;
            self.volume_target = None;
            if self.quit_after_stream_stops {
                self.should_quit = true;
            }
            self.quit_after_stream_stops = false;
        }
    }

    fn log(&mut self, message: String) {
        self.logs.push(message);
        if self.logs.len() > 10 {
            let overflow = self.logs.len() - 10;
            self.logs.drain(0..overflow);
        }
    }

    fn apply_profile_to_session(&mut self) {
        let mut template = StartOptions::defaults_for_device("template".to_string());
        template.apply_profile(self.profile);
        self.framerate = template.framerate;
        self.crf = template.crf;
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(12),
                Constraint::Length(8),
            ])
            .split(frame.area());

        let title = if let Some(stream) = &self.stream {
            format!("DisplayFrost | Streaming to {}", stream.device)
        } else {
            "DisplayFrost | Chromecast Control".to_string()
        };

        let header =
            Paragraph::new(title).block(Block::default().borders(Borders::ALL).title("Status"));
        frame.render_widget(header, chunks[0]);

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(chunks[1]);

        let is_discovering = matches!(self.discovery, DiscoveryState::Running(_));
        let items: Vec<ListItem> = if is_discovering && self.devices.is_empty() {
            vec![ListItem::new("Discovering...")]
        } else if self.devices.is_empty() {
            vec![ListItem::new("No devices")]
        } else {
            self.devices
                .iter()
                .map(|device| {
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
                    ListItem::new(format!("{}{} ({location})", device.name, id_note))
                })
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Chromecast Devices"),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("-> ");

        let mut state = ListState::default();
        if !self.devices.is_empty() {
            state.select(Some(self.selected));
        }
        frame.render_stateful_widget(list, middle[0], &mut state);

        let mut detail_lines = vec![
            format!(
                "Audio: {} (A to toggle)",
                if self.audio { "on" } else { "off" }
            ),
            format!("Encoder: {} (E to cycle)", self.encoder.label()),
            format!("Profile: {} (P to cycle)", self.profile.label()),
            format!("Framerate: {} fps (+/-)", self.framerate),
            format!("CRF: {} ([/])", self.crf),
            format!("Port: {}", self.port),
        ];

        if let Some(stream) = &self.stream {
            detail_lines.push(format!("Streaming: {} (S to stop)", stream.device));
            if let Some(target) = self.volume_target {
                detail_lines.push(format!(
                    "Volume: {}% (applying, v/V)",
                    (target * 100.0) as u32
                ));
            } else if let Some(vol) = self.volume {
                detail_lines.push(format!("Volume: {}% (v/V)", (vol * 100.0) as u32));
            }
        } else {
            detail_lines.push("Press Enter to start streaming".to_string());
        }

        if let Some(error) = &self.error {
            detail_lines.push(String::new());
            detail_lines.push(format!("Error: {error}"));
        }

        let details = Paragraph::new(detail_lines.join("\n"))
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Session"));
        frame.render_widget(details, middle[1]);

        let controls = if self.stream.is_some() {
            "Streaming mode: [S] stop  [Q] stop + quit  [v/V] volume"
        } else {
            "Browse: [J/K] arrows  [R] refresh  [A] audio  [E] encoder  [P] profile  [Enter] start  [Q] quit"
        };

        let log_body = if self.logs.is_empty() {
            controls.to_string()
        } else {
            format!("{}\n\n{}", controls, self.logs.join("\n"))
        };

        let logs = Paragraph::new(log_body)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Logs"));
        frame.render_widget(logs, chunks[2]);
    }
}

fn spawn_volume_worker() -> (mpsc::Sender<VolumeRequest>, mpsc::Receiver<VolumeEvent>) {
    let (request_tx, request_rx) = mpsc::channel::<VolumeRequest>();
    let (event_tx, event_rx) = mpsc::channel::<VolumeEvent>();

    thread::spawn(move || {
        while let Ok(request) = request_rx.recv() {
            match request {
                VolumeRequest::Get { host } => match castv2::get_volume(&host) {
                    Ok(level) => {
                        let _ = event_tx.send(VolumeEvent::Current { host, level });
                    }
                    Err(err) => {
                        let _ = event_tx.send(VolumeEvent::Failed {
                            host,
                            action: "read",
                            message: err.to_string(),
                        });
                    }
                },
                VolumeRequest::Set { host, level } => match castv2::set_volume(&host, level) {
                    Ok(()) => {
                        let _ = event_tx.send(VolumeEvent::SetApplied { host, level });
                    }
                    Err(err) => {
                        let _ = event_tx.send(VolumeEvent::Failed {
                            host,
                            action: "change",
                            message: err.to_string(),
                        });
                    }
                },
            }
        }
    });

    (request_tx, event_rx)
}
