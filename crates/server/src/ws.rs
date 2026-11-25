// SPDX-License-Identifier: AGPL-3.0-or-later
//! The WebSocket endpoints: `/ws/lobby` and `/ws/sessions/{id}`.
//!
//! Before any upgrade, the HTTP `Origin` header is checked against
//! `PUBLIC_ORIGIN` (when configured) to block cross-site WebSocket
//! hijacking; a missing or foreign origin gets `403 Forbidden` and never a
//! socket. Upgraded sockets accept JSON text messages of at most
//! [`MAX_MESSAGE_BYTES`].
//!
//! Each connection runs in one task that multiplexes four things: frames
//! from the peer, the connection's bounded outbound queue (see
//! the `client` module), a server ping every [`PING_INTERVAL`] (a peer silent
//! for [`IDLE_TIMEOUT`] is dropped, so a sleeping laptop cannot keep a
//! creator "connected" forever), and an optional deadline. Writes time out
//! after [`SEND_TIMEOUT`], so a stalled peer cannot pin the task either.
//!
//! Each connection is also rate limited where a client could otherwise
//! flood others: `set_table` (every accepted click becomes a serial command
//! to the creator, a radio packet in the classroom, and a statistics line)
//! and `create_session` (which checks the creation PIN). Excess requests
//! get `Error { code: "rate_limited" }` and are not processed. Station
//! reports are never limited, so the board cannot miss a change.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant, MissedTickBehavior};
use trafficlights_proto::{LobbyClientMsg, LobbyServerMsg, SessionClientMsg, SessionServerMsg};

use crate::client::Client;
use crate::session;
use crate::state::{AppState, ConnectionGuard, CreateError};

/// The largest WebSocket message (and frame) accepted from a client.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
/// How often the server pings an idle peer.
pub const PING_INTERVAL: Duration = Duration::from_secs(20);
/// How long a peer may stay completely silent (not even answering pings)
/// before its connection is dropped.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// The longest one outbound frame may take to write.
pub const SEND_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to wait for the peer's half of a close handshake.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);
/// `set_table` per connection: a burst of this many clicks...
pub const SET_TABLE_BURST: u32 = 10;
/// ...then this many per second.
pub const SET_TABLE_PER_SECOND: u32 = 5;
/// `create_session` per lobby connection: a burst of this many requests...
pub const CREATE_SESSION_BURST: u32 = 10;
/// ...then this many per second.
pub const CREATE_SESSION_PER_SECOND: u32 = 1;
/// Failed PIN attempts on one lobby connection before it is closed.
pub const MAX_PIN_FAILURES_PER_CONNECTION: u32 = 10;

/// Whether `id` can name a session: 1 to 64 ASCII letters, digits, or
/// hyphens. Session IDs are UUIDs, so anything else (including any path
/// separator or dot) is rejected before it reaches a lookup.
pub fn valid_session_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// `GET /ws/lobby`: the live session list, and session creation.
pub(crate) async fn lobby(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if let Some(rejected) = reject_origin(&state, &headers) {
        return rejected;
    }
    match upgrade {
        Ok(upgrade) => {
            let Some(guard) = state.try_connection_guard() else {
                return too_many_connections();
            };
            limit(upgrade).on_upgrade(move |socket| lobby_socket(state, socket, guard))
        }
        Err(rejection) => rejection.into_response(),
    }
}

fn too_many_connections() -> Response {
    crate::api_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "too_many_connections",
        "Too many connections; please try again later.",
    )
}

/// `GET /ws/sessions/{id}`: one session, for viewers and its creator.
pub(crate) async fn session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if let Some(rejected) = reject_origin(&state, &headers) {
        return rejected;
    }
    match upgrade {
        Ok(upgrade) => {
            let Some(guard) = state.try_connection_guard() else {
                return too_many_connections();
            };
            limit(upgrade).on_upgrade(move |socket| session_socket(state, id, socket, guard))
        }
        Err(rejection) => rejection.into_response(),
    }
}

/// The first check of every upgrade request, before anything else is
/// looked at: `403 Forbidden` unless the `Origin` is allowed.
fn reject_origin(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    if state.config().origin_allowed(origin) {
        return None;
    }
    tracing::warn!(
        origin = origin.unwrap_or("<none>"),
        "WebSocket origin rejected"
    );
    Some(crate::api_error(
        StatusCode::FORBIDDEN,
        "forbidden_origin",
        "WebSocket connections are only accepted from this site.",
    ))
}

/// Cap what a client may send in one message or frame.
fn limit(upgrade: WebSocketUpgrade) -> WebSocketUpgrade {
    upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
}

async fn lobby_socket(state: Arc<AppState>, socket: WebSocket, _guard: ConnectionGuard) {
    let (client, rx) = state.new_client();
    let mut connection = Connection::new(socket, rx, client.clone());
    state.lobby_join(&client);
    if state.is_shutting_down() {
        client.close("server shutting down");
    }
    let mut creates = RateLimit::new(CREATE_SESSION_BURST, CREATE_SESSION_PER_SECOND);
    let mut pin_failures: u32 = 0;
    loop {
        match connection.next_event(None).await {
            Event::Text(text) => match serde_json::from_str::<LobbyClientMsg>(&text) {
                Ok(LobbyClientMsg::Ping) => client.send(&LobbyServerMsg::Pong),
                Ok(LobbyClientMsg::CreateSession {
                    name,
                    start,
                    end,
                    pin,
                }) => {
                    if !creates.allow(Instant::now()) {
                        client.send(&lobby_error("rate_limited", RATE_LIMITED_MESSAGE));
                        continue;
                    }
                    let created = state
                        .create_session(&client, &name, start, end, pin.as_deref())
                        .await;
                    if let Err(error) = created {
                        client.send(&LobbyServerMsg::Error {
                            code: error.code().to_string(),
                            message: error.message().to_string(),
                        });
                        if error == CreateError::InvalidPin {
                            pin_failures += 1;
                            if pin_failures >= MAX_PIN_FAILURES_PER_CONNECTION {
                                client.close("too many failed PIN attempts");
                            }
                        }
                    }
                }
                Err(_) => client.send(&lobby_error("bad_request", BAD_REQUEST_MESSAGE)),
            },
            Event::Binary => client.send(&lobby_error("bad_request", BAD_REQUEST_MESSAGE)),
            Event::Deadline => {}
            Event::Closed => break,
        }
    }
    state.lobby_leave(client.id());
}

async fn session_socket(
    state: Arc<AppState>,
    id: String,
    socket: WebSocket,
    _guard: ConnectionGuard,
) {
    let (client, rx) = state.new_client();
    let mut connection = Connection::new(socket, rx, client.clone());
    let joined = if valid_session_id(&id) {
        session::join(&state, &id, &client).await
    } else {
        session::reply_missing(&client);
        None
    };
    let Some(cell) = joined else {
        // Deliver the queued answer and close frame, then stop.
        while !matches!(connection.next_event(None).await, Event::Closed) {}
        return;
    };

    let mut settle_at = Some(Instant::now() + state.config().auth_window);
    let mut clicks = RateLimit::new(SET_TABLE_BURST, SET_TABLE_PER_SECOND);
    loop {
        match connection.next_event(settle_at).await {
            Event::Text(text) => match serde_json::from_str::<SessionClientMsg>(&text) {
                Ok(SessionClientMsg::SetTable { .. }) if !clicks.allow(Instant::now()) => {
                    client.send(&session_error("rate_limited", RATE_LIMITED_MESSAGE));
                }
                Ok(message) => session::handle(&state, &cell, &client, message).await,
                Err(_) => client.send(&session_error("bad_request", BAD_REQUEST_MESSAGE)),
            },
            Event::Binary => client.send(&session_error("bad_request", BAD_REQUEST_MESSAGE)),
            Event::Deadline => {
                settle_at = None;
                session::settle(&state, &cell, &client).await;
            }
            Event::Closed => break,
        }
    }
    session::leave(&state, &cell, &client).await;
}

const BAD_REQUEST_MESSAGE: &str = "Messages must be JSON text matching the protocol.";
const RATE_LIMITED_MESSAGE: &str = "Too many requests; please slow down.";

fn lobby_error(code: &str, message: &str) -> LobbyServerMsg {
    LobbyServerMsg::Error {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn session_error(code: &str, message: &str) -> SessionServerMsg {
    SessionServerMsg::Error {
        code: code.to_string(),
        message: message.to_string(),
    }
}

/// A token bucket: up to `burst` actions at once, refilled continuously at
/// `per_second` actions per second.
#[derive(Debug, Clone, Copy)]
struct RateLimit {
    burst: f64,
    per_second: f64,
    tokens: f64,
    updated: Instant,
}

impl RateLimit {
    fn new(burst: u32, per_second: u32) -> Self {
        Self {
            burst: f64::from(burst),
            per_second: f64::from(per_second),
            tokens: f64::from(burst),
            updated: Instant::now(),
        }
    }

    /// Whether one more action is allowed at `now`; takes a token if so.
    fn allow(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.updated).as_secs_f64();
        self.updated = self.updated.max(now);
        self.tokens = (self.tokens + elapsed * self.per_second).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// What happened next on a connection.
enum Event {
    /// The peer sent a text message.
    Text(Utf8Bytes),
    /// The peer sent a binary message, which this protocol never uses.
    Binary,
    /// The deadline passed to [`Connection::next_event`] was reached.
    Deadline,
    /// The connection is over: the peer left or went silent, the server
    /// closed it, or it was kicked for being too slow.
    Closed,
}

/// One upgraded socket together with its outbound queue.
struct Connection {
    socket: WebSocket,
    outbound: mpsc::Receiver<Message>,
    client: Client,
    ping: tokio::time::Interval,
    last_heard: Instant,
}

impl Connection {
    fn new(socket: WebSocket, outbound: mpsc::Receiver<Message>, client: Client) -> Self {
        let mut ping = tokio::time::interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
        ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Self {
            socket,
            outbound,
            client,
            ping,
            last_heard: Instant::now(),
        }
    }

    /// Serve the connection until the peer sends a message, `deadline`
    /// passes, or the connection ends. Outbound messages and pings are
    /// written in between. Every future raced here is cancel-safe, and
    /// writes happen only after the race is decided, so nothing is lost.
    async fn next_event(&mut self, deadline: Option<Instant>) -> Event {
        loop {
            tokio::select! {
                incoming = self.socket.recv() => {
                    self.last_heard = Instant::now();
                    match incoming {
                        Some(Ok(Message::Text(text))) => return Event::Text(text),
                        Some(Ok(Message::Binary(_))) => return Event::Binary,
                        Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                        Some(Ok(Message::Close(_))) => {
                            // Reading once more flushes the automatic close
                            // reply and observes the end of the stream.
                            let _ = timeout(CLOSE_TIMEOUT, self.socket.recv()).await;
                            return Event::Closed;
                        }
                        Some(Err(_)) | None => return Event::Closed,
                    }
                }
                outbound = self.outbound.recv() => {
                    let Some(message) = outbound else {
                        return Event::Closed;
                    };
                    let closing = matches!(message, Message::Close(_));
                    if !self.write(message).await {
                        return Event::Closed;
                    }
                    if closing {
                        self.finish_close().await;
                        return Event::Closed;
                    }
                }
                () = self.client.kicked() => return Event::Closed,
                _ = self.ping.tick() => {
                    if self.last_heard.elapsed() > IDLE_TIMEOUT
                        || !self.write(Message::Ping(Bytes::new())).await
                    {
                        return Event::Closed;
                    }
                }
                () = wait_until(deadline) => return Event::Deadline,
            }
        }
    }

    /// Write one frame; `false` when the socket failed or stalled.
    async fn write(&mut self, message: Message) -> bool {
        matches!(
            timeout(SEND_TIMEOUT, self.socket.send(message)).await,
            Ok(Ok(()))
        )
    }

    /// After sending our close frame, wait briefly for the peer's.
    async fn finish_close(&mut self) {
        let _ = timeout(CLOSE_TIMEOUT, async {
            while let Some(Ok(message)) = self.socket.recv().await {
                if matches!(message, Message::Close(_)) {
                    break;
                }
            }
        })
        .await;
    }
}

async fn wait_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::{valid_session_id, RateLimit};

    #[test]
    fn rate_limits_allow_a_burst_then_refill_over_time() {
        let mut limit = RateLimit::new(3, 2);
        let start = Instant::now();
        assert!((0..3).all(|_| limit.allow(start)));
        assert!(!limit.allow(start));
        // Half a second refills one token at two per second.
        let later = start + Duration::from_millis(500);
        assert!(limit.allow(later));
        assert!(!limit.allow(later));
        // A long pause refills to the burst, never beyond it.
        let much_later = later + Duration::from_secs(60);
        assert!((0..3).all(|_| limit.allow(much_later)));
        assert!(!limit.allow(much_later));
        // A clock reading from the past never adds tokens.
        assert!(!limit.allow(start));
    }

    #[test]
    fn session_ids_are_short_hyphenated_alphanumerics() {
        assert!(valid_session_id("0195f1a2-3b4c-7d8e-9f0a-1b2c3d4e5f60"));
        assert!(valid_session_id("a"));
        assert!(valid_session_id(&"a".repeat(64)));
        assert!(!valid_session_id(""));
        assert!(!valid_session_id(&"a".repeat(65)));
        for id in [
            "..", "../etc", "a/b", "a\\b", "a.jsonl", "a b", "a%2F", "\u{e9}",
        ] {
            assert!(!valid_session_id(id), "{id:?}");
        }
    }
}
