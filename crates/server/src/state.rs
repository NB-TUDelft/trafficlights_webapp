// SPDX-License-Identifier: AGPL-3.0-or-later
//! The shared server state: the session registry, lobby subscribers,
//! retained logs, session creation and finalization, the disconnect-grace
//! watchdog, and graceful shutdown.
//!
//! Locking: the registry sits behind a `std::sync::Mutex` that is never held
//! across an `.await`; each session sits behind its own `tokio::sync::Mutex`
//! held for the whole handling of one message, so every message is applied
//! atomically. The only nesting allowed is session lock, then registry lock
//! (the lobby reads cached summaries, never another session's lock), which
//! rules out lock-order deadlocks.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;
use tokio::sync::Notify;
use tokio::task::AbortHandle;
use trafficlights_proto::{
    normalize_session_name, EndReason, JsonlEvent, LobbyServerMsg, ServerBuildInfo,
    SessionServerMsg, SessionSummary, TableRange, PROTO_VERSION,
};

use crate::client::{encode, Client, ConnId};
use crate::config::ServerConfig;
use crate::log::SessionLog;
use crate::session::Session;

/// The most ended-session logs kept downloadable at once; beyond this the
/// oldest is deleted early, so session churn cannot grow memory or disk.
pub const MAX_RETAINED_LOGS: usize = 256;

/// How long [`AppState::shutdown`] waits for clients to receive their
/// final messages.
const SHUTDOWN_DRAIN: Duration = Duration::from_secs(2);

/// Failed creation-PIN attempts allowed server-wide per window before
/// further attempts are refused without checking the PIN.
pub const MAX_PIN_FAILURES: usize = 20;
/// The sliding window for [`MAX_PIN_FAILURES`].
pub const PIN_FAILURE_WINDOW: Duration = Duration::from_secs(60);
/// The most tables one displayed range may hold (`end - start + 1`).
pub const MAX_RANGE_LEN: usize = 512;

/// Server-global sliding-window limiter for failed creation-PIN attempts.
#[derive(Debug, Default)]
pub struct PinFailureLimiter {
    failures: VecDeque<Instant>,
}

impl PinFailureLimiter {
    fn prune(&mut self, now: Instant) {
        while let Some(front) = self.failures.front() {
            if now.saturating_duration_since(*front) >= PIN_FAILURE_WINDOW {
                self.failures.pop_front();
            } else {
                break;
            }
        }
    }

    /// Whether the budget is exhausted at `now`.
    pub fn is_limited(&mut self, now: Instant) -> bool {
        self.prune(now);
        self.failures.len() >= MAX_PIN_FAILURES
    }

    /// Record one failed attempt at `now`.
    pub fn record_failure(&mut self, now: Instant) {
        self.prune(now);
        self.failures.push_back(now);
    }
}

/// UTC epoch milliseconds now.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Constant-time secret comparison (creator tokens and the creation PIN).
pub fn secrets_match(expected: &str, provided: &str) -> bool {
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

/// A session's creator capability token.
///
/// Its `Debug` output is redacted, so no `{:?}` of a session, a registry
/// entry, or this value can ever write the token to the server log; the
/// only ways to read it are [`CreatorToken::matches`] (constant time) and
/// [`CreatorToken::reveal`], used once to hand it to its creator.
#[derive(Clone, PartialEq, Eq)]
pub struct CreatorToken(String);

impl CreatorToken {
    /// A fresh 32-byte token from the operating system CSPRNG, as 64
    /// lowercase hex characters.
    fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let mut token = String::with_capacity(64);
        for byte in bytes {
            for nibble in [byte >> 4, byte & 0x0f] {
                token.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
            }
        }
        Self(token)
    }

    /// Whether `provided` is this token, compared in constant time.
    pub fn matches(&self, provided: &str) -> bool {
        secrets_match(&self.0, provided)
    }

    /// The token text, for the one `session_created` reply to its creator.
    fn reveal(&self) -> String {
        self.0.clone()
    }
}

impl std::fmt::Debug for CreatorToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CreatorToken(<redacted>)")
    }
}

/// One active session: identity and secrets outside the lock, mutable
/// state inside it.
#[derive(Debug)]
pub struct SessionCell {
    pub(crate) id: String,
    pub(crate) token: CreatorToken,
    pub(crate) log_path: PathBuf,
    pub(crate) session: tokio::sync::Mutex<Session>,
}

#[derive(Debug)]
struct RetainedLog {
    token: CreatorToken,
    path: PathBuf,
    reason: EndReason,
    ended_at: Instant,
    /// The task deleting this log when its retention window ends; aborted
    /// when the log is evicted early, so churn leaves no idle tasks behind.
    cleanup: AbortHandle,
}

#[derive(Debug, Default)]
struct Registry {
    sessions: HashMap<String, Arc<SessionCell>>,
    summaries: HashMap<String, SessionSummary>,
    reserved: usize,
    retained: HashMap<String, RetainedLog>,
    lobby: HashMap<ConnId, Client>,
}

impl Registry {
    /// Every live session in lobby order: oldest first.
    fn sorted_summaries(&self) -> Vec<SessionSummary> {
        let mut sessions: Vec<SessionSummary> = self.summaries.values().cloned().collect();
        sessions.sort_by(|a, b| {
            a.created_at_ms
                .cmp(&b.created_at_ms)
                .then_with(|| a.session_id.cmp(&b.session_id))
        });
        sessions
    }

    fn broadcast_lobby(&self) {
        let update = LobbyServerMsg::LobbyUpdate {
            sessions: self.sorted_summaries(),
        };
        if let Some(text) = encode(&update) {
            for client in self.lobby.values() {
                client.send_text(&text);
            }
        }
    }
}

/// What a session ID currently refers to.
#[derive(Debug)]
pub enum Lookup {
    /// A live session.
    Active(Arc<SessionCell>),
    /// A session that ended recently; its log is still downloadable.
    Ended(EndReason),
    /// Unknown or expired.
    Missing,
}

/// Why a `create_session` request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateError {
    /// The name failed `normalize_session_name`.
    InvalidName,
    /// The range failed `TableRange::new`.
    InvalidRange,
    /// The creation PIN is missing or wrong.
    InvalidPin,
    /// `MAX_SESSIONS` sessions are already active.
    MaxSessionsReached,
    /// The server is shutting down.
    ShuttingDown,
    /// The statistics log could not be created.
    Internal,
    /// Too many failed PIN attempts server-wide in the current window.
    RateLimited,
}

impl CreateError {
    /// The stable error code sent to the client.
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidName => "invalid_name",
            Self::InvalidRange => "invalid_range",
            Self::InvalidPin => "invalid_pin",
            Self::MaxSessionsReached => "max_sessions_reached",
            Self::ShuttingDown => "shutting_down",
            Self::Internal => "internal_error",
            Self::RateLimited => "rate_limited",
        }
    }

    /// The human-readable explanation sent to the client.
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidName => {
                "Session name must be 1 to 80 characters without control characters."
            }
            Self::InvalidRange => {
                "Table range must satisfy 1 <= start <= end <= 9999 and contain at most 512 tables."
            }
            Self::InvalidPin => "Wrong or missing session creation PIN.",
            Self::MaxSessionsReached => "The maximum number of active sessions is reached.",
            Self::ShuttingDown => "The server is shutting down.",
            Self::Internal => "The session could not be created.",
            Self::RateLimited => "Too many failed PIN attempts; please wait and try again.",
        }
    }
}

struct Shared {
    config: ServerConfig,
    build_info: ServerBuildInfo,
    registry: Mutex<Registry>,
    next_conn: AtomicU64,
    shutting_down: AtomicBool,
    connections: AtomicUsize,
    idle: Notify,
    pin_failures: Mutex<PinFailureLimiter>,
}

/// The shared application state behind every route.
#[derive(Clone)]
pub struct AppState {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.shared.config)
            .finish_non_exhaustive()
    }
}

/// Counts a live WebSocket connection for [`AppState::shutdown`].
pub struct ConnectionGuard {
    state: AppState,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if self.state.shared.connections.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.state.shared.idle.notify_waiters();
        }
    }
}

/// Holds one `MAX_SESSIONS` slot while a session is being created.
struct Reservation {
    state: AppState,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut registry = self.state.registry();
        registry.reserved = registry.reserved.saturating_sub(1);
    }
}

impl AppState {
    /// Fresh state for `config`; touches no disk until a session starts.
    pub fn new(config: ServerConfig) -> Self {
        let build_info = ServerBuildInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: crate::build_commit().map(str::to_string),
            license: crate::LICENSE.to_string(),
            source_url: crate::SOURCE_URL.to_string(),
            proto_version: PROTO_VERSION,
            pin_required: config.create_session_pin.is_some(),
        };
        Self {
            shared: Arc::new(Shared {
                config,
                build_info,
                registry: Mutex::new(Registry::default()),
                next_conn: AtomicU64::new(1),
                shutting_down: AtomicBool::new(false),
                connections: AtomicUsize::new(0),
                idle: Notify::new(),
                pin_failures: Mutex::new(PinFailureLimiter::default()),
            }),
        }
    }

    /// The configuration in effect.
    pub fn config(&self) -> &ServerConfig {
        &self.shared.config
    }

    /// Build, license, and protocol information for clients.
    pub fn build_info(&self) -> &ServerBuildInfo {
        &self.shared.build_info
    }

    /// The number of active sessions.
    pub fn active_sessions(&self) -> usize {
        self.registry().sessions.len()
    }

    /// Whether [`AppState::shutdown`] has started.
    pub fn is_shutting_down(&self) -> bool {
        self.shared.shutting_down.load(Ordering::Acquire)
    }

    fn registry(&self) -> MutexGuard<'_, Registry> {
        self.shared
            .registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// A new client handle with a server-unique ID, plus its queue.
    pub(crate) fn new_client(
        &self,
    ) -> (
        Client,
        tokio::sync::mpsc::Receiver<axum::extract::ws::Message>,
    ) {
        Client::new(self.shared.next_conn.fetch_add(1, Ordering::Relaxed))
    }

    /// Atomically reserve one connection slot, or `None` when
    /// `MAX_CONNECTIONS` connections are already open.
    pub(crate) fn try_connection_guard(&self) -> Option<ConnectionGuard> {
        let max = self.shared.config.max_connections;
        let mut current = self.shared.connections.load(Ordering::Acquire);
        loop {
            if current >= max {
                return None;
            }
            match self.shared.connections.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(ConnectionGuard {
                        state: self.clone(),
                    });
                }
                Err(actual) => current = actual,
            }
        }
    }

    fn pin_limited(&self) -> bool {
        self.shared
            .pin_failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_limited(Instant::now())
    }

    fn record_pin_failure(&self) {
        self.shared
            .pin_failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .record_failure(Instant::now());
    }

    /// Subscribe a lobby client, sending it the snapshot first.
    pub(crate) fn lobby_join(&self, client: &Client) {
        let mut registry = self.registry();
        client.send(&LobbyServerMsg::LobbySnapshot {
            sessions: registry.sorted_summaries(),
            server_info: self.shared.build_info.clone(),
        });
        registry.lobby.insert(client.id(), client.clone());
    }

    /// Unsubscribe a lobby client.
    pub(crate) fn lobby_leave(&self, id: ConnId) {
        self.registry().lobby.remove(&id);
    }

    /// What `id` refers to right now.
    pub(crate) fn lookup(&self, id: &str) -> Lookup {
        let registry = self.registry();
        if let Some(cell) = registry.sessions.get(id) {
            return Lookup::Active(Arc::clone(cell));
        }
        match registry.retained.get(id) {
            Some(retained) => Lookup::Ended(retained.reason),
            None => Lookup::Missing,
        }
    }

    /// The creator token and log path of an active or retained session.
    pub(crate) fn log_access(&self, id: &str) -> Option<(CreatorToken, PathBuf)> {
        let registry = self.registry();
        if let Some(cell) = registry.sessions.get(id) {
            return Some((cell.token.clone(), cell.log_path.clone()));
        }
        registry
            .retained
            .get(id)
            .map(|retained| (retained.token.clone(), retained.path.clone()))
    }

    /// Validate and create a session for `requester`, replying with
    /// `session_created` to it alone and then updating the whole lobby.
    pub(crate) async fn create_session(
        &self,
        requester: &Client,
        name: &str,
        start: u16,
        end: u16,
        pin: Option<&str>,
    ) -> Result<(), CreateError> {
        // Check the PIN first, so callers without it get no validation
        // oracle for names or ranges.
        if let Some(expected) = &self.shared.config.create_session_pin {
            if self.pin_limited() {
                return Err(CreateError::RateLimited);
            }
            if !secrets_match(expected, pin.unwrap_or_default()) {
                self.record_pin_failure();
                return Err(CreateError::InvalidPin);
            }
        }
        let name = normalize_session_name(name).map_err(|_| CreateError::InvalidName)?;
        let range = TableRange::new(start, end).map_err(|_| CreateError::InvalidRange)?;
        if range.len() > MAX_RANGE_LEN {
            return Err(CreateError::InvalidRange);
        }
        let _reservation = self.reserve_slot()?;

        let session_id = uuid::Uuid::now_v7().to_string();
        let creator_token = CreatorToken::generate();
        let now = now_ms();
        let log = SessionLog::create(&self.shared.config.log_dir, &session_id, &name, range, now)
            .await
            .map_err(|error| {
                tracing::error!(%error, "cannot create the session log");
                CreateError::Internal
            })?;
        let cell = Arc::new(SessionCell {
            id: session_id.clone(),
            token: creator_token.clone(),
            log_path: log.path().to_path_buf(),
            session: tokio::sync::Mutex::new(Session::new(
                session_id.clone(),
                name,
                range,
                now,
                log,
            )),
        });
        let summary = {
            let session = cell.session.lock().await;
            session.summary()
        };

        // Insert into the registry before arming the grace watchdog, so a
        // watchdog that fires at once can never finalize a session the
        // registry does not know yet.
        {
            let mut registry = self.registry();
            if self.shared.shutting_down.load(Ordering::Acquire) {
                drop(registry);
                let path = cell.log_path.clone();
                tokio::spawn(async move {
                    let _ = tokio::fs::remove_file(path).await;
                });
                return Err(CreateError::ShuttingDown);
            }
            registry
                .sessions
                .insert(session_id.clone(), Arc::clone(&cell));
            registry
                .summaries
                .insert(session_id.clone(), summary.clone());
        }
        {
            let mut session = cell.session.lock().await;
            if session.end_reason().is_none() {
                // The receiver starts disconnected until the creator attaches it.
                self.arm_grace(&cell, &mut session);
            }
        }
        tracing::info!(session_id = %session_id, "session created");
        requester.send(&LobbyServerMsg::SessionCreated {
            session_id,
            creator_token: creator_token.reveal(),
            summary,
        });
        self.registry().broadcast_lobby();
        Ok(())
    }

    fn reserve_slot(&self) -> Result<Reservation, CreateError> {
        let mut registry = self.registry();
        if self.shared.shutting_down.load(Ordering::Acquire) {
            return Err(CreateError::ShuttingDown);
        }
        if registry.sessions.len() + registry.reserved >= self.shared.config.max_sessions {
            return Err(CreateError::MaxSessionsReached);
        }
        registry.reserved += 1;
        Ok(Reservation {
            state: self.clone(),
        })
    }

    /// Refresh a live session's lobby summary and push it to the lobby,
    /// unless nothing the lobby shows has changed.
    pub(crate) fn publish(&self, session: &Session) {
        let summary = session.summary();
        let mut registry = self.registry();
        if !registry.sessions.contains_key(&summary.session_id)
            || registry.summaries.get(&summary.session_id) == Some(&summary)
        {
            return;
        }
        registry
            .summaries
            .insert(summary.session_id.clone(), summary);
        registry.broadcast_lobby();
    }

    /// Start (or restart) the disconnect-grace watchdog. When it expires
    /// without being canceled, the session ends with `grace_timeout`.
    pub(crate) fn arm_grace(&self, cell: &Arc<SessionCell>, session: &mut Session) {
        let generation = session.next_grace_generation();
        let state = self.clone();
        let cell = Arc::clone(cell);
        let grace = self.shared.config.disconnect_grace;
        let task = tokio::spawn(async move {
            tokio::time::sleep(grace).await;
            let mut session = cell.session.lock().await;
            if session.grace_generation() == generation {
                // Forget (without aborting) this very task's handle, so
                // finalizing cannot cancel itself halfway through.
                session.forget_grace_task();
                state
                    .finalize(&cell, &mut session, EndReason::GraceTimeout)
                    .await;
            }
        });
        session.set_grace_task(task.abort_handle());
    }

    /// Cancel the disconnect-grace watchdog, if armed.
    pub(crate) fn cancel_grace(&self, session: &mut Session) {
        session.next_grace_generation();
    }

    /// End a session: append `session_end`, tell every client, keep the
    /// log downloadable for `LOG_RETENTION_SECONDS`, then delete it.
    pub(crate) async fn finalize(
        &self,
        cell: &SessionCell,
        session: &mut Session,
        reason: EndReason,
    ) {
        if session.end_reason().is_some() {
            return;
        }
        session.set_ended(reason);
        self.cancel_grace(session);
        session
            .log
            .record(now_ms(), |ts_ms, elapsed_ms| JsonlEvent::SessionEnd {
                ts_ms,
                elapsed_ms,
                reason,
            })
            .await;
        session.log.close().await;

        if let Some(text) = encode(&SessionServerMsg::SessionEnded {
            reason,
            log_available: self.log_available(reason),
        }) {
            for client in session.take_clients() {
                client.send_text(&text);
                client.close("session ended");
            }
        }

        let evicted = {
            let mut registry = self.registry();
            registry.sessions.remove(&cell.id);
            registry.summaries.remove(&cell.id);
            let evicted = if registry.retained.len() >= MAX_RETAINED_LOGS {
                let oldest = registry
                    .retained
                    .iter()
                    .min_by_key(|(_, retained)| retained.ended_at)
                    .map(|(id, _)| id.clone());
                oldest.and_then(|id| registry.retained.remove(&id))
            } else {
                None
            };
            // Spawned with the registry locked, so the cleanup cannot look
            // for its entry before it is inserted.
            let cleanup = self.spawn_log_cleanup(cell.id.clone());
            registry.retained.insert(
                cell.id.clone(),
                RetainedLog {
                    token: cell.token.clone(),
                    path: cell.log_path.clone(),
                    reason,
                    ended_at: Instant::now(),
                    cleanup,
                },
            );
            registry.broadcast_lobby();
            evicted
        };
        if let Some(evicted) = evicted {
            evicted.cleanup.abort();
            let _ = tokio::fs::remove_file(&evicted.path).await;
        }
        tracing::info!(session_id = %cell.id, ?reason, "session ended");
    }

    /// Delete the log of ended session `id` once `LOG_RETENTION_SECONDS`
    /// have passed.
    fn spawn_log_cleanup(&self, id: String) -> AbortHandle {
        let state = self.clone();
        let retention = self.shared.config.log_retention;
        tokio::spawn(async move {
            tokio::time::sleep(retention).await;
            let removed = state.registry().retained.remove(&id);
            if let Some(retained) = removed {
                if let Err(error) = tokio::fs::remove_file(&retained.path).await {
                    tracing::warn!(session_id = %id, %error, "cannot delete an expired session log");
                }
            }
        })
        .abort_handle()
    }

    /// Whether a session that ended for `reason` keeps its log
    /// downloadable for a while: not when `LOG_RETENTION_SECONDS` is `0`,
    /// and not after a server shutdown, which closes the download route too.
    /// A shutdown leaves those logs in `LOG_DIR` (ephemeral by default).
    pub(crate) fn log_available(&self, reason: EndReason) -> bool {
        !self.shared.config.log_retention.is_zero() && reason != EndReason::Shutdown
    }

    /// Graceful shutdown: refuse new sessions, end every active session
    /// with `shutdown` (flushing its log and notifying its clients), close
    /// the lobby, and wait briefly for clients to receive it all.
    ///
    /// Idempotent: only the first call does anything; later ones return at
    /// once.
    pub async fn shutdown(&self) {
        let cells: Vec<Arc<SessionCell>> = {
            let registry = self.registry();
            if self.shared.shutting_down.swap(true, Ordering::AcqRel) {
                return;
            }
            registry.sessions.values().cloned().collect()
        };
        for cell in cells {
            let mut session = cell.session.lock().await;
            self.finalize(&cell, &mut session, EndReason::Shutdown)
                .await;
        }
        for client in self.registry().lobby.values() {
            client.close("server shutting down");
        }
        if tokio::time::timeout(SHUTDOWN_DRAIN, self.wait_idle())
            .await
            .is_err()
        {
            tracing::warn!("some WebSocket clients did not close before shutdown");
        }
    }

    async fn wait_idle(&self) {
        loop {
            let mut notified = pin!(self.shared.idle.notified());
            notified.as_mut().enable();
            if self.shared.connections.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        secrets_match, AppState, CreateError, CreatorToken, PinFailureLimiter, MAX_PIN_FAILURES,
        PIN_FAILURE_WINDOW,
    };
    use crate::config::ServerConfig;

    #[test]
    fn the_pin_limiter_exhausts_and_recovers_after_the_window() {
        let mut limiter = PinFailureLimiter::default();
        let start = Instant::now();
        for _ in 0..MAX_PIN_FAILURES {
            assert!(!limiter.is_limited(start));
            limiter.record_failure(start);
        }
        assert!(limiter.is_limited(start));
        assert!(limiter.is_limited(start + PIN_FAILURE_WINDOW - Duration::from_secs(1)));
        assert!(!limiter.is_limited(start + PIN_FAILURE_WINDOW + Duration::from_secs(1)));
    }

    #[test]
    fn connection_slots_are_capped_and_released_on_drop() {
        let state = AppState::new(ServerConfig {
            max_connections: 2,
            ..ServerConfig::default()
        });
        let first = state.try_connection_guard();
        let second = state.try_connection_guard();
        assert!(first.is_some() && second.is_some());
        assert!(state.try_connection_guard().is_none());
        drop(first);
        assert!(state.try_connection_guard().is_some());
    }

    #[test]
    fn rate_limited_is_a_stable_create_error() {
        assert_eq!(CreateError::RateLimited.code(), "rate_limited");
        assert!(CreateError::InvalidRange.message().contains("512"));
    }

    #[test]
    fn tokens_are_64_lowercase_hex_and_unique() {
        let first = CreatorToken::generate().reveal();
        let second = CreatorToken::generate().reveal();
        assert_eq!(first.len(), 64);
        assert!(first
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch)));
        assert_ne!(first, second);
    }

    #[test]
    fn tokens_match_only_themselves_and_never_print() {
        let token = CreatorToken::generate();
        let text = token.reveal();
        assert!(token.matches(&text));
        assert!(!token.matches(""));
        assert!(!token.matches(&text[..63]));
        assert!(!token.matches(&format!("{text}0")));
        let debug = format!("{token:?}");
        assert!(!debug.contains(&text), "{debug}");
        assert_eq!(debug, "CreatorToken(<redacted>)");
    }

    #[test]
    fn secret_comparison_requires_an_exact_match() {
        assert!(secrets_match("abc123", "abc123"));
        assert!(!secrets_match("abc123", "abc124"));
        assert!(!secrets_match("abc123", "abc12"));
        assert!(!secrets_match("abc123", ""));
    }
}
