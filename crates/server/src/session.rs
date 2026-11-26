// SPDX-License-Identifier: AGPL-3.0-or-later
//! One live session: its state, its connected clients, and the handling of
//! every `/ws/sessions/{id}` message with strict role checks.
//!
//! Each handler runs with the session lock held, so a message is validated,
//! applied, logged, and broadcast as one atomic step. A rejected message
//! never mutates state, broadcasts, or appends to the statistics log.
//!
//! A connection passes through up to three roles:
//!
//! - **pending** right after joining: it already receives every broadcast
//!   and may use everything a viewer may, but is not counted as a viewer
//!   yet, because the creator's browser authenticates right after
//!   connecting and must never show up as a passing viewer in the lobby or
//!   in the `viewer_count` statistics;
//! - **viewer** once [`ServerConfig::auth_window`](crate::config::ServerConfig)
//!   passes without a valid `auth`. This depends on time alone, never on a
//!   message, so no rejected message can change the count;
//! - **creator** after a valid `auth`. There is at most one creator: a
//!   newer authenticated connection (a refreshed tab, a second device)
//!   supersedes the older one, which is closed.
//!
//! In a session that collects viewer names, a connection other than the
//! creator must `identify` before its `set_table` is accepted. The name is
//! kept for that connection only, never broadcast, and written to the
//! statistics log as the `actor` of its changes; the creator is always
//! logged as `host`, both for its own clicks and for the changes only it
//! can cause (Reset All, a range change).
//!
//! The receiver (the creator's browser serial bridge) is connected only
//! while the current creator says so. Whenever it stops being connected,
//! the disconnect-grace watchdog is armed; it is canceled only when the
//! receiver connects again. So `receiver_connected == false` always means
//! the watchdog is running.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::task::AbortHandle;
use trafficlights_proto::{
    normalize_session_name, normalize_viewer_name, ChangeSource, Color, EndReason, FirmwareInfo,
    FirmwareMode, JsonlEvent, SerialCommand, SessionClientMsg, SessionServerMsg, SessionSummary,
    TableRange, TapMode, HOST_ACTOR, MAX_STATION_ID,
};

use crate::board::Board;
use crate::client::{encode, Client, ConnId};
use crate::log::SessionLog;
use crate::state::{now_ms, AppState, Lookup, SessionCell, MAX_RANGE_LEN};

/// Longest serial port label kept, in characters.
const MAX_PORT_LABEL_CHARS: usize = 120;
/// Longest firmware version string kept, in characters.
const MAX_FIRMWARE_VERSION_CHARS: usize = 64;
/// Longest viewer key kept from `identify`, in bytes.
const MAX_VIEWER_KEY_BYTES: usize = 64;
/// Creator state-mutating messages: burst allowed at once.
pub const CREATOR_MUTATION_BURST: u32 = 200;
/// Creator state-mutating messages: refill per second.
pub const CREATOR_MUTATION_PER_SECOND: u32 = 50;

/// A token bucket for creator state-mutating messages.
#[derive(Debug, Clone, Copy)]
pub struct TokenBucket {
    burst: f64,
    per_second: f64,
    tokens: f64,
    updated: Instant,
}

impl TokenBucket {
    pub fn new(burst: u32, per_second: u32, now: Instant) -> Self {
        Self {
            burst: f64::from(burst),
            per_second: f64::from(per_second),
            tokens: f64::from(burst),
            updated: now,
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
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

/// The mutable state of one active session.
#[derive(Debug)]
pub struct Session {
    id: String,
    name: String,
    created_at_ms: i64,
    board: Board,
    receiver_connected: bool,
    port_label: Option<String>,
    firmware: Option<FirmwareInfo>,
    tap_mode: TapMode,
    tap_mode_locked: bool,
    beeps_enabled: bool,
    beeps_locked: bool,
    collect_names: bool,
    creator: Option<Client>,
    viewers: HashMap<ConnId, Client>,
    pending: HashMap<ConnId, Client>,
    /// Names given with `identify`, by connection (never the creator's).
    viewer_names: HashMap<ConnId, ViewerName>,
    pub(crate) log: SessionLog,
    grace_generation: u64,
    grace_task: Option<AbortHandle>,
    end_reason: Option<EndReason>,
    creator_bucket: TokenBucket,
    log_failure_notified: bool,
}

impl Session {
    /// A fresh all-green session whose receiver is not yet connected.
    /// `collect_names` is fixed for the session's lifetime.
    pub fn new(
        id: String,
        name: String,
        range: TableRange,
        collect_names: bool,
        now_ms: i64,
        log: SessionLog,
    ) -> Self {
        Self {
            id,
            name,
            created_at_ms: now_ms,
            board: Board::new(range),
            receiver_connected: false,
            port_label: None,
            firmware: None,
            tap_mode: TapMode::Green,
            tap_mode_locked: false,
            beeps_enabled: false,
            beeps_locked: false,
            collect_names,
            creator: None,
            viewers: HashMap::new(),
            pending: HashMap::new(),
            viewer_names: HashMap::new(),
            log,
            grace_generation: 0,
            grace_task: None,
            end_reason: None,
            creator_bucket: TokenBucket::new(
                CREATOR_MUTATION_BURST,
                CREATOR_MUTATION_PER_SECOND,
                Instant::now(),
            ),
            log_failure_notified: false,
        }
    }

    /// The lobby summary.
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            session_id: self.id.clone(),
            name: self.name.clone(),
            range: self.board.range(),
            counts: self.board.counts(),
            receiver_connected: self.receiver_connected,
            firmware: self.firmware.clone(),
            viewer_count: self.viewer_count(),
            created_at_ms: self.created_at_ms,
        }
    }

    /// Connected viewers, excluding the creator and connections still
    /// inside their authentication window.
    pub fn viewer_count(&self) -> u32 {
        u32::try_from(self.viewers.len()).unwrap_or(u32::MAX)
    }

    /// Why the session ended, once it has.
    pub fn end_reason(&self) -> Option<EndReason> {
        self.end_reason
    }

    pub(crate) fn set_ended(&mut self, reason: EndReason) {
        self.end_reason = Some(reason);
    }

    /// Invalidate any armed grace watchdog (aborting it) and return the
    /// generation a newly armed one must carry.
    pub(crate) fn next_grace_generation(&mut self) -> u64 {
        self.grace_generation = self.grace_generation.wrapping_add(1);
        if let Some(task) = self.grace_task.take() {
            task.abort();
        }
        self.grace_generation
    }

    pub(crate) fn grace_generation(&self) -> u64 {
        self.grace_generation
    }

    pub(crate) fn set_grace_task(&mut self, task: AbortHandle) {
        self.grace_task = Some(task);
    }

    pub(crate) fn forget_grace_task(&mut self) {
        self.grace_task = None;
    }

    /// Detach every client (for finalization).
    pub(crate) fn take_clients(&mut self) -> Vec<Client> {
        let mut clients: Vec<Client> = self.viewers.drain().map(|(_, client)| client).collect();
        clients.extend(self.pending.drain().map(|(_, client)| client));
        clients.extend(self.creator.take());
        self.viewer_names.clear();
        clients
    }

    /// Who the statistics log names for a change made by connection `id`:
    /// nobody unless the session collects names; then [`HOST_ACTOR`] for
    /// the creator and the given name for anyone else (`None` before that
    /// connection identified, which `set_table` refuses).
    fn actor(&self, id: ConnId) -> Option<String> {
        if !self.collect_names {
            return None;
        }
        if self.is_creator(id) {
            return Some(HOST_ACTOR.to_string());
        }
        self.viewer_names.get(&id).map(|viewer| viewer.name.clone())
    }

    /// The actor of changes only the creator can cause.
    fn host_actor(&self) -> Option<String> {
        self.collect_names.then(|| HOST_ACTOR.to_string())
    }

    fn is_creator(&self, id: ConnId) -> bool {
        self.creator
            .as_ref()
            .is_some_and(|creator| creator.id() == id)
    }

    fn is_member(&self, id: ConnId) -> bool {
        self.is_creator(id) || self.viewers.contains_key(&id) || self.pending.contains_key(&id)
    }

    /// Every connection that is not the creator.
    fn non_creators(&self) -> impl Iterator<Item = &Client> {
        self.viewers.values().chain(self.pending.values())
    }

    fn clients(&self) -> impl Iterator<Item = &Client> {
        self.creator.iter().chain(self.non_creators())
    }

    fn broadcast(&self, message: &SessionServerMsg) {
        if let Some(text) = encode(message) {
            for client in self.clients() {
                client.send_text(&text);
            }
        }
    }

    fn send_to_creator(&self, message: &SessionServerMsg) {
        if let Some(creator) = &self.creator {
            creator.send(message);
        }
    }

    fn snapshot(&self, is_creator: bool, state: &AppState) -> SessionServerMsg {
        SessionServerMsg::Snapshot {
            session_id: self.id.clone(),
            name: self.name.clone(),
            range: self.board.range(),
            tables: self.board.entries(),
            counts: self.board.counts(),
            receiver_connected: self.receiver_connected,
            port_label: self.port_label.clone(),
            firmware: self.firmware.clone(),
            viewer_count: self.viewer_count(),
            tap_mode: self.tap_mode,
            tap_mode_locked: self.tap_mode_locked,
            beeps_enabled: self.beeps_enabled,
            beeps_locked: self.beeps_locked,
            collect_names: self.collect_names,
            is_creator,
            unlisted_tables: if is_creator {
                self.board.unlisted_tables()
            } else {
                Vec::new()
            },
            server_now_ms: now_ms(),
            server_info: Box::new(state.build_info().clone()),
        }
    }

    /// Send every client a snapshot tailored to its role: only the
    /// creator's copy carries the unlisted (out-of-range) stations.
    fn send_snapshots(&self, state: &AppState) {
        if let Some(text) = encode(&self.snapshot(false, state)) {
            for client in self.non_creators() {
                client.send_text(&text);
            }
        }
        self.send_to_creator(&self.snapshot(true, state));
    }

    fn meta(&self) -> SessionServerMsg {
        SessionServerMsg::SessionMeta {
            name: self.name.clone(),
            range: self.board.range(),
            viewer_count: self.viewer_count(),
            tap_mode: self.tap_mode,
            tap_mode_locked: self.tap_mode_locked,
            beeps_enabled: self.beeps_enabled,
            beeps_locked: self.beeps_locked,
        }
    }

    fn receiver_status(&self) -> SessionServerMsg {
        SessionServerMsg::ReceiverStatus {
            connected: self.receiver_connected,
            port_label: self.port_label.clone(),
            firmware: self.firmware.clone(),
        }
    }

    fn table_update(&self, table: u16) -> SessionServerMsg {
        SessionServerMsg::TableUpdate {
            table,
            color: self.board.color(table),
            red_since_ms: self.board.red_since_ms(table),
            counts: self.board.counts(),
        }
    }

    /// Tell the creator once that statistics logging stopped.
    fn surface_log_failure(&mut self, newly_failed: bool) {
        if newly_failed && !self.log_failure_notified {
            self.log_failure_notified = true;
            self.send_to_creator(&error(
                "log_write_failed",
                "Statistics logging stopped (the log could not be written). The live board keeps running.",
            ));
        }
    }

    /// Log the viewer count and tell everyone, after a viewer joined or
    /// left.
    async fn viewers_changed(&mut self, state: &AppState, now: i64) {
        let viewers = self.viewer_count();
        let failed = self
            .log
            .record(now, |ts_ms, elapsed_ms| JsonlEvent::ViewerCount {
                ts_ms,
                elapsed_ms,
                viewers,
            })
            .await;
        self.surface_log_failure(failed);
        self.broadcast(&self.meta());
        state.publish(self);
    }

    /// Count a pending connection as a viewer; a no-op for any other.
    async fn settle_member(&mut self, state: &AppState, id: ConnId, now: i64) {
        if let Some(client) = self.pending.remove(&id) {
            self.viewers.insert(id, client);
            self.viewers_changed(state, now).await;
        }
    }

    /// The receiver is no longer reachable (the creator said so, left, or
    /// was superseded): mark it disconnected, log it, and arm the grace
    /// watchdog. A no-op while it is already disconnected, so the watchdog
    /// keeps counting from the first moment the receiver went away.
    async fn receiver_lost(&mut self, state: &AppState, cell: &Arc<SessionCell>, now: i64) {
        if !self.receiver_connected {
            return;
        }
        self.receiver_connected = false;
        self.port_label = None;
        self.firmware = None;
        self.log_receiver_status(now).await;
        state.arm_grace(cell, self);
        self.broadcast(&self.receiver_status());
        state.publish(self);
    }

    async fn log_receiver_status(&mut self, now: i64) {
        let connected = self.receiver_connected;
        let firmware = self.firmware.clone();
        let failed = self
            .log
            .record(now, |ts_ms, elapsed_ms| JsonlEvent::ReceiverStatus {
                ts_ms,
                elapsed_ms,
                connected,
                firmware,
            })
            .await;
        self.surface_log_failure(failed);
    }

    async fn log_state_change(&mut self, now: i64, change: Transition) {
        let failed = self
            .log
            .record(now, |ts_ms, elapsed_ms| JsonlEvent::StateChange {
                ts_ms,
                elapsed_ms,
                table: change.table,
                prev: change.prev,
                next: change.next,
                source: change.source,
                in_range: change.in_range,
                actor: change.actor,
            })
            .await;
        self.surface_log_failure(failed);
    }
}

/// A name given with `identify`, with the giving browser's key.
#[derive(Debug)]
struct ViewerName {
    name: String,
    key: Option<String>,
}

/// One table transition for the statistics log.
struct Transition {
    table: u16,
    prev: Option<Color>,
    next: Color,
    source: ChangeSource,
    in_range: bool,
    /// Who made it, in a session that collects viewer names.
    actor: Option<String>,
}

fn error(code: &str, message: &str) -> SessionServerMsg {
    SessionServerMsg::Error {
        code: code.to_string(),
        message: message.to_string(),
    }
}

/// Strip ASCII control characters from client-supplied display text.
fn sanitize_text(text: String) -> String {
    text.chars().filter(|ch| !ch.is_ascii_control()).collect()
}

fn truncate_chars(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        text
    } else {
        text.chars().take(max).collect()
    }
}

/// Keep client-supplied firmware strings bounded in memory and on disk.
fn bound_firmware(mut firmware: FirmwareInfo) -> FirmwareInfo {
    firmware.version = firmware
        .version
        .map(|version| truncate_chars(sanitize_text(version), MAX_FIRMWARE_VERSION_CHARS));
    firmware
}

/// Creator messages that mutate state and are token-bucket limited.
fn creator_mutating(message: &SessionClientMsg) -> bool {
    matches!(
        message,
        SessionClientMsg::StationReport { .. }
            | SessionClientMsg::SerialStatus { .. }
            | SessionClientMsg::ResetAll
            | SessionClientMsg::SetTableRange { .. }
            | SessionClientMsg::RenameSession { .. }
            | SessionClientMsg::SetTapMode { .. }
            | SessionClientMsg::SetBeepMode { .. }
    )
}

/// Messages only the authenticated creator may send.
fn creator_only(message: &SessionClientMsg) -> bool {
    !matches!(
        message,
        SessionClientMsg::Auth { .. }
            | SessionClientMsg::Identify { .. }
            | SessionClientMsg::SetTable { .. }
            | SessionClientMsg::Ping
    )
}

/// Join session `id`. The client gets a viewer snapshot at once and is
/// counted as a viewer later (see the module documentation). Ended or
/// unknown sessions are answered (and the client closed) here, returning
/// `None`.
pub(crate) async fn join(state: &AppState, id: &str, client: &Client) -> Option<Arc<SessionCell>> {
    let cell = match state.lookup(id) {
        Lookup::Active(cell) => cell,
        Lookup::Ended(reason) => {
            reply_ended(state, client, reason);
            return None;
        }
        Lookup::Missing => {
            reply_missing(client);
            return None;
        }
    };
    let mut session = cell.session.lock().await;
    if let Some(reason) = session.end_reason() {
        reply_ended(state, client, reason);
        return None;
    }
    session.pending.insert(client.id(), client.clone());
    client.send(&session.snapshot(false, state));
    drop(session);
    Some(cell)
}

/// Tell a client that asked for a malformed, unknown, or expired session
/// ID that it does not exist, and close it.
pub(crate) fn reply_missing(client: &Client) {
    client.send(&error("session_not_found", "Session not found."));
    client.close("session not found");
}

fn reply_ended(state: &AppState, client: &Client, reason: EndReason) {
    client.send(&SessionServerMsg::SessionEnded {
        reason,
        log_available: state.log_available(reason),
    });
    client.close("session ended");
}

/// The authentication window of `client` passed without a valid `auth`:
/// count it as a viewer from now on.
pub(crate) async fn settle(state: &AppState, cell: &Arc<SessionCell>, client: &Client) {
    let mut session = cell.session.lock().await;
    if session.end_reason().is_none() {
        session.settle_member(state, client.id(), now_ms()).await;
    }
}

/// Remove a disconnected client. A viewer leaving updates the viewer
/// count; the current creator leaving makes the receiver unreachable, which
/// arms the grace watchdog. A superseded creator leaving changes nothing.
pub(crate) async fn leave(state: &AppState, cell: &Arc<SessionCell>, client: &Client) {
    let mut session = cell.session.lock().await;
    if session.end_reason().is_some() {
        return;
    }
    let now = now_ms();
    session.pending.remove(&client.id());
    session.viewer_names.remove(&client.id());
    if session.viewers.remove(&client.id()).is_some() {
        session.viewers_changed(state, now).await;
    } else if session.is_creator(client.id()) {
        session.creator = None;
        session.receiver_lost(state, cell, now).await;
    }
}

/// Apply one message from `me`.
pub(crate) async fn handle(
    state: &AppState,
    cell: &Arc<SessionCell>,
    me: &Client,
    message: SessionClientMsg,
) {
    let mut session = cell.session.lock().await;
    // Ended sessions and superseded or kicked connections act on nothing.
    if session.end_reason().is_some() || !session.is_member(me.id()) {
        return;
    }
    let now = now_ms();
    if creator_only(&message) && !session.is_creator(me.id()) {
        me.send(&error("forbidden", "Creator authorization required."));
        return;
    }
    if creator_mutating(&message) && !session.creator_bucket.allow(Instant::now()) {
        me.send(&error(
            "rate_limited",
            "Too many requests; please slow down.",
        ));
        return;
    }
    match message {
        SessionClientMsg::Ping => me.send(&SessionServerMsg::Pong),
        SessionClientMsg::Auth { creator_token } => {
            auth(state, cell, &mut session, me, &creator_token, now).await;
        }
        SessionClientMsg::Identify { name, key } => identify(&mut session, me, &name, key),
        SessionClientMsg::SetTable { table, color } => {
            set_table(state, &mut session, me, table, color, now).await;
        }
        SessionClientMsg::StationReport { table, color } => {
            station_report(state, &mut session, me, table, color, now).await;
        }
        SessionClientMsg::SerialStatus {
            connected,
            port_label,
            firmware,
        } => {
            let port_label =
                port_label.map(|label| truncate_chars(sanitize_text(label), MAX_PORT_LABEL_CHARS));
            let firmware = firmware.map(bound_firmware);
            serial_status(
                state,
                cell,
                &mut session,
                connected,
                port_label,
                firmware,
                now,
            )
            .await;
        }
        SessionClientMsg::ResetAll => reset_all(state, &mut session, me, now).await,
        SessionClientMsg::SetTableRange { start, end } => {
            set_table_range(state, &mut session, me, start, end, now).await;
        }
        SessionClientMsg::RenameSession { name } => match normalize_session_name(&name) {
            Ok(name) => {
                session.name = name;
                session.broadcast(&session.meta());
                state.publish(&session);
            }
            Err(_) => me.send(&error(
                "invalid_name",
                "Session name must be 1 to 80 characters without control characters.",
            )),
        },
        SessionClientMsg::SetTapMode { tap_mode, locked } => {
            if session.tap_mode != tap_mode || session.tap_mode_locked != locked {
                session.tap_mode = tap_mode;
                session.tap_mode_locked = locked;
                session.broadcast(&session.meta());
            }
        }
        SessionClientMsg::SetBeepMode { enabled, locked } => {
            if session.beeps_enabled != enabled || session.beeps_locked != locked {
                session.beeps_enabled = enabled;
                session.beeps_locked = locked;
                session.broadcast(&session.meta());
            }
        }
        SessionClientMsg::EndSession => {
            state.finalize(cell, &mut session, EndReason::Manual).await;
        }
    }
}

async fn auth(
    state: &AppState,
    cell: &Arc<SessionCell>,
    session: &mut Session,
    me: &Client,
    provided: &str,
    now: i64,
) {
    if !cell.token.matches(provided) {
        me.send(&error("unauthorized", "Invalid creator token."));
        return;
    }
    if !session.is_creator(me.id()) {
        session.pending.remove(&me.id());
        // From now on this connection is logged as the host.
        session.viewer_names.remove(&me.id());
        let was_viewer = session.viewers.remove(&me.id()).is_some();
        if let Some(previous) = session.creator.replace(me.clone()) {
            // A refreshed tab or second device took over. The old socket is
            // no longer a member, so its eventual cleanup changes nothing.
            // Its browser held the serial bridge, though, so the receiver is
            // unreachable until this connection reports its own.
            previous.send(&error(
                "superseded",
                "Creator controls moved to a newer connection.",
            ));
            previous.close("superseded");
            session.receiver_lost(state, cell, now).await;
        }
        if was_viewer {
            session.viewers_changed(state, now).await;
        }
    }
    me.send(&SessionServerMsg::AuthOk {
        is_creator: true,
        unlisted_tables: session.board.unlisted_tables(),
    });
}

/// A viewer gives its name, in a session that collects names. No two
/// connections hold the same name (ignoring letter case) at once, except
/// that a connection presenting the holder's own browser key takes the name
/// over from that stale connection. A rejected name changes nothing (an
/// earlier valid name stays). The creator needs no name: it is always
/// logged as the host.
fn identify(session: &mut Session, me: &Client, name: &str, key: Option<String>) {
    if !session.collect_names {
        me.send(&error(
            "names_not_collected",
            "This session does not collect viewer names.",
        ));
        return;
    }
    if session.is_creator(me.id()) {
        return;
    }
    let Ok(name) = normalize_viewer_name(name) else {
        me.send(&error(
            "invalid_viewer_name",
            "Name must be 1 to 40 characters without control characters, and not \"host\".",
        ));
        return;
    };
    let key = key.filter(|key| valid_viewer_key(key));
    let wanted = name.to_lowercase();
    let holder = session
        .viewer_names
        .iter()
        .find(|(id, held)| **id != me.id() && held.name.to_lowercase() == wanted)
        .map(|(id, held)| (*id, held.key.clone()));
    if let Some((holder, held_key)) = holder {
        if key.is_none() || held_key != key {
            me.send(&error(
                "name_taken",
                "Someone in this session already uses that name. Choose another one.",
            ));
            return;
        }
        // The same browser reconnected before its old connection dropped.
        session.viewer_names.remove(&holder);
    }
    session
        .viewer_names
        .insert(me.id(), ViewerName { name, key });
}

/// Whether a viewer key is short ASCII letters, digits, and hyphens; any
/// other key is ignored, which only disables the reconnect takeover.
fn valid_viewer_key(key: &str) -> bool {
    (1..=MAX_VIEWER_KEY_BYTES).contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// Any viewer may set an in-range table, but only while the receiver is
/// connected: the command is relayed to the creator's serial bridge first,
/// and state changes only then. Offline, nothing changes and nothing is
/// logged (fixing the baseline's state-before-serial bug). In a session
/// that collects viewer names, a viewer must have identified first.
async fn set_table(
    state: &AppState,
    session: &mut Session,
    me: &Client,
    table: u16,
    color: Color,
    now: i64,
) {
    let actor = session.actor(me.id());
    if session.collect_names && actor.is_none() {
        me.send(&error(
            "name_required",
            "Enter your name before changing tables.",
        ));
        return;
    }
    if !session.board.range().contains(table) {
        me.send(&error(
            "table_out_of_range",
            "Table is outside the session range.",
        ));
        return;
    }
    if session.creator.is_none() || !session.receiver_connected {
        me.send(&error("receiver_disconnected", "Receiver is disconnected."));
        return;
    }
    session.send_to_creator(&SessionServerMsg::SerialCommand {
        command: SerialCommand::SetTable { table, color },
    });
    if let Some(prev) = session.board.set_color(table, color, now) {
        session
            .log_state_change(
                now,
                Transition {
                    table,
                    prev: Some(prev),
                    next: color,
                    source: ChangeSource::TeacherClick,
                    in_range: true,
                    actor,
                },
            )
            .await;
        session.broadcast(&session.table_update(table));
        state.publish(session);
    }
}

/// A station's color as heard by the creator's serial bridge. In-range
/// tables update the board for everyone; out-of-range stations are
/// remembered and shown to the creator only.
async fn station_report(
    state: &AppState,
    session: &mut Session,
    me: &Client,
    table: u16,
    color: Color,
    now: i64,
) {
    if table > MAX_STATION_ID {
        me.send(&error("invalid_table", "Station ID must be at most 9999."));
        return;
    }
    if session.board.range().contains(table) {
        if let Some(prev) = session.board.set_color(table, color, now) {
            session
                .log_state_change(
                    now,
                    Transition {
                        table,
                        prev: Some(prev),
                        next: color,
                        source: ChangeSource::StudentRadio,
                        in_range: true,
                        actor: None,
                    },
                )
                .await;
            session.broadcast(&session.table_update(table));
            state.publish(session);
        }
        return;
    }
    if let Some(prev) = session.board.record_unlisted(table, color, now) {
        session
            .log_state_change(
                now,
                Transition {
                    table,
                    prev,
                    next: color,
                    source: ChangeSource::StudentRadio,
                    in_range: false,
                    actor: None,
                },
            )
            .await;
    }
    session.send_to_creator(&SessionServerMsg::UnlistedTables {
        tables: session.board.unlisted_tables(),
    });
}

async fn serial_status(
    state: &AppState,
    cell: &Arc<SessionCell>,
    session: &mut Session,
    connected: bool,
    port_label: Option<String>,
    firmware: Option<FirmwareInfo>,
    now: i64,
) {
    if !connected {
        session.receiver_lost(state, cell, now).await;
        return;
    }
    // A station-mode board is not a receiver: keep it disconnected (and
    // keep the grace watchdog as it is) while still showing its identity.
    let is_station = firmware
        .as_ref()
        .is_some_and(|info| info.mode == Some(FirmwareMode::Station));
    if is_station {
        let was_connected = session.receiver_connected;
        let changed = was_connected || session.firmware != firmware;
        session.receiver_connected = false;
        session.port_label = port_label;
        session.firmware = firmware;
        if was_connected {
            state.arm_grace(cell, session);
        }
        if changed {
            session.log_receiver_status(now).await;
        }
        session.broadcast(&session.receiver_status());
        state.publish(session);
        return;
    }
    let changed = !session.receiver_connected || session.firmware != firmware;
    session.receiver_connected = true;
    session.port_label = port_label;
    session.firmware = firmware;
    state.cancel_grace(session);
    if changed {
        session.log_receiver_status(now).await;
    }
    session.broadcast(&session.receiver_status());
    state.publish(session);
}

async fn reset_all(state: &AppState, session: &mut Session, me: &Client, now: i64) {
    if session.creator.is_none() || !session.receiver_connected {
        me.send(&error("receiver_disconnected", "Receiver is disconnected."));
        return;
    }
    session.send_to_creator(&SessionServerMsg::SerialCommand {
        command: SerialCommand::ResetAll,
    });
    for (table, prev) in session.board.reset_to_green() {
        let actor = session.host_actor();
        session
            .log_state_change(
                now,
                Transition {
                    table,
                    prev: Some(prev),
                    next: Color::Green,
                    source: ChangeSource::TeacherResetAll,
                    in_range: true,
                    actor,
                },
            )
            .await;
    }
    session.send_snapshots(state);
    state.publish(session);
}

/// Change the displayed range. Every lit table of the old range is reset
/// to green and logged as `range_reset` while it was still in range, then
/// the `range_change` line records the new denominator.
async fn set_table_range(
    state: &AppState,
    session: &mut Session,
    me: &Client,
    start: u16,
    end: u16,
    now: i64,
) {
    let Ok(next_range) = TableRange::new(start, end) else {
        me.send(&error(
            "invalid_range",
            "Table range must satisfy 1 <= start <= end <= 9999 and contain at most 512 tables.",
        ));
        return;
    };
    if next_range.len() > MAX_RANGE_LEN {
        me.send(&error(
            "invalid_range",
            "Table range must satisfy 1 <= start <= end <= 9999 and contain at most 512 tables.",
        ));
        return;
    }
    let prev_range = session.board.range();
    let (reset, pruned) = session.board.change_range(next_range);
    for (table, prev) in reset {
        let actor = session.host_actor();
        session
            .log_state_change(
                now,
                Transition {
                    table,
                    prev: Some(prev),
                    next: Color::Green,
                    source: ChangeSource::RangeReset,
                    in_range: true,
                    actor,
                },
            )
            .await;
    }
    let failed = session
        .log
        .record(now, |ts_ms, elapsed_ms| JsonlEvent::RangeChange {
            ts_ms,
            elapsed_ms,
            prev_range,
            next_range,
        })
        .await;
    session.surface_log_failure(failed);
    session.broadcast(&session.meta());
    session.send_snapshots(state);
    if pruned {
        // An empty list is omitted from the snapshot, so say it explicitly.
        session.send_to_creator(&SessionServerMsg::UnlistedTables {
            tables: session.board.unlisted_tables(),
        });
    }
    state.publish(session);
}

#[cfg(test)]
mod tests {
    use trafficlights_proto::{Color, FirmwareInfo, SessionClientMsg};

    use std::time::{Duration, Instant};

    use super::{
        bound_firmware, creator_mutating, creator_only, sanitize_text, truncate_chars,
        valid_viewer_key, TokenBucket, MAX_FIRMWARE_VERSION_CHARS,
    };

    #[test]
    fn viewer_keys_are_short_hyphenated_alphanumerics() {
        assert!(valid_viewer_key("0195f1a2-3b4c-7d8e-9f0a-1b2c3d4e5f60"));
        assert!(valid_viewer_key(&"k".repeat(64)));
        for key in ["", "has space", "a.b", "\u{e9}", "k\n"] {
            assert!(!valid_viewer_key(key), "{key:?}");
        }
        assert!(!valid_viewer_key(&"k".repeat(65)));
    }

    #[test]
    fn only_auth_identify_set_table_and_ping_are_open_to_viewers() {
        let open = [
            SessionClientMsg::Auth {
                creator_token: String::new(),
            },
            SessionClientMsg::Identify {
                name: "Alice".to_string(),
                key: None,
            },
            SessionClientMsg::SetTable {
                table: 1,
                color: Color::Red,
            },
            SessionClientMsg::Ping,
        ];
        for message in &open {
            assert!(!creator_only(message), "{message:?}");
        }
        let creator = [
            SessionClientMsg::StationReport {
                table: 1,
                color: Color::Red,
            },
            SessionClientMsg::SerialStatus {
                connected: true,
                port_label: None,
                firmware: None,
            },
            SessionClientMsg::ResetAll,
            SessionClientMsg::SetTableRange { start: 1, end: 2 },
            SessionClientMsg::RenameSession {
                name: "x".to_string(),
            },
            SessionClientMsg::EndSession,
        ];
        for message in &creator {
            assert!(creator_only(message), "{message:?}");
        }
    }

    #[test]
    fn client_strings_are_bounded_by_characters() {
        assert_eq!(truncate_chars("abc".to_string(), 3), "abc");
        assert_eq!(truncate_chars("abcd".to_string(), 3), "abc");
        assert_eq!(
            truncate_chars("\u{e9}\u{e9}\u{e9}\u{e9}".to_string(), 2),
            "\u{e9}\u{e9}"
        );
        let firmware = bound_firmware(FirmwareInfo {
            version: Some("v".repeat(500)),
            ..FirmwareInfo::legacy()
        });
        assert_eq!(
            firmware.version.map(|version| version.chars().count()),
            Some(MAX_FIRMWARE_VERSION_CHARS)
        );
    }

    #[test]
    fn control_characters_are_stripped_from_display_text() {
        assert_eq!(sanitize_text("a\u{0}b\nc\u{7f}d".to_string()), "abcd");
        assert_eq!(sanitize_text("plain".to_string()), "plain");
        let firmware = bound_firmware(FirmwareInfo {
            version: Some("v1\u{1b}[31m".to_string()),
            ..FirmwareInfo::legacy()
        });
        assert_eq!(firmware.version.as_deref(), Some("v1[31m"));
    }

    #[test]
    fn creator_mutating_covers_exactly_the_state_changing_messages() {
        assert!(creator_mutating(&SessionClientMsg::StationReport {
            table: 1,
            color: Color::Red,
        }));
        assert!(creator_mutating(&SessionClientMsg::SerialStatus {
            connected: true,
            port_label: None,
            firmware: None,
        }));
        assert!(creator_mutating(&SessionClientMsg::ResetAll));
        assert!(creator_mutating(&SessionClientMsg::SetTableRange {
            start: 1,
            end: 2
        }));
        assert!(creator_mutating(&SessionClientMsg::RenameSession {
            name: "x".to_string()
        }));
        assert!(!creator_mutating(&SessionClientMsg::EndSession));
        assert!(!creator_mutating(&SessionClientMsg::Ping));
        assert!(!creator_mutating(&SessionClientMsg::SetTable {
            table: 1,
            color: Color::Red,
        }));
    }

    #[test]
    fn the_creator_bucket_allows_a_burst_then_refills() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(200, 50, start);
        assert!((0..200).all(|_| bucket.allow(start)));
        assert!(!bucket.allow(start));
        let later = start + Duration::from_secs(1);
        assert!((0..50).all(|_| bucket.allow(later)));
        assert!(!bucket.allow(later));
    }
}
