use crate::util;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RTSP_PATH: &str = "/wfd1.0";
const STATE_FILE_NAME: &str = "miracast-native.state";
const USER_AGENT: &str = "displayfrost-native/0.1";
const P2P_POLL_MS: u64 = 500;

const GET_PARAMETER_BODY: &str = concat!(
    "wfd_video_formats\r\n",
    "wfd_audio_codecs\r\n",
    "wfd_client_rtp_ports\r\n",
    "wfd_content_protection\r\n",
    "wfd_display_edid\r\n",
    "wfd_coupled_sink\r\n",
    "wfd_trigger_method\r\n"
);

const SET_PARAMETER_BODY: &str = concat!(
    "wfd_video_formats: 30 00 02 02 00008c60 00000000 00000000 00 0000 0000 00 none none\r\n",
    "wfd_audio_codecs: LPCM 00000002 00\r\n",
    "wfd_client_rtp_ports: RTP/AVP/UDP;unicast 19000 0 mode=play\r\n",
    "wfd_content_protection: none\r\n"
);

#[derive(Debug, Serialize, Deserialize)]
struct NativeMiracastState {
    host: String,
    rtsp_port: u16,
    session_id: Option<String>,
    next_cseq: u32,
    started_at_epoch: u64,
    last_success_epoch: u64,
    last_status: String,
}

#[derive(Debug)]
struct RtspResponse {
    code: u16,
    reason: String,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Debug)]
struct NativeRtspSession {
    host: String,
    rtsp_port: u16,
    cseq: u32,
    session_id: Option<String>,
    stream: TcpStream,
}

impl NativeRtspSession {
    fn connect(host: &str, rtsp_port: u16, timeout: Duration) -> Result<Self> {
        let mut resolved = (host, rtsp_port)
            .to_socket_addrs()
            .with_context(|| format!("Failed to resolve {host}:{rtsp_port}"))?;
        let addr = resolved
            .next()
            .ok_or_else(|| anyhow!("No address resolved for {host}:{rtsp_port}"))?;
        let stream = TcpStream::connect_timeout(&addr, timeout)
            .with_context(|| format!("Failed to connect to Miracast sink at {addr}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .with_context(|| "Failed to set RTSP read timeout")?;
        stream
            .set_write_timeout(Some(timeout))
            .with_context(|| "Failed to set RTSP write timeout")?;

        Ok(Self {
            host: host.to_string(),
            rtsp_port,
            cseq: 1,
            session_id: None,
            stream,
        })
    }

    fn set_session_id(&mut self, session_id: Option<String>) {
        self.session_id = session_id;
    }

    fn next_cseq(&self) -> u32 {
        self.cseq
    }

    fn options(&mut self) -> Result<RtspResponse> {
        self.send_request("OPTIONS", None, None)
    }

    fn get_parameter(&mut self) -> Result<RtspResponse> {
        self.send_request(
            "GET_PARAMETER",
            Some("text/parameters"),
            Some(GET_PARAMETER_BODY),
        )
    }

    fn set_parameter(&mut self) -> Result<RtspResponse> {
        self.send_request(
            "SET_PARAMETER",
            Some("text/parameters"),
            Some(SET_PARAMETER_BODY),
        )
    }

    fn teardown(&mut self) -> Result<RtspResponse> {
        self.send_request("TEARDOWN", None, None)
    }

    fn send_request(
        &mut self,
        method: &str,
        content_type: Option<&str>,
        body: Option<&str>,
    ) -> Result<RtspResponse> {
        let uri = format!("rtsp://{}:{}{}", self.host, self.rtsp_port, RTSP_PATH);
        let payload = body.unwrap_or("");

        let mut request = String::new();
        request.push_str(&format!("{method} {uri} RTSP/1.0\r\n"));
        request.push_str(&format!("CSeq: {}\r\n", self.cseq));
        request.push_str(&format!("User-Agent: {USER_AGENT}\r\n"));
        request.push_str("Connection: keep-alive\r\n");
        if let Some(session_id) = &self.session_id {
            request.push_str(&format!("Session: {session_id}\r\n"));
        }
        if let Some(content_type) = content_type {
            request.push_str(&format!("Content-Type: {content_type}\r\n"));
        }
        if !payload.is_empty() {
            request.push_str(&format!("Content-Length: {}\r\n", payload.len()));
        }
        request.push_str("\r\n");
        if !payload.is_empty() {
            request.push_str(payload);
        }

        self.stream
            .write_all(request.as_bytes())
            .with_context(|| format!("Failed to send RTSP {method} request"))?;
        self.stream
            .flush()
            .with_context(|| format!("Failed to flush RTSP {method} request"))?;

        let response = read_rtsp_response(&mut self.stream)?;

        if let Some(session) = response.headers.get("session") {
            let session_id = session
                .split(';')
                .next()
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty());
            if session_id.is_some() {
                self.session_id = session_id;
            }
        }

        self.cseq = self.cseq.saturating_add(1);
        Ok(response)
    }
}

pub fn discover_cmd(interface: Option<&str>, timeout_secs: u64) -> Result<()> {
    ensure_wpa_cli_available()?;

    let iface = resolve_p2p_interface(interface);
    println!("Miracast P2P discover on interface: {iface}");

    let start = wpa_cli(&iface, &["p2p_find"])?;
    if is_fail_output(&start) {
        bail!("p2p_find failed on {iface}: {}", start.trim());
    }

    let wait = Duration::from_secs(timeout_secs.max(1));
    thread::sleep(wait);

    let peers_raw = wpa_cli(&iface, &["p2p_peers"])?;
    let _ = wpa_cli(&iface, &["p2p_stop_find"]);

    let peers = parse_p2p_peers(&peers_raw);
    if peers.is_empty() {
        println!("No Miracast peers found.");
        println!("Tip: enable screen mirroring on the TV and retry.");
        return Ok(());
    }

    println!("Found {} Miracast peer(s):", peers.len());
    for (idx, peer) in peers.iter().enumerate() {
        let details = wpa_cli(&iface, &["p2p_peer", peer]).unwrap_or_default();
        let attrs = parse_key_value_lines(&details);
        let name = attrs
            .get("device_name")
            .map(String::as_str)
            .unwrap_or("(unknown)");
        let model = attrs
            .get("model_name")
            .map(String::as_str)
            .unwrap_or("(n/a)");
        let status = attrs.get("status").map(String::as_str).unwrap_or("(n/a)");
        println!(
            "{}. {} [{}] model={} status={}",
            idx + 1,
            name,
            peer,
            model,
            status
        );
    }

    println!("Next: `displayfrost miracast connect --peer <ADDR> --interface {iface}`");
    Ok(())
}

pub fn connect_cmd(
    peer: &str,
    interface: Option<&str>,
    timeout_secs: u64,
    pin: Option<&str>,
    force: bool,
) -> Result<()> {
    ensure_wpa_cli_available()?;

    let peer = peer.trim();
    if !looks_like_peer_addr(peer) {
        bail!("Invalid peer address '{peer}'. Expected format xx:xx:xx:xx:xx:xx");
    }

    let iface = resolve_p2p_interface(interface);
    println!("Miracast P2P connect on interface: {iface}");

    if force {
        let _ = wpa_cli(&iface, &["p2p_stop_find"]);
        let _ = wpa_cli(&iface, &["p2p_flush"]);
    }

    let known_peers = parse_p2p_peers(&wpa_cli(&iface, &["p2p_peers"]).unwrap_or_default());
    if !contains_peer(&known_peers, peer) {
        let start = wpa_cli(&iface, &["p2p_find"])?;
        if is_fail_output(&start) {
            bail!("p2p_find failed before connect: {}", start.trim());
        }
        thread::sleep(Duration::from_secs(3));
    }

    let connect_response = if let Some(pin_value) = pin {
        wpa_cli(&iface, &["p2p_connect", peer, pin_value, "keypad"])?
    } else {
        wpa_cli(&iface, &["p2p_connect", peer, "pbc"])?
    };
    if is_fail_output(&connect_response) {
        bail!(
            "p2p_connect failed for {peer} on {iface}: {}",
            connect_response.trim()
        );
    }

    println!("p2p_connect accepted: {}", connect_response.trim());
    println!(
        "Waiting for P2P link completion ({}s)...",
        timeout_secs.max(1)
    );

    let timeout = Duration::from_secs(timeout_secs.max(1));
    let start = Instant::now();
    loop {
        let status_raw = wpa_cli(&iface, &["status"]).unwrap_or_default();
        let status = parse_key_value_lines(&status_raw);
        let wpa_state = status
            .get("wpa_state")
            .map(String::as_str)
            .unwrap_or_default();
        let p2p_device_address = status
            .get("p2p_device_address")
            .map(String::as_str)
            .unwrap_or_default();
        if wpa_state.eq_ignore_ascii_case("COMPLETED")
            && (p2p_device_address.is_empty() || p2p_device_address.eq_ignore_ascii_case(peer))
        {
            println!("P2P link established with {peer}.");
            if let Some(ip) = infer_peer_ip(peer) {
                println!("Detected sink IP via ARP/neighbor table: {ip}");
                println!("Next: `displayfrost miracast start --host {ip}`");
            }
            return Ok(());
        }

        if start.elapsed() >= timeout {
            break;
        }
        thread::sleep(Duration::from_millis(P2P_POLL_MS));
    }

    println!("Connect request sent, but link is not completed yet.");
    println!(
        "Check sink prompt and run `displayfrost miracast discover --interface {iface}` again."
    );
    Ok(())
}

pub fn start_cmd(host: &str, rtsp_port: u16, timeout_secs: u64, force: bool) -> Result<()> {
    let host = host.trim();
    if host.is_empty() {
        bail!("Missing Miracast sink host");
    }
    let timeout = Duration::from_secs(timeout_secs.max(1));

    if let Some(existing) = read_state()?
        && !force
    {
        bail!(
            "Native Miracast session already tracked for {}:{} (status: {}). Use --force to replace.",
            existing.host,
            existing.rtsp_port,
            existing.last_status
        );
    }

    let mut session = NativeRtspSession::connect(host, rtsp_port, timeout)?;
    let options = session.options()?;
    ensure_success("OPTIONS", &options)?;

    let mut last_status = "OPTIONS_OK".to_string();
    println!("RTSP OPTIONS ok: {} {}", options.code, options.reason);

    match session.get_parameter() {
        Ok(resp) if is_success(resp.code) => {
            last_status = "GET_PARAMETER_OK".to_string();
            println!("RTSP GET_PARAMETER ok: {} {}", resp.code, resp.reason);
            if !resp.body.trim().is_empty() {
                println!("Sink capabilities received ({} bytes).", resp.body.len());
            }
        }
        Ok(resp) => {
            println!(
                "Warning: GET_PARAMETER returned {} {}",
                resp.code, resp.reason
            );
        }
        Err(err) => {
            println!("Warning: GET_PARAMETER failed: {err:#}");
        }
    }

    match session.set_parameter() {
        Ok(resp) if is_success(resp.code) => {
            last_status = "SET_PARAMETER_OK".to_string();
            println!("RTSP SET_PARAMETER ok: {} {}", resp.code, resp.reason);
        }
        Ok(resp) => {
            println!(
                "Warning: SET_PARAMETER returned {} {}",
                resp.code, resp.reason
            );
        }
        Err(err) => {
            println!("Warning: SET_PARAMETER failed: {err:#}");
        }
    }

    let state = NativeMiracastState {
        host: host.to_string(),
        rtsp_port,
        session_id: session.session_id.clone(),
        next_cseq: session.next_cseq(),
        started_at_epoch: now_epoch(),
        last_success_epoch: now_epoch(),
        last_status,
    };
    write_state(&state)?;

    println!(
        "Native Miracast control session prepared for {}:{}.",
        state.host, state.rtsp_port
    );
    if let Some(session_id) = &state.session_id {
        println!("Session id: {session_id}");
    }
    println!(
        "Use `displayfrost miracast status` to verify and `displayfrost miracast stop` to teardown state."
    );
    Ok(())
}

pub fn status_cmd() -> Result<()> {
    let Some(state) = read_state()? else {
        println!("No native Miracast session tracked.");
        return Ok(());
    };

    println!("Native Miracast session:");
    println!("  sink: {}:{}", state.host, state.rtsp_port);
    println!("  status: {}", state.last_status);
    println!("  next cseq: {}", state.next_cseq);
    if let Some(session_id) = &state.session_id {
        println!("  session: {session_id}");
    } else {
        println!("  session: (none)");
    }

    let timeout = Duration::from_secs(3);
    match NativeRtspSession::connect(&state.host, state.rtsp_port, timeout)
        .and_then(|mut session| session.options())
    {
        Ok(resp) if is_success(resp.code) => {
            println!(
                "  control channel: reachable (OPTIONS {} {})",
                resp.code, resp.reason
            );
        }
        Ok(resp) => {
            println!(
                "  control channel: reachable but non-success response ({} {})",
                resp.code, resp.reason
            );
        }
        Err(err) => {
            println!("  control channel: unreachable ({err:#})");
        }
    }

    Ok(())
}

pub fn stop_cmd() -> Result<()> {
    let Some(state) = read_state()? else {
        println!("No native Miracast session tracked.");
        return Ok(());
    };

    let timeout = Duration::from_secs(3);
    let teardown_result = (|| -> Result<()> {
        let mut session = NativeRtspSession::connect(&state.host, state.rtsp_port, timeout)?;
        session.set_session_id(state.session_id.clone());
        let resp = session.teardown()?;
        if is_success(resp.code) {
            println!("RTSP TEARDOWN ok: {} {}", resp.code, resp.reason);
        } else {
            println!(
                "Warning: RTSP TEARDOWN returned {} {}",
                resp.code, resp.reason
            );
        }
        Ok(())
    })();

    clear_state()?;

    if let Err(err) = teardown_result {
        println!("Cleared local native session state; remote teardown failed: {err:#}");
    } else {
        println!("Native Miracast session cleared.");
    }
    Ok(())
}

fn ensure_success(method: &str, response: &RtspResponse) -> Result<()> {
    if is_success(response.code) {
        return Ok(());
    }
    bail!(
        "{method} failed with RTSP status {} {}",
        response.code,
        response.reason
    )
}

fn is_success(code: u16) -> bool {
    (200..300).contains(&code)
}

fn read_rtsp_response(stream: &mut TcpStream) -> Result<RtspResponse> {
    let mut raw: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];

    let header_end = loop {
        let n = stream
            .read(&mut chunk)
            .with_context(|| "Failed to read RTSP response header")?;
        if n == 0 {
            bail!("RTSP socket closed while waiting for response");
        }
        raw.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subslice(&raw, b"\r\n\r\n") {
            break pos + 4;
        }
        if raw.len() > 64 * 1024 {
            bail!("RTSP response headers too large");
        }
    };

    let header_text = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let mut lines = header_text.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| anyhow!("Missing RTSP status line"))?;
    let mut status_parts = status_line.splitn(3, ' ');
    let protocol = status_parts.next().unwrap_or_default();
    if !protocol.starts_with("RTSP/") {
        bail!("Invalid RTSP status line: {status_line}");
    }
    let code = status_parts
        .next()
        .ok_or_else(|| anyhow!("Missing RTSP status code"))?
        .parse::<u16>()
        .with_context(|| format!("Invalid RTSP status code in: {status_line}"))?;
    let reason = status_parts.next().unwrap_or_default().trim().to_string();

    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    while raw.len() < header_end + content_length {
        let n = stream
            .read(&mut chunk)
            .with_context(|| "Failed to read RTSP response body")?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..n]);
    }

    if raw.len() < header_end + content_length {
        bail!(
            "RTSP response body truncated (expected {content_length} bytes, got {})",
            raw.len().saturating_sub(header_end)
        );
    }

    let body = String::from_utf8_lossy(&raw[header_end..header_end + content_length]).to_string();
    Ok(RtspResponse {
        code,
        reason,
        headers,
        body,
    })
}

fn find_subslice(data: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || data.len() < needle.len() {
        return None;
    }
    data.windows(needle.len())
        .position(|window| window == needle)
}

fn state_file_path() -> Result<PathBuf> {
    let base = if let Ok(path) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(path)
    } else {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .with_context(|| "HOME is not set and XDG_CACHE_HOME is missing")?;
        home.join(".cache")
    };

    let preferred = base.join("displayfrost");
    if fs::create_dir_all(&preferred).is_ok() {
        return Ok(preferred.join(STATE_FILE_NAME));
    }

    let fallback = std::env::temp_dir().join("displayfrost-cache");
    fs::create_dir_all(&fallback)
        .with_context(|| format!("Failed to create cache dir {}", fallback.display()))?;
    Ok(fallback.join(STATE_FILE_NAME))
}

fn read_state() -> Result<Option<NativeMiracastState>> {
    let path = state_file_path()?;
    if !path.is_file() {
        return Ok(None);
    }

    let raw =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let state: NativeMiracastState = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(state))
}

fn write_state(state: &NativeMiracastState) -> Result<()> {
    let path = state_file_path()?;
    let raw = serde_json::to_string_pretty(state).with_context(|| "Failed to serialize state")?;
    fs::write(&path, raw).with_context(|| format!("Failed to write {}", path.display()))
}

fn clear_state() -> Result<()> {
    let path = state_file_path()?;
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn ensure_wpa_cli_available() -> Result<()> {
    if util::command_exists("wpa_cli") {
        Ok(())
    } else {
        bail!("wpa_cli is not installed (required for native Miracast P2P)");
    }
}

fn resolve_p2p_interface(interface: Option<&str>) -> String {
    if let Some(interface) = interface {
        let trimmed = interface.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    detect_wifi_interface_from_nmcli()
        .or_else(detect_wifi_interface_from_iw)
        .unwrap_or_else(|| "wlan0".to_string())
}

fn detect_wifi_interface_from_nmcli() -> Option<String> {
    if !util::command_exists("nmcli") {
        return None;
    }

    let output = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "device", "status"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut fallback: Option<String> = None;
    for line in text.lines() {
        let mut parts = line.split(':');
        let device = parts.next().unwrap_or_default().trim();
        let kind = parts.next().unwrap_or_default().trim();
        let state = parts.next().unwrap_or_default().trim();
        if kind != "wifi" || device.is_empty() || device == "--" {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(device.to_string());
        }
        if state != "unavailable" {
            return Some(device.to_string());
        }
    }
    fallback
}

fn wpa_cli(interface: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("wpa_cli")
        .args(["-i", interface])
        .args(args)
        .output()
        .with_context(|| format!("Failed to run wpa_cli -i {interface} {}", args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let combined = format!("{} {}", stdout, stderr).to_ascii_lowercase();
        let mut hint = String::new();
        if combined.contains("no such file or directory") {
            hint = format!(
                " | hint: start wpa_supplicant for interface {interface} so control socket exists"
            );
        } else if combined.contains("permission denied")
            || combined.contains("operation not permitted")
        {
            hint = " | hint: run with sudo or grant your user access to /run/wpa_supplicant control sockets"
                .to_string();
        }
        bail!(
            "wpa_cli command failed (exit {}): {}{}{}",
            output.status.code().unwrap_or(-1),
            if stdout.trim().is_empty() {
                "".to_string()
            } else {
                format!("stdout='{}' ", stdout.trim())
            },
            stderr,
            hint
        );
    }
    Ok(stdout)
}

fn parse_key_value_lines(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    map
}

fn parse_p2p_peers(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.eq_ignore_ascii_case("ok"))
        .filter(|line| !line.to_ascii_uppercase().starts_with("FAIL"))
        .filter(|line| looks_like_peer_addr(line))
        .map(ToString::to_string)
        .collect()
}

fn contains_peer(peers: &[String], target: &str) -> bool {
    peers.iter().any(|peer| peer.eq_ignore_ascii_case(target))
}

fn looks_like_peer_addr(value: &str) -> bool {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 6 {
        return false;
    }
    parts
        .iter()
        .all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_fail_output(value: &str) -> bool {
    let trimmed = value.trim().to_ascii_uppercase();
    trimmed.starts_with("FAIL") || trimmed.contains("UNKNOWN COMMAND")
}

fn detect_wifi_interface_from_iw() -> Option<String> {
    if !util::command_exists("iw") {
        return None;
    }
    let output = Command::new("iw").arg("dev").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("Interface ")
            && !name.trim().is_empty()
        {
            return Some(name.trim().to_string());
        }
    }
    None
}

fn infer_peer_ip(peer: &str) -> Option<String> {
    if !util::command_exists("ip") {
        return None;
    }
    let output = Command::new("ip").args(["neigh", "show"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let wanted = peer.to_ascii_lowercase();
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.contains(&wanted) {
            continue;
        }
        if let Some(ip) = line.split_whitespace().next()
            && ip.parse::<std::net::IpAddr>().is_ok()
        {
            return Some(ip.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_p2p_peers_filters_noise() {
        let raw = "\nOK\nFAIL-BUSY\n11:22:33:44:55:66\nnot-a-peer\nAA:BB:CC:DD:EE:FF\n";
        let peers = parse_p2p_peers(raw);
        assert_eq!(
            peers,
            vec![
                "11:22:33:44:55:66".to_string(),
                "AA:BB:CC:DD:EE:FF".to_string()
            ]
        );
    }

    #[test]
    fn looks_like_peer_addr_validates_mac_like_format() {
        assert!(looks_like_peer_addr("11:22:33:44:55:66"));
        assert!(looks_like_peer_addr("aa:bb:cc:dd:ee:ff"));
        assert!(!looks_like_peer_addr("11:22:33:44:55"));
        assert!(!looks_like_peer_addr("11:22:33:44:55:6g"));
        assert!(!looks_like_peer_addr("11-22-33-44-55-66"));
    }

    #[test]
    fn parse_key_value_lines_lowercases_keys() {
        let map = parse_key_value_lines("WPA_STATE=COMPLETED\np2p_device_address=AA:BB\n");
        assert_eq!(map.get("wpa_state").map(String::as_str), Some("COMPLETED"));
        assert_eq!(
            map.get("p2p_device_address").map(String::as_str),
            Some("AA:BB")
        );
    }
}
