// SPDX-License-Identifier: AGPL-3.0-or-later
//! The append-only session statistics `.jsonl` file.
//!
//! Every event is serialized, appended, and flushed to the operating system
//! immediately, so nothing is buffered in memory (R15) and a download always
//! sees every event recorded so far. Closing the log also syncs it to the
//! disk. Timestamps are clamped so `ts_ms` and `elapsed_ms` never go
//! backward even if the wall clock does.

use std::io;
use std::path::{Path, PathBuf};

use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use trafficlights_proto::{JsonlEvent, TableRange, STATS_SCHEMA_VERSION};

/// Keeps a session's log timestamps monotonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonotonicClock {
    start_ts_ms: i64,
    last_ts_ms: i64,
}

impl MonotonicClock {
    /// A clock whose session started at `start_ts_ms`.
    pub fn new(start_ts_ms: i64) -> Self {
        Self {
            start_ts_ms,
            last_ts_ms: start_ts_ms,
        }
    }

    /// The `(ts_ms, elapsed_ms)` pair for an event observed at wall-clock
    /// `now_ms`: never earlier than the previous event.
    pub fn stamp(&mut self, now_ms: i64) -> (i64, u64) {
        let ts_ms = now_ms.max(self.last_ts_ms);
        self.last_ts_ms = ts_ms;
        let elapsed = u64::try_from(ts_ms.saturating_sub(self.start_ts_ms)).unwrap_or(0);
        (ts_ms, elapsed)
    }
}

/// An open session statistics log.
#[derive(Debug)]
pub struct SessionLog {
    file: Option<File>,
    path: PathBuf,
    clock: MonotonicClock,
}

/// The path of a session's log inside `dir`.
pub fn log_path(dir: &Path, session_id: &str) -> PathBuf {
    dir.join(format!("{session_id}.jsonl"))
}

impl SessionLog {
    /// Create `LOG_DIR` if needed, create the session's file (refusing to
    /// reuse an existing one), and write the `session_start` header line.
    pub async fn create(
        dir: &Path,
        session_id: &str,
        name: &str,
        range: TableRange,
        now_ms: i64,
    ) -> io::Result<Self> {
        tokio::fs::create_dir_all(dir).await?;
        let path = log_path(dir, session_id);
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)
            .await?;
        let mut log = Self {
            file: Some(file),
            path,
            clock: MonotonicClock::new(now_ms),
        };
        log.write(&JsonlEvent::SessionStart {
            schema_version: STATS_SCHEMA_VERSION,
            ts_ms: now_ms,
            elapsed_ms: 0,
            session_id: session_id.to_string(),
            name: name.to_string(),
            range,
        })
        .await?;
        Ok(log)
    }

    /// The file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event built from its monotonic `(ts_ms, elapsed_ms)`.
    /// A failed write is reported in the server log and otherwise ignored:
    /// the classroom keeps working even if the statistics disk is full.
    pub async fn record(&mut self, now_ms: i64, build: impl FnOnce(i64, u64) -> JsonlEvent) {
        let (ts_ms, elapsed_ms) = self.clock.stamp(now_ms);
        let event = build(ts_ms, elapsed_ms);
        if let Err(error) = self.write(&event).await {
            tracing::warn!(path = %self.path.display(), %error, "cannot append to session log");
        }
    }

    /// Flush, sync, and close the file; later records are dropped.
    pub async fn close(&mut self) {
        if let Some(mut file) = self.file.take() {
            if let Err(error) = file.flush().await {
                tracing::warn!(path = %self.path.display(), %error, "cannot flush session log");
            }
            if let Err(error) = file.sync_all().await {
                tracing::warn!(path = %self.path.display(), %error, "cannot sync session log");
            }
        }
    }

    async fn write(&mut self, event: &JsonlEvent) -> io::Result<()> {
        let Some(file) = self.file.as_mut() else {
            return Err(io::Error::other("session log already closed"));
        };
        let mut line = event.to_jsonl_line().map_err(io::Error::other)?;
        line.push('\n');
        file.write_all(line.as_bytes()).await?;
        file.flush().await
    }
}

#[cfg(test)]
mod tests {
    use trafficlights_proto::{EndReason, JsonlEvent, TableRange, STATS_SCHEMA_VERSION};

    use super::{log_path, MonotonicClock, SessionLog};

    #[tokio::test]
    async fn a_log_starts_with_its_header_appends_and_refuses_reuse() {
        let dir =
            std::env::temp_dir().join(format!("trafficlights-log-unit-{}", uuid::Uuid::now_v7()));
        let range = TableRange::default_classroom();
        let created = SessionLog::create(&dir, "abc", "Room 1", range, 5_000).await;
        let Ok(mut log) = created else {
            panic!("cannot create the log: {created:?}");
        };
        assert_eq!(log.path(), log_path(&dir, "abc"));
        log.record(7_500, |ts_ms, elapsed_ms| JsonlEvent::SessionEnd {
            ts_ms,
            elapsed_ms,
            reason: EndReason::Manual,
        })
        .await;
        log.close().await;
        // Closed logs drop later records instead of failing the caller.
        log.record(9_000, |ts_ms, elapsed_ms| JsonlEvent::ViewerCount {
            ts_ms,
            elapsed_ms,
            viewers: 1,
        })
        .await;

        let text = tokio::fs::read_to_string(log_path(&dir, "abc"))
            .await
            .unwrap_or_default();
        let events: Vec<JsonlEvent> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        assert_eq!(text.lines().count(), 2, "{text}");
        assert_eq!(
            events,
            vec![
                JsonlEvent::SessionStart {
                    schema_version: STATS_SCHEMA_VERSION,
                    ts_ms: 5_000,
                    elapsed_ms: 0,
                    session_id: "abc".to_string(),
                    name: "Room 1".to_string(),
                    range,
                },
                JsonlEvent::SessionEnd {
                    ts_ms: 7_500,
                    elapsed_ms: 2_500,
                    reason: EndReason::Manual,
                },
            ]
        );
        assert!(text.ends_with('\n'));

        // An existing file is never reused or truncated.
        assert!(SessionLog::create(&dir, "abc", "Again", range, 6_000)
            .await
            .is_err());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn stamps_never_go_backward() {
        let mut clock = MonotonicClock::new(1_000);
        assert_eq!(clock.stamp(1_500), (1_500, 500));
        // The wall clock stepped back: the stamp holds at the last value.
        assert_eq!(clock.stamp(1_200), (1_500, 500));
        assert_eq!(clock.stamp(900), (1_500, 500));
        assert_eq!(clock.stamp(2_000), (2_000, 1_000));
    }

    #[test]
    fn a_clock_before_the_start_reports_zero_elapsed() {
        let mut clock = MonotonicClock::new(1_000);
        assert_eq!(clock.stamp(10), (1_000, 0));
    }
}
