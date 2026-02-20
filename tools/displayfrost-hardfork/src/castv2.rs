use anyhow::{Context, Result, anyhow, bail};
use native_tls::{TlsConnector, TlsStream};
use prost::Message;
use serde_json::{Value, json};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

#[derive(Debug, Clone)]
struct CastSession {
    app_id: String,
    session_id: String,
    transport_id: String,
}

pub fn play_media(host: &str, url: &str, content_type: &str, stream_type: &str) -> Result<()> {
    let mut conn = CastConnection::connect(host, 8009, Duration::from_secs(10))
        .with_context(|| format!("Failed to connect to cast host {host}:8009"))?;

    let sender = "sender-0";
    conn.send_connect(sender, "receiver-0")?;

    let existing = conn
        .receiver_status(sender)
        .with_context(|| "failed to fetch initial receiver status")?;
    let mut session = extract_session(existing.as_ref());

    if session
        .as_ref()
        .is_none_or(|s| s.app_id != DEFAULT_MEDIA_RECEIVER_APP_ID)
    {
        session = Some(
            conn.launch_default_media_receiver(sender)
                .with_context(|| "failed to launch default media receiver app")?,
        );
    }

    let session = session.ok_or_else(|| anyhow!("Could not resolve cast session"))?;

    conn.send_connect(sender, &session.transport_id)?;

    let request_id = conn.next_request_id();
    conn.send_json(
        sender,
        &session.transport_id,
        NS_MEDIA,
        json!({
            "type": "LOAD",
            "requestId": request_id,
            "sessionId": session.session_id,
            "autoplay": true,
            "currentTime": 0,
            "media": {
                "contentId": url,
                "streamType": stream_type,
                "contentType": content_type,
                "metadata": {
                    "metadataType": 0,
                    "title": "DisplayFrost",
                    "subtitle": "Live desktop stream",
                    "images": []
                },
                "customData": {
                    "source": "DisplayFrost",
                    "kind": "desktop-mirroring"
                }
            }
        }),
    )?;

    let ack = conn
        .wait_for_json(sender, Duration::from_secs(8), |value| {
            value
                .get("requestId")
                .and_then(Value::as_i64)
                .map(|v| v == request_id as i64)
                .unwrap_or(false)
                || value
                    .get("type")
                    .and_then(Value::as_str)
                    .map(|typ| typ == "MEDIA_STATUS" || typ == "LOAD_FAILED")
                    .unwrap_or(false)
        })
        .with_context(|| "no acknowledgement from receiver after LOAD")?;

    if ack
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| t == "LOAD_FAILED")
    {
        bail!("Cast receiver returned LOAD_FAILED: {ack}");
    }

    Ok(())
}

pub fn stop_media(host: &str) -> Result<()> {
    let mut conn = CastConnection::connect(host, 8009, Duration::from_secs(10))
        .with_context(|| format!("Failed to connect to cast host {host}:8009"))?;

    let sender = "sender-0";
    conn.send_connect(sender, "receiver-0")?;

    let status = conn
        .receiver_status(sender)
        .with_context(|| "failed to fetch receiver status before STOP")?;
    let Some(session) = extract_session(status.as_ref()) else {
        return Ok(());
    };

    let request_id = conn.next_request_id();
    conn.send_json(
        sender,
        "receiver-0",
        NS_RECEIVER,
        json!({
            "type": "STOP",
            "requestId": request_id,
            "sessionId": session.session_id,
        }),
    )?;

    let _ = conn.wait_for_json(sender, Duration::from_secs(5), |value| {
        value
            .get("requestId")
            .and_then(Value::as_i64)
            .map(|v| v == request_id as i64)
            .unwrap_or(false)
            || value
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "RECEIVER_STATUS")
    });

    Ok(())
}

pub fn media_player_state(host: &str) -> Result<Option<String>> {
    let mut conn = CastConnection::connect(host, 8009, Duration::from_secs(10))
        .with_context(|| format!("Failed to connect to cast host {host}:8009"))?;

    let sender = "sender-0";
    conn.send_connect(sender, "receiver-0")?;

    let receiver_status = conn
        .receiver_status(sender)
        .with_context(|| "failed to fetch receiver status before media GET_STATUS")?;
    let Some(session) = extract_session(receiver_status.as_ref()) else {
        return Ok(None);
    };

    conn.send_connect(sender, &session.transport_id)?;

    let request_id = conn.next_request_id();
    conn.send_json(
        sender,
        &session.transport_id,
        NS_MEDIA,
        json!({"type":"GET_STATUS","requestId":request_id}),
    )?;

    let media_status = conn.wait_for_json(sender, Duration::from_secs(4), |value| {
        value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "MEDIA_STATUS")
    })?;

    let state = media_status
        .get("status")
        .and_then(Value::as_array)
        .and_then(|statuses| statuses.first())
        .and_then(|entry| entry.get("playerState"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    Ok(state)
}

pub fn set_volume(host: &str, level: f32) -> Result<()> {
    let level = level.clamp(0.0, 1.0);
    let mut conn = CastConnection::connect(host, 8009, Duration::from_secs(10))
        .with_context(|| format!("Failed to connect to cast host {host}:8009"))?;

    let sender = "sender-0";
    conn.send_connect(sender, "receiver-0")?;

    let request_id = conn.next_request_id();
    conn.send_json(
        sender,
        "receiver-0",
        NS_RECEIVER,
        json!({
            "type": "SET_VOLUME",
            "requestId": request_id,
            "volume": {
                "level": level
            }
        }),
    )?;

    let _ = conn.wait_for_json(sender, Duration::from_secs(5), |value| {
        value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "RECEIVER_STATUS")
    });

    Ok(())
}

pub fn get_volume(host: &str) -> Result<Option<f32>> {
    let mut conn = CastConnection::connect(host, 8009, Duration::from_secs(10))
        .with_context(|| format!("Failed to connect to cast host {host}:8009"))?;

    let sender = "sender-0";
    conn.send_connect(sender, "receiver-0")?;

    let status = conn
        .receiver_status(sender)
        .with_context(|| "failed to fetch receiver status for volume")?;

    let volume = status
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|v| v.get("volume"))
        .and_then(|v| v.get("level"))
        .and_then(Value::as_f64)
        .map(|v| v as f32);

    Ok(volume)
}

fn extract_session(status_value: Option<&Value>) -> Option<CastSession> {
    let applications = status_value?
        .get("status")?
        .get("applications")?
        .as_array()?;
    let app = applications.first()?;

    let app_id = app.get("appId")?.as_str()?.to_string();
    let session_id = app.get("sessionId")?.as_str()?.to_string();
    let transport_id = app.get("transportId")?.as_str()?.to_string();

    Some(CastSession {
        app_id,
        session_id,
        transport_id,
    })
}

struct CastConnection {
    stream: TlsStream<TcpStream>,
    request_id: u32,
}

impl CastConnection {
    fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self> {
        let addr = format!("{host}:{port}");
        let sockaddr = addr
            .to_socket_addrs()
            .with_context(|| format!("Could not resolve {addr}"))?
            .next()
            .ok_or_else(|| anyhow!("No socket address for {addr}"))?;

        let tcp = TcpStream::connect_timeout(&sockaddr, timeout)
            .with_context(|| format!("TCP connect timeout to {addr}"))?;
        tcp.set_read_timeout(Some(Duration::from_millis(500)))
            .with_context(|| "Failed to set read timeout")?;
        tcp.set_write_timeout(Some(Duration::from_secs(5)))
            .with_context(|| "Failed to set write timeout")?;

        let tls = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .with_context(|| "Failed to build TLS connector")?
            .connect(host, tcp)
            .with_context(|| format!("TLS handshake failed for {host}"))?;

        Ok(Self {
            stream: tls,
            request_id: 1,
        })
    }

    fn next_request_id(&mut self) -> u32 {
        let id = self.request_id;
        self.request_id = self.request_id.saturating_add(1);
        id
    }

    fn send_connect(&mut self, source_id: &str, destination_id: &str) -> Result<()> {
        self.send_json(
            source_id,
            destination_id,
            NS_CONNECTION,
            json!({
                "type":"CONNECT",
                "origin":{},
                "userAgent":"DisplayFrost",
                "senderInfo":{
                    "sdkType":2,
                    "version":"15.605.1.3",
                    "browserVersion":"44.0.2403.30",
                    "platform":4,
                    "systemVersion":"Linux",
                    "connectionType":1
                }
            }),
        )
    }

    fn receiver_status(&mut self, sender: &str) -> Result<Option<Value>> {
        let request_id = self.next_request_id();
        self.send_json(
            sender,
            "receiver-0",
            NS_RECEIVER,
            json!({"type":"GET_STATUS","requestId":request_id}),
        )?;

        let status = self.wait_for_json(sender, Duration::from_secs(5), |value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "RECEIVER_STATUS")
        })?;

        Ok(Some(status))
    }

    fn launch_default_media_receiver(&mut self, sender: &str) -> Result<CastSession> {
        let request_id = self.next_request_id();
        self.send_json(
            sender,
            "receiver-0",
            NS_RECEIVER,
            json!({
                "type":"LAUNCH",
                "appId":DEFAULT_MEDIA_RECEIVER_APP_ID,
                "requestId":request_id
            }),
        )?;

        let status = self.wait_for_json(sender, Duration::from_secs(10), |value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "RECEIVER_STATUS")
        })?;

        extract_session(Some(&status)).ok_or_else(|| anyhow!("Failed to parse LAUNCH status"))
    }

    fn send_json(
        &mut self,
        source_id: &str,
        destination_id: &str,
        namespace: &str,
        payload: Value,
    ) -> Result<()> {
        let payload_utf8 = serde_json::to_string(&payload)
            .with_context(|| format!("Failed to serialize payload for namespace {namespace}"))?;

        let message = CastMessage {
            protocol_version: ProtocolVersion::Castv210 as i32,
            source_id: source_id.to_string(),
            destination_id: destination_id.to_string(),
            namespace: namespace.to_string(),
            payload_type: PayloadType::String as i32,
            payload_utf8: Some(payload_utf8),
            payload_binary: None,
        };

        let mut encoded = Vec::new();
        message
            .encode(&mut encoded)
            .with_context(|| "Failed to encode cast protobuf message")?;

        let len = encoded.len() as u32;
        self.stream
            .write_all(&len.to_be_bytes())
            .with_context(|| "Failed to write cast frame length")?;
        self.stream
            .write_all(&encoded)
            .with_context(|| "Failed to write cast frame payload")?;
        self.stream
            .flush()
            .with_context(|| "Failed to flush cast frame")?;

        Ok(())
    }

    fn wait_for_json<F>(
        &mut self,
        sender: &str,
        timeout: Duration,
        mut predicate: F,
    ) -> Result<Value>
    where
        F: FnMut(&Value) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut last_json: Option<Value> = None;

        while Instant::now() < deadline {
            let Some(msg) = self.read_message()? else {
                continue;
            };

            if msg.namespace == NS_HEARTBEAT
                && matches!(msg.payload_utf8.as_deref(), Some(p) if p.contains("\"PING\""))
            {
                let _ =
                    self.send_json(sender, &msg.source_id, NS_HEARTBEAT, json!({"type":"PONG"}));
                continue;
            }

            let Some(payload_str) = msg.payload_utf8 else {
                continue;
            };

            let Ok(value) = serde_json::from_str::<Value>(&payload_str) else {
                continue;
            };

            if predicate(&value) {
                return Ok(value);
            }

            last_json = Some(value);
        }

        let hint = last_json
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<no payload>".to_string());
        bail!("Timed out waiting for cast response. Last payload: {hint}")
    }

    fn read_message(&mut self) -> Result<Option<CastMessage>> {
        let mut len_buf = [0u8; 4];
        match self.stream.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(err) if is_timeout(&err) => return Ok(None),
            Err(err) => return Err(err).with_context(|| "Failed to read cast frame length"),
        }

        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        self.stream
            .read_exact(&mut payload)
            .with_context(|| "Failed to read cast frame payload")?;

        let message = CastMessage::decode(payload.as_slice())
            .with_context(|| "Failed to decode cast protobuf message")?;

        Ok(Some(message))
    }
}

fn is_timeout(err: &std::io::Error) -> bool {
    matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
enum ProtocolVersion {
    Castv210 = 0,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
enum PayloadType {
    String = 0,
    Binary = 1,
}

#[derive(Clone, PartialEq, prost::Message)]
struct CastMessage {
    #[prost(enumeration = "ProtocolVersion", tag = "1")]
    protocol_version: i32,
    #[prost(string, tag = "2")]
    source_id: String,
    #[prost(string, tag = "3")]
    destination_id: String,
    #[prost(string, tag = "4")]
    namespace: String,
    #[prost(enumeration = "PayloadType", tag = "5")]
    payload_type: i32,
    #[prost(string, optional, tag = "6")]
    payload_utf8: Option<String>,
    #[prost(bytes, optional, tag = "7")]
    payload_binary: Option<Vec<u8>>,
}
