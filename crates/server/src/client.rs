// SPDX-License-Identifier: AGPL-3.0-or-later
//! The outbound side of one WebSocket connection.
//!
//! Each connection has a small bounded queue drained by its own writer task.
//! Senders never wait: a client whose queue is full is too slow to keep up,
//! so it is kicked (its connection task wakes and disconnects it cleanly)
//! instead of buffering without bound (R15).

use std::sync::Arc;

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes};
use serde::Serialize;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Notify};

/// Messages queued per connection before it counts as too slow.
pub const OUTBOUND_CAPACITY: usize = 64;

/// A server-unique connection identifier.
pub type ConnId = u64;

/// A cheap, cloneable handle for sending to one connection.
#[derive(Debug, Clone)]
pub struct Client {
    id: ConnId,
    tx: mpsc::Sender<Message>,
    kick: Arc<Notify>,
}

impl Client {
    /// A handle plus the queue its writer task drains.
    pub fn new(id: ConnId) -> (Self, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::channel(OUTBOUND_CAPACITY);
        (
            Self {
                id,
                tx,
                kick: Arc::new(Notify::new()),
            },
            rx,
        )
    }

    /// This connection's identifier.
    pub fn id(&self) -> ConnId {
        self.id
    }

    /// Queue an already-encoded JSON text frame.
    pub fn send_text(&self, text: &Utf8Bytes) {
        self.push(Message::Text(text.clone()));
    }

    /// Encode and queue one message.
    pub fn send<T: Serialize>(&self, message: &T) {
        if let Some(text) = encode(message) {
            self.send_text(&text);
        }
    }

    /// Queue a normal close frame; the writer stops after sending it.
    pub fn close(&self, reason: &'static str) {
        self.push(Message::Close(Some(CloseFrame {
            code: 1000,
            reason: Utf8Bytes::from_static(reason),
        })));
    }

    /// Resolves once this client has been kicked for being too slow.
    pub async fn kicked(&self) {
        self.kick.notified().await;
    }

    fn push(&self, message: Message) {
        match self.tx.try_send(message) {
            Ok(()) | Err(TrySendError::Closed(_)) => {}
            Err(TrySendError::Full(_)) => {
                tracing::debug!(conn = self.id, "client too slow; disconnecting");
                self.kick.notify_one();
            }
        }
    }
}

/// Serialize a message once, for sending to any number of clients.
pub fn encode<T: Serialize>(message: &T) -> Option<Utf8Bytes> {
    match serde_json::to_string(message) {
        Ok(text) => Some(Utf8Bytes::from(text)),
        Err(error) => {
            tracing::error!(%error, "cannot encode a WebSocket message");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::ws::Message;

    use super::{Client, OUTBOUND_CAPACITY};

    #[tokio::test]
    async fn a_full_queue_kicks_instead_of_blocking() {
        let (client, mut rx) = Client::new(7);
        for _ in 0..OUTBOUND_CAPACITY {
            client.send(&"x");
        }
        // One past capacity: dropped, and the client is marked as kicked.
        client.send(&"overflow");
        tokio::time::timeout(std::time::Duration::from_secs(1), client.kicked())
            .await
            .expect("kicked");
        let mut received = 0;
        while let Ok(message) = rx.try_recv() {
            assert!(matches!(message, Message::Text(ref text) if text.as_str() == "\"x\""));
            received += 1;
        }
        assert_eq!(received, OUTBOUND_CAPACITY);
    }
}
