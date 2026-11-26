// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test support: a real server on an ephemeral loopback port with its own
//! temporary `LOG_DIR`, a minimal RFC 6455 WebSocket client over plain
//! `tokio::net::TcpStream` (no extra crates), and statistics-log helpers.
//!
//! The client sends the sample key from RFC 6455 section 1.3, whose
//! `Sec-WebSocket-Accept` is known, so the real axum handshake is checked
//! without any hashing. Client frames are masked, as the RFC requires.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::Router;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tower::ServiceExt;
use trafficlights_proto::{
    FirmwareInfo, FirmwareKind, FirmwareMode, JsonlEvent, LobbyClientMsg, LobbyServerMsg,
    SessionClientMsg, SessionServerMsg, STATS_SCHEMA_VERSION,
};
use trafficlights_server::{build_router_with_state, AppState, ServerConfig};

/// The sample `Sec-WebSocket-Key` from RFC 6455 section 1.3.
const WS_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
/// Its `Sec-WebSocket-Accept`, from the same section.
const WS_ACCEPT: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
/// The longest any single expected event may take to arrive.
pub const STEP: Duration = Duration::from_secs(5);
/// The authentication window used by test servers.
pub const AUTH_WINDOW: Duration = Duration::from_millis(200);

/// A fresh, empty, uniquely named directory under the system temp dir.
pub fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("trafficlights-{label}-{}", uuid::Uuid::now_v7()))
}

/// A server running in this test's runtime.
pub struct TestServer {
    pub addr: SocketAddr,
    pub state: Arc<AppState>,
    router: Router,
    log_dir: PathBuf,
    task: JoinHandle<()>,
}

/// A session created through the lobby.
pub struct Created {
    pub id: String,
    pub token: String,
}

impl TestServer {
    /// A server with test defaults (its own `LOG_DIR`, a short auth
    /// window), adjusted by `edit`.
    pub async fn start(edit: impl FnOnce(&mut ServerConfig)) -> Self {
        let log_dir = temp_dir("test-logs");
        let mut config = ServerConfig {
            log_dir: log_dir.clone(),
            auth_window: AUTH_WINDOW,
            ..ServerConfig::default()
        };
        edit(&mut config);
        let state = Arc::new(AppState::new(config));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local address");
        let router = build_router_with_state(Arc::clone(&state));
        let served = router.clone();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, served).await;
        });
        Self {
            addr,
            state,
            router,
            log_dir,
            task,
        }
    }

    /// Open a WebSocket at `path`, or return the HTTP status of a refused
    /// upgrade.
    pub async fn ws(&self, path: &str, origin: Option<&str>) -> Result<WsClient, u16> {
        WsClient::connect(self.addr, path, origin).await
    }

    /// Open a WebSocket that must be accepted.
    pub async fn ws_ok(&self, path: &str) -> WsClient {
        match self.ws(path, None).await {
            Ok(client) => client,
            Err(status) => panic!("upgrade of {path} refused with {status}"),
        }
    }

    /// A lobby connection, after its snapshot.
    pub async fn lobby(&self) -> (WsClient, LobbyServerMsg) {
        let mut lobby = self.ws_ok("/ws/lobby").await;
        let snapshot: LobbyServerMsg = lobby.recv().await;
        assert!(
            matches!(snapshot, LobbyServerMsg::LobbySnapshot { .. }),
            "{snapshot:?}"
        );
        (lobby, snapshot)
    }

    /// Create a session `name` over `start..=end` through a fresh lobby.
    pub async fn create(&self, name: &str, start: u16, end: u16) -> Created {
        self.create_with(name, start, end, false).await
    }

    /// Create a session that collects viewer names when `collect_names`.
    pub async fn create_with(
        &self,
        name: &str,
        start: u16,
        end: u16,
        collect_names: bool,
    ) -> Created {
        let (mut lobby, _) = self.lobby().await;
        lobby
            .send(&LobbyClientMsg::CreateSession {
                name: name.to_string(),
                start,
                end,
                pin: None,
                collect_names,
            })
            .await;
        created(&lobby.recv().await)
    }

    /// Connect to session `id` as a counted viewer.
    pub async fn viewer(&self, id: &str) -> WsClient {
        let mut viewer = self.ws_ok(&format!("/ws/sessions/{id}")).await;
        let snapshot: SessionServerMsg = viewer.recv().await;
        assert!(
            matches!(
                snapshot,
                SessionServerMsg::Snapshot {
                    is_creator: false,
                    ..
                }
            ),
            "{snapshot:?}"
        );
        // Counted once its auth window passes; the broadcast reaches it too.
        // Viewers are added one at a time, so this meta is its own.
        viewer
            .recv_until(|msg: &SessionServerMsg| {
                matches!(msg, SessionServerMsg::SessionMeta { .. })
            })
            .await;
        viewer.barrier().await;
        viewer
    }

    /// Connect to a session as its creator (authenticated at once).
    pub async fn creator(&self, session: &Created) -> WsClient {
        let mut creator = self.ws_ok(&format!("/ws/sessions/{}", session.id)).await;
        let snapshot: SessionServerMsg = creator.recv().await;
        assert!(
            matches!(snapshot, SessionServerMsg::Snapshot { .. }),
            "{snapshot:?}"
        );
        creator
            .send(&SessionClientMsg::Auth {
                creator_token: session.token.clone(),
            })
            .await;
        let auth = creator
            .recv_until(|msg: &SessionServerMsg| matches!(msg, SessionServerMsg::AuthOk { .. }))
            .await;
        assert!(
            matches!(
                auth,
                SessionServerMsg::AuthOk {
                    is_creator: true,
                    ..
                }
            ),
            "{auth:?}"
        );
        creator
    }

    /// Connect as the creator and attach a (simulated) receiver.
    pub async fn creator_with_receiver(&self, session: &Created) -> WsClient {
        let mut creator = self.creator(session).await;
        creator.attach_receiver().await;
        creator
    }

    /// `GET uri` in process, with an optional bearer token.
    pub async fn get(&self, uri: &str, bearer: Option<&str>) -> (StatusCode, HeaderMap, String) {
        let mut request = Request::builder().uri(uri);
        if let Some(token) = bearer {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = request.body(Body::empty()).expect("request");
        self.request(request).await
    }

    /// Send any request in process.
    pub async fn request(&self, request: Request<Body>) -> (StatusCode, HeaderMap, String) {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("response");
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        (status, headers, String::from_utf8_lossy(&body).into_owned())
    }

    /// The statistics log file of session `id`.
    pub fn log_path(&self, id: &str) -> PathBuf {
        self.log_dir.join(format!("{id}.jsonl"))
    }

    /// The raw statistics log of session `id` (empty when missing).
    pub fn log_text(&self, id: &str) -> String {
        std::fs::read_to_string(self.log_path(id)).unwrap_or_default()
    }

    /// The parsed statistics log of session `id`.
    pub fn log_events(&self, id: &str) -> Vec<JsonlEvent> {
        parse_log(&self.log_text(id))
    }

    /// How many `.jsonl` files the log directory holds.
    pub fn log_files(&self) -> usize {
        std::fs::read_dir(&self.log_dir).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
                .count()
        })
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_dir_all(&self.log_dir);
    }
}

/// The session in a `session_created` reply.
pub fn created(message: &LobbyServerMsg) -> Created {
    match message {
        LobbyServerMsg::SessionCreated {
            session_id,
            creator_token,
            ..
        } => Created {
            id: session_id.clone(),
            token: creator_token.clone(),
        },
        other => panic!("expected session_created, got {other:?}"),
    }
}

/// The firmware the simulated receiver reports.
pub fn new_receiver_firmware() -> FirmwareInfo {
    FirmwareInfo {
        kind: FirmwareKind::New,
        mode: Some(FirmwareMode::Receiver),
        version: Some("draft-1".to_string()),
        station_id: None,
        station_color: None,
    }
}

/// Parse every line of a statistics log; any unparsable line fails.
pub fn parse_log(text: &str) -> Vec<JsonlEvent> {
    text.lines()
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("bad statistics line {line:?}: {error}"))
        })
        .collect()
}

/// `(ts_ms, elapsed_ms)` of any statistics event.
pub fn stamps(event: &JsonlEvent) -> (i64, u64) {
    match event {
        JsonlEvent::SessionStart {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::ReceiverStatus {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::StateChange {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::RangeChange {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::ViewerCount {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::SessionEnd {
            ts_ms, elapsed_ms, ..
        } => (*ts_ms, *elapsed_ms),
    }
}

/// `event` with its `ts_ms` and `elapsed_ms` zeroed, for comparing
/// event sequences without their timing.
pub fn unstamped(event: &JsonlEvent) -> JsonlEvent {
    let mut event = event.clone();
    match &mut event {
        JsonlEvent::SessionStart {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::ReceiverStatus {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::StateChange {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::RangeChange {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::ViewerCount {
            ts_ms, elapsed_ms, ..
        }
        | JsonlEvent::SessionEnd {
            ts_ms, elapsed_ms, ..
        } => {
            *ts_ms = 0;
            *elapsed_ms = 0;
        }
    }
    event
}

/// The documented file invariants: a `session_start` header with
/// `schema_version: 1` and `elapsed_ms: 0` for session `id`, then
/// non-decreasing `ts_ms` with `elapsed_ms` measured from the header.
pub fn assert_log_invariants(id: &str, events: &[JsonlEvent]) {
    let Some(JsonlEvent::SessionStart {
        schema_version,
        ts_ms: start,
        elapsed_ms: 0,
        session_id,
        ..
    }) = events.first()
    else {
        panic!("line 1 must be session_start with elapsed_ms 0: {events:?}");
    };
    assert_eq!(*schema_version, STATS_SCHEMA_VERSION);
    assert_eq!(session_id, id);
    let mut last = (*start, 0);
    for event in events {
        let (ts_ms, elapsed_ms) = stamps(event);
        assert!(ts_ms >= last.0, "ts_ms went backward at {event:?}");
        assert!(
            elapsed_ms >= last.1,
            "elapsed_ms went backward at {event:?}"
        );
        assert_eq!(
            i64::try_from(elapsed_ms).ok(),
            Some(ts_ms - start),
            "elapsed_ms must count from session_start: {event:?}"
        );
        last = (ts_ms, elapsed_ms);
    }
    let starts = events
        .iter()
        .filter(|event| matches!(event, JsonlEvent::SessionStart { .. }))
        .count();
    assert_eq!(starts, 1, "exactly one session_start: {events:?}");
}

/// Wait until `check` holds, polling, for at most [`STEP`].
pub async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + STEP;
    while !check().await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// One server frame, as seen by the test client.
pub enum Frame {
    /// A text message.
    Text(String),
    /// A close frame, with its status code when present. Already answered.
    Close(Option<u16>),
    /// The TCP connection ended.
    Eof,
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "Text({text})"),
            Self::Close(code) => write!(f, "Close({code:?})"),
            Self::Eof => f.write_str("Eof"),
        }
    }
}

/// A minimal RFC 6455 client.
pub struct WsClient {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl WsClient {
    /// Perform the opening handshake for `path`. A refused upgrade
    /// returns its HTTP status code.
    pub async fn connect(addr: SocketAddr, path: &str, origin: Option<&str>) -> Result<Self, u16> {
        let mut stream = TcpStream::connect(addr).await.expect("connect");
        let mut request = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {WS_KEY}\r\nSec-WebSocket-Version: 13\r\n"
        );
        if let Some(origin) = origin {
            request.push_str(&format!("Origin: {origin}\r\n"));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("send handshake");

        let mut client = Self {
            stream,
            buffer: Vec::new(),
        };
        let head_end = loop {
            if let Some(end) = client.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break end;
            }
            let more = timeout(STEP, client.read_more())
                .await
                .expect("handshake response in time");
            assert!(more, "connection closed during the handshake");
        };
        let head = String::from_utf8_lossy(&client.buffer[..head_end]).into_owned();
        client.buffer.drain(..head_end + 4);
        let status: u16 = head
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("bad status line: {head}"));
        if status != 101 {
            return Err(status);
        }
        let accept = head.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("sec-websocket-accept")
                .then(|| value.trim().to_string())
        });
        assert_eq!(accept.as_deref(), Some(WS_ACCEPT), "{head}");
        Ok(client)
    }

    /// Send one message as JSON text.
    pub async fn send<T: Serialize>(&mut self, message: &T) {
        let text = serde_json::to_string(message).expect("encode");
        self.send_text(&text).await;
    }

    /// Send one text message.
    pub async fn send_text(&mut self, text: &str) {
        self.send_frame(0x1, text.as_bytes())
            .await
            .expect("send text frame");
    }

    /// Send one text message the server may refuse by dropping the
    /// connection midway; a failed write is not an error here.
    pub async fn try_send_text(&mut self, text: &str) {
        let _ = self.send_frame(0x1, text.as_bytes()).await;
    }

    /// Send one binary message.
    pub async fn send_binary(&mut self, bytes: &[u8]) {
        self.send_frame(0x2, bytes)
            .await
            .expect("send binary frame");
    }

    /// Start the closing handshake.
    pub async fn send_close(&mut self) {
        self.send_frame(0x8, &1000_u16.to_be_bytes())
            .await
            .expect("send close frame");
    }

    async fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
        const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];
        let mut frame = vec![0x80 | opcode];
        let len = payload.len();
        if len < 126 {
            frame.push(0x80 | u8::try_from(len).expect("short length"));
        } else if let Ok(len) = u16::try_from(len) {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&len.to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&u64::try_from(len).expect("length").to_be_bytes());
        }
        frame.extend_from_slice(&MASK);
        frame.extend(
            payload
                .iter()
                .zip(MASK.iter().cycle())
                .map(|(byte, mask)| byte ^ mask),
        );
        self.stream.write_all(&frame).await
    }

    async fn read_more(&mut self) -> bool {
        let mut chunk = [0_u8; 8192];
        match self.stream.read(&mut chunk).await {
            Ok(0) | Err(_) => false,
            Ok(read) => {
                self.buffer.extend_from_slice(&chunk[..read]);
                true
            }
        }
    }

    async fn take(&mut self, len: usize) -> Option<Vec<u8>> {
        while self.buffer.len() < len {
            if !self.read_more().await {
                return None;
            }
        }
        Some(self.buffer.drain(..len).collect())
    }

    /// The next frame from the server. Pings are answered and skipped;
    /// a close frame is answered before it is returned.
    pub async fn next_frame(&mut self) -> Frame {
        loop {
            let Some(head) = self.take(2).await else {
                return Frame::Eof;
            };
            assert_eq!(head[0] & 0x80, 0x80, "unexpected fragmented frame");
            assert_eq!(head[1] & 0x80, 0, "server frames must not be masked");
            let len = match head[1] & 0x7f {
                126 => match self.take(2).await {
                    Some(bytes) => usize::from(u16::from_be_bytes([bytes[0], bytes[1]])),
                    None => return Frame::Eof,
                },
                127 => match self.take(8).await {
                    Some(bytes) => {
                        let mut wide = [0_u8; 8];
                        wide.copy_from_slice(&bytes);
                        usize::try_from(u64::from_be_bytes(wide)).expect("frame length")
                    }
                    None => return Frame::Eof,
                },
                short => usize::from(short),
            };
            let Some(payload) = self.take(len).await else {
                return Frame::Eof;
            };
            match head[0] & 0x0f {
                0x1 => return Frame::Text(String::from_utf8(payload).expect("utf-8 text")),
                0x8 => {
                    let code =
                        (payload.len() >= 2).then(|| u16::from_be_bytes([payload[0], payload[1]]));
                    let echo = payload.get(..2).unwrap_or_default().to_vec();
                    let _ = self.send_frame(0x8, &echo).await;
                    return Frame::Close(code);
                }
                0x9 => {
                    let _ = self.send_frame(0xA, &payload).await;
                }
                0xA => {}
                opcode => panic!("unexpected opcode {opcode:#x}"),
            }
        }
    }

    /// The next text message, raw.
    pub async fn recv_text(&mut self) -> String {
        match timeout(STEP, self.next_frame()).await {
            Ok(Frame::Text(text)) => text,
            Ok(other) => panic!("expected a text message, got {other:?}"),
            Err(_) => panic!("no message within {STEP:?}"),
        }
    }

    /// The next message, parsed.
    pub async fn recv<T: DeserializeOwned>(&mut self) -> T {
        let text = self.recv_text().await;
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("unexpected {text}: {error}"))
    }

    /// Skip messages until one satisfies `wanted`.
    pub async fn recv_until<T: DeserializeOwned + fmt::Debug>(
        &mut self,
        wanted: impl Fn(&T) -> bool,
    ) -> T {
        loop {
            let message: T = self.recv().await;
            if wanted(&message) {
                return message;
            }
        }
    }

    /// Every session message the server queued for this connection
    /// before it answered a `ping` sent now. A `ping` changes nothing on
    /// the server, so this is a side-effect-free way to wait until every
    /// earlier message has been handled.
    pub async fn barrier(&mut self) -> Vec<SessionServerMsg> {
        self.send(&SessionClientMsg::Ping).await;
        let mut before = Vec::new();
        loop {
            match self.recv().await {
                SessionServerMsg::Pong => return before,
                other => before.push(other),
            }
        }
    }

    /// Every lobby message queued before a `ping` sent now was answered.
    pub async fn lobby_barrier(&mut self) -> Vec<LobbyServerMsg> {
        self.send(&LobbyClientMsg::Ping).await;
        let mut before = Vec::new();
        loop {
            match self.recv().await {
                LobbyServerMsg::Pong => return before,
                other => before.push(other),
            }
        }
    }

    /// Read until the server closes the connection; returns the text
    /// messages received first and the close code, if any.
    pub async fn expect_close(&mut self) -> (Vec<String>, Option<u16>) {
        let mut texts = Vec::new();
        loop {
            match timeout(STEP, self.next_frame()).await {
                Ok(Frame::Text(text)) => texts.push(text),
                Ok(Frame::Close(code)) => return (texts, code),
                Ok(Frame::Eof) => return (texts, None),
                Err(_) => panic!("still open after {STEP:?}; received {texts:?}"),
            }
        }
    }

    /// Send a close frame and wait for the server's answer.
    pub async fn close(mut self) {
        self.send_close().await;
        let _ = self.expect_close().await;
    }

    /// Report the simulated receiver as connected, and wait for the
    /// server to confirm it.
    pub async fn attach_receiver(&mut self) {
        self.send(&SessionClientMsg::SerialStatus {
            connected: true,
            port_label: Some("micro:bit (test)".to_string()),
            firmware: Some(new_receiver_firmware()),
        })
        .await;
        self.recv_until(|msg: &SessionServerMsg| {
            matches!(
                msg,
                SessionServerMsg::ReceiverStatus {
                    connected: true,
                    ..
                }
            )
        })
        .await;
    }
}
